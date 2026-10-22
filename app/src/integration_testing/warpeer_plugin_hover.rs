//! Real terminal-link hover coverage for Warpeer plugins.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pathfinder_geometry::vector::vec2f;
use warpeer_plugin_protocol::{PluginId, SurfacePoint};
use warpui::integration::TestStep;
use warpui::{SingletonEntity as _, async_assert};

use super::step::new_step_with_default_assertions;
use super::view_getters::single_terminal_view;
use crate::terminal::block_list_element::GridType;
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::{WithinBlock, WithinModel};
use crate::warpeer_plugin::WarpeerPluginHost;

const IMAGE_PREVIEW_ID: &str = "dev.warpeer.image-preview";

fn image_preview_id() -> PluginId {
    PluginId::parse(IMAGE_PREVIEW_ID).expect("image preview plugin ID is valid")
}

pub fn hover_last_block_image_output(pointer: [f32; 2]) -> TestStep {
    new_step_with_default_assertions("Hover the image path emitted by ls").with_action(
        move |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                let block_index = terminal
                    .model
                    .lock()
                    .block_list()
                    .last_non_hidden_block()
                    .expect("ls output block should exist")
                    .index();
                let position = WithinModel::BlockList(WithinBlock::new(
                    Point::new(0, 0),
                    block_index,
                    GridType::Output,
                ));
                terminal.integration_hover_terminal_resource(
                    position,
                    vec2f(pointer[0], pointer[1]),
                    ctx,
                );
            });
        },
    )
}

pub fn leave_image_output_hover() -> TestStep {
    new_step_with_default_assertions("Leave the image link hover").with_action(
        |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                terminal.integration_leave_terminal_resource(ctx);
            });
        },
    )
}

pub fn pin_current_image_preview() -> TestStep {
    new_step_with_default_assertions("Pin the image preview from the link action").with_action(
        |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                terminal.integration_pin_current_plugin_preview(ctx);
            });
        },
    )
}

pub fn close_pinned_image_preview() -> TestStep {
    new_step_with_default_assertions("Close the pinned image preview").with_action(
        |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                terminal.integration_cancel_plugin_hover(ctx);
            });
        },
    )
}

pub fn move_pinned_image_preview(origin: [f32; 2]) -> TestStep {
    new_step_with_default_assertions("Move the pinned image preview container").with_action(
        move |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, _| {
                terminal.integration_move_plugin_container(SurfacePoint {
                    x: origin[0],
                    y: origin[1],
                });
            });
        },
    )
}

pub fn assert_pinned_image_page(
    title: &'static str,
    stack_depth: usize,
    surface_count: usize,
    origin: [f32; 2],
) -> TestStep {
    TestStep::new(&format!("Wait for Image Preview page: {title}"))
        .set_timeout(Duration::from_secs(25))
        .add_named_assertion(
            "page, surface, and origin are authoritative",
            move |app, window_id| {
                let context = single_terminal_view(app, window_id).read(app, |terminal, _| {
                    terminal.integration_plugin_hover_context()
                });
                let ready = context.as_ref().is_some_and(|context| {
                    app.read(|ctx| {
                        let host = WarpeerPluginHost::as_ref(ctx);
                        host.client()
                            .terminal_container(context)
                            .is_some_and(|presented| {
                                let snapshot = &presented.container;
                                snapshot.pages.len() == stack_depth
                                    && snapshot
                                        .active_page()
                                        .and_then(|page| page.chrome.title.as_deref())
                                        == Some(title)
                                    && (snapshot.origin.x - origin[0]).abs() < 0.5
                                    && (snapshot.origin.y - origin[1]).abs() < 0.5
                                    && presented.surface.frame.is_some()
                                    && host.integration_surface_count() == surface_count
                            })
                    })
                });
                async_assert!(
                    ready,
                    "Image Preview should preserve its single page and dragged origin"
                )
            },
        )
}

pub fn assert_detected_preview_ready(path: PathBuf, pointer: [f32; 2]) -> TestStep {
    TestStep::new("Wait for the ls image preview")
        .set_timeout(Duration::from_secs(25))
        .add_named_assertion(
            "file scanner, pointer anchor, worker, and frame are ready",
            move |app, window_id| {
                let context = single_terminal_view(app, window_id).read(app, |terminal, _| {
                    terminal.integration_plugin_hover_context()
                });
                let ready = context.as_ref().is_some_and(|context| {
                    let target_matches = context
                        .resolved_resource
                        .as_deref()
                        .is_some_and(|target| Path::new(target) == path);
                    let anchor_matches = (context.anchor[0] - pointer[0]).abs() < 0.5
                        && (context.anchor[1] - pointer[1]).abs() < 0.5;
                    let frame_ready = app.read(|ctx| {
                        WarpeerPluginHost::as_ref(ctx)
                            .client()
                            .terminal_surface(context)
                            .is_some_and(|surface| surface.frame.is_some())
                    });
                    target_matches && anchor_matches && frame_ready
                });
                async_assert!(
                    ready && live_external_image_bytes() > 0,
                    "ls image hover should resolve, stay pointer-anchored, and render image pixels"
                )
            },
        )
}

pub fn assert_hover_preview_retained() -> TestStep {
    TestStep::new("Assert hover preview is not dismissed immediately").add_named_assertion(
        "surface survives the pointer-exit grace period",
        |app, _| {
            let retained = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                host.integration_surface_count() == 1
                    && host.integration_hover_context_count() == 1
                    && host.worker_state(&image_preview_id()).is_some()
            });
            async_assert!(
                retained,
                "preview should remain visible immediately after pointer exit"
            )
        },
    )
}

pub fn assert_pinned_preview_ready(path: PathBuf, pointer: [f32; 2]) -> TestStep {
    TestStep::new("Wait for the pinned image preview")
        .set_timeout(Duration::from_secs(25))
        .add_named_assertion(
            "explicit preview is rendered and remains pointer-anchored",
            move |app, window_id| {
                let context = single_terminal_view(app, window_id).read(app, |terminal, _| {
                    terminal.integration_plugin_hover_context()
                });
                let surface_state = context.as_ref().map(|context| {
                    app.read(|ctx| {
                        WarpeerPluginHost::as_ref(ctx)
                            .client()
                            .terminal_surface(context)
                            .map(|surface| (surface.frame.is_some(), surface.descriptor.bounds))
                    })
                });
                let ready = context.as_ref().is_some_and(|context| {
                    let target_matches = context
                        .resolved_resource
                        .as_deref()
                        .is_some_and(|target| Path::new(target) == path);
                    let anchor_matches = (context.anchor[0] - pointer[0]).abs() < 0.5
                        && (context.anchor[1] - pointer[1]).abs() < 0.5;
                    let frame_ready = surface_state
                        .as_ref()
                        .and_then(|state| state.as_ref())
                        .is_some_and(|(ready, _)| *ready);
                    context.explicit_preview && target_matches && anchor_matches && frame_ready
                });
                let image_bytes = live_external_image_bytes();
                async_assert!(
                    ready && image_bytes > 0,
                    "pinned preview should retain its context and pixels: context={context:?}, surface={surface_state:?}, image_bytes={image_bytes}"
                )
            },
        )
}

pub fn assert_pinned_preview_retained() -> TestStep {
    TestStep::new("Assert pinned preview survives hover dismissal").add_named_assertion(
        "explicit surface remains until a close action",
        |app, window_id| {
            let explicit = single_terminal_view(app, window_id).read(app, |terminal, _| {
                terminal
                    .integration_plugin_hover_context()
                    .is_some_and(|context| context.explicit_preview)
            });
            let retained = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                host.integration_surface_count() == 1 && host.integration_hover_context_count() == 1
            });
            async_assert!(
                explicit && retained,
                "pinned preview should remain visible until it is explicitly closed"
            )
        },
    )
}

#[cfg(target_os = "macos")]
fn live_external_image_bytes() -> u64 {
    warpui::image_cache::live_external_image_bytes()
}

#[cfg(not(target_os = "macos"))]
fn live_external_image_bytes() -> u64 {
    0
}
