use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;
use sysinfo::{Pid, ProcessesToUpdate, System};
use warpeer_plugin_protocol::{
    Capability, ContextId, PluginId, Revision, TerminalHoverContext, WorkerState,
};
use warpeer_plugin_runtime::PluginSource;
use warpui::integration::TestStep;
use warpui::{App, SingletonEntity as _, ViewHandle, WindowId, async_assert};

use super::step::new_step_with_default_assertions;
use super::view_getters::single_terminal_view;
use crate::settings_view::{
    PluginsPageAction, PluginsSettingsPageView, empty_plugin_widget_id, plugin_detail_position_id,
    plugin_detail_widget_id, plugin_diagnostics_widget_id, plugin_inventory_widget_id,
    plugin_list_position_id,
};
use crate::warpeer_plugin::WarpeerPluginHost;
const IMAGE_PREVIEW_ID: &str = "dev.warpeer.image-preview";

#[derive(Clone)]
pub struct PreviewProbe {
    context: TerminalHoverContext,
}
impl PreviewProbe {
    pub fn local(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        Self::new(
            path.to_string_lossy().into_owned(),
            false,
            path.parent().map(Path::to_path_buf),
        )
    }

    pub fn remote(url: impl Into<String>) -> Self {
        Self::new(url.into(), true, None)
    }

    fn new(target: String, explicit_hyperlink: bool, working_directory: Option<PathBuf>) -> Self {
        Self {
            context: TerminalHoverContext {
                context_id: ContextId::new(),
                revision: Revision(1),
                terminal_id: "integration-terminal".into(),
                pane_id: "integration-pane".into(),
                workspace_id: Some("integration-workspace".into()),
                working_directory: working_directory
                    .map(|path| path.to_string_lossy().into_owned()),
                resource_text: target.clone(),
                resolved_resource: Some(target),
                explicit_hyperlink,
                explicit_preview: false,
                anchor: [24., 24., 9., 18.],
                viewport: [0., 0., 900., 600.],
            },
        }
    }
}

#[derive(Clone, Default)]
pub struct MemoryProbe(Arc<AtomicU64>);

impl MemoryProbe {
    fn store(&self, bytes: u64) {
        self.0.store(bytes, Ordering::Release);
    }

    fn load(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

fn image_preview_id() -> PluginId {
    PluginId::parse(IMAGE_PREVIEW_ID).expect("image preview plugin id is valid")
}

fn plugins_page(app: &App, window_id: WindowId) -> ViewHandle<PluginsSettingsPageView> {
    app.views_of_type::<PluginsSettingsPageView>(window_id)
        .and_then(|views| views.first().cloned())
        .expect("Plugins settings page should be mounted")
}

fn dispatch_plugin_action(action: PluginsPageAction) -> TestStep {
    new_step_with_default_assertions(&format!("Dispatch plugin action: {action:?}")).with_action(
        move |app, window_id, _| {
            let page = plugins_page(app, window_id);
            app.dispatch_typed_action(window_id, &[page.id()], &action);
        },
    )
}

fn widget_painted(app: &App, window_id: WindowId, id: &'static str) -> bool {
    app.presenter(window_id).is_some_and(|presenter| {
        presenter
            .borrow()
            .position_cache()
            .get_position(id)
            .is_some()
    })
}

fn widget_width(app: &App, window_id: WindowId, id: &'static str) -> Option<f32> {
    app.presenter(window_id).and_then(|presenter| {
        presenter
            .borrow()
            .position_cache()
            .get_position(id)
            .map(|bounds| bounds.size().x())
    })
}

pub fn assert_plugins_page_ready() -> TestStep {
    TestStep::new("Assert Plugins settings page is complete").add_named_assertion(
        "Plugins title, inventory, detail, and diagnostics are rendered",
        |app, window_id| {
            let page = plugins_page(app, window_id);
            let (title, selected) = page.read(app, |page, _| {
                (
                    page.integration_page_title(),
                    page.integration_selected_plugin().cloned(),
                )
            });
            async_assert!(
                title == "Plugins"
                    && selected == Some(image_preview_id())
                    && widget_painted(app, window_id, plugin_inventory_widget_id())
                    && widget_painted(app, window_id, plugin_detail_widget_id())
                    && widget_painted(app, window_id, plugin_diagnostics_widget_id())
                    && widget_painted(app, window_id, plugin_list_position_id())
                    && widget_painted(app, window_id, plugin_detail_position_id())
                    && widget_width(app, window_id, plugin_list_position_id())
                        .is_some_and(|width| width >= 240.)
                    && widget_width(app, window_id, plugin_detail_position_id())
                        .is_some_and(|width| width >= 280.),
                "Plugins page should keep a readable master list and detail pane"
            )
        },
    )
}

pub fn assert_plugins_empty_state() -> TestStep {
    TestStep::new("Assert Plugins empty state is rendered").add_named_assertion(
        "local package empty state is painted",
        |app, window_id| {
            let inventory_empty = app.read(|ctx| {
                WarpeerPluginHost::as_ref(ctx)
                    .inventory()
                    .is_ok_and(|plugins| plugins.is_empty())
            });
            async_assert!(
                inventory_empty && widget_painted(app, window_id, empty_plugin_widget_id()),
                "Plugins should render the local-package empty state"
            )
        },
    )
}

pub fn set_image_preview_setting(key: &'static str, value: Value) -> TestStep {
    dispatch_plugin_action(PluginsPageAction::SetSetting {
        plugin_id: image_preview_id(),
        key: key.into(),
        value,
    })
}

pub fn set_image_preview_permission(capability: Capability, enabled: bool) -> TestStep {
    dispatch_plugin_action(PluginsPageAction::SetPermission {
        plugin_id: image_preview_id(),
        capability,
        enabled,
    })
}

pub fn update_image_preview_direct(
    setting: Option<(&'static str, Value)>,
    permission: Option<(Capability, bool)>,
) -> TestStep {
    new_step_with_default_assertions("Update Image Preview host state").with_action(
        move |app, _, _| {
            WarpeerPluginHost::handle(app).update(app, |host, ctx| {
                let id = image_preview_id();
                if let Some((key, value)) = &setting {
                    let mut values = host
                        .inventory()
                        .expect("plugin inventory should load")
                        .into_iter()
                        .find(|plugin| plugin.manifest.id == id)
                        .expect("Image Preview should be installed")
                        .settings;
                    values.insert((*key).into(), value.clone());
                    host.update_settings(&id, values)
                        .expect("Image Preview setting should update");
                }
                if let Some((capability, enabled)) = &permission {
                    host.set_permission(&id, capability.clone(), *enabled)
                        .expect("Image Preview permission should update");
                }
                ctx.notify();
            });
        },
    )
}

pub fn set_image_preview_enabled_direct(enabled: bool) -> TestStep {
    new_step_with_default_assertions("Set Image Preview enablement").with_action(
        move |app, _, _| {
            WarpeerPluginHost::handle(app).update(app, |host, ctx| {
                host.set_enabled(&image_preview_id(), enabled)
                    .expect("Image Preview enablement should update");
                ctx.notify();
            });
        },
    )
}

pub fn recover_image_preview_direct() -> TestStep {
    new_step_with_default_assertions("Recover Image Preview directly").with_action(|app, _, _| {
        WarpeerPluginHost::handle(app).update(app, |host, ctx| {
            host.recover(&image_preview_id())
                .expect("Image Preview should recover");
            ctx.notify();
        });
    })
}

pub fn toggle_image_preview() -> TestStep {
    dispatch_plugin_action(PluginsPageAction::ToggleEnabled(image_preview_id()))
}

pub fn retry_image_preview() -> TestStep {
    dispatch_plugin_action(PluginsPageAction::Retry(image_preview_id()))
}

pub fn simulate_image_preview_crash(detail: &'static str) -> TestStep {
    new_step_with_default_assertions("Simulate an Image Preview worker crash").with_action(
        move |app, _, _| {
            WarpeerPluginHost::handle(app).update(app, |host, ctx| {
                host.integration_note_crash(&image_preview_id(), detail);
                ctx.notify();
            });
        },
    )
}

pub fn assert_image_preview_state(
    enabled: bool,
    state: WorkerState,
    grant: Option<(Capability, bool)>,
    setting: Option<(&'static str, Value)>,
    diagnostic: Option<&'static str>,
) -> TestStep {
    TestStep::new("Assert Image Preview lifecycle state").add_named_assertion(
        "plugin state is persisted and observable",
        move |app, _| {
            let expected_state = state.clone();
            let matches = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                let grant_matches = grant.as_ref().is_none_or(|(capability, expected)| {
                    host.integration_has_grant(&image_preview_id(), capability) == *expected
                });
                let setting_matches = setting.as_ref().is_none_or(|(key, expected)| {
                    host.integration_setting(&image_preview_id(), key).as_ref() == Some(expected)
                });
                let diagnostic_matches = diagnostic.is_none_or(|expected| {
                    host.diagnostics(&image_preview_id())
                        .iter()
                        .any(|entry| entry.message.contains(expected))
                });
                host.integration_enabled(&image_preview_id()) == Some(enabled)
                    && host.worker_state(&image_preview_id()) == Some(expected_state)
                    && grant_matches
                    && setting_matches
                    && diagnostic_matches
            });
            async_assert!(
                matches,
                "Image Preview lifecycle state did not reach the expected value"
            )
        },
    )
}

pub fn request_remove_image_preview() -> TestStep {
    dispatch_plugin_action(PluginsPageAction::RequestRemove(image_preview_id()))
}

pub fn cancel_remove_image_preview() -> TestStep {
    dispatch_plugin_action(PluginsPageAction::CancelRemove)
}

pub fn confirm_remove_image_preview(retain_data: bool) -> TestStep {
    dispatch_plugin_action(PluginsPageAction::ConfirmRemove { retain_data })
}

pub fn install_image_preview(path: PathBuf) -> TestStep {
    dispatch_plugin_action(PluginsPageAction::PackagePicked(Ok(path)))
}

pub fn confirm_install_image_preview() -> TestStep {
    dispatch_plugin_action(PluginsPageAction::ConfirmInstall)
}

pub fn assert_image_preview_source(source: PluginSource) -> TestStep {
    TestStep::new("Assert Image Preview install source").add_named_assertion(
        format!("Image Preview source is {source:?}"),
        move |app, _| {
            let actual = app
                .read(|ctx| WarpeerPluginHost::as_ref(ctx).integration_source(&image_preview_id()));
            async_assert!(
                actual == Some(source),
                "Image Preview source should be {source:?}"
            )
        },
    )
}

pub fn start_preview(probe: PreviewProbe) -> TestStep {
    new_step_with_default_assertions("Start an Image Preview hover").with_action(
        move |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                terminal.integration_start_plugin_hover(probe.context.clone(), ctx);
            });
        },
    )
}

pub fn assert_preview_ready(probe: PreviewProbe) -> TestStep {
    TestStep::new("Wait for Image Preview surface and frame")
        .set_timeout(Duration::from_secs(25))
        .add_named_assertion("surface frame is rendered through WarpUI", move |app, _| {
            let (ready, detail) = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                let snapshot = host.client().terminal_surface(&probe.context);
                let ready = snapshot
                    .as_ref()
                    .is_some_and(|surface| surface.frame.is_some())
                    && host.integration_surface_count() == 1
                    && host
                        .integration_worker_memory_bytes(&image_preview_id())
                        .is_some();
                let diagnostics = host
                    .diagnostics(&image_preview_id())
                    .into_iter()
                    .map(|entry| entry.message)
                    .collect::<Vec<_>>();
                let detail = format!(
                    "enabled={:?}, state={:?}, grant_read={}, surfaces={}, frame={}, connections={}, pending={}, worker_memory={:?}, diagnostics={diagnostics:?}",
                    host.integration_enabled(&image_preview_id()),
                    host.worker_state(&image_preview_id()),
                    host.integration_has_grant(&image_preview_id(), &Capability::ResourceRead),
                    host.integration_surface_count(),
                    snapshot.as_ref().is_some_and(|surface| surface.frame.is_some()),
                    host.integration_connection_count(),
                    host.integration_pending_request_count(),
                    host.integration_worker_memory_bytes(&image_preview_id()),
                );
                (ready, detail)
            });
            async_assert!(
                ready && live_external_image_bytes() > 0,
                "Image Preview should have a live worker, surface, frame, and external image: {detail}, external_image_bytes={}",
                live_external_image_bytes(),
            )
        })
}

pub fn assert_preview_surface_open(probe: PreviewProbe) -> TestStep {
    TestStep::new("Wait for Image Preview to open its surface")
        .set_timeout(Duration::from_secs(10))
        .add_named_assertion("worker and surface are ready for layout", move |app, _| {
            let open = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                host.client().terminal_surface(&probe.context).is_some()
                    && host.worker_state(&image_preview_id()) == Some(WorkerState::Running)
            });
            async_assert!(
                open,
                "Image Preview should open a surface from a live worker"
            )
        })
}

pub fn cancel_preview() -> TestStep {
    new_step_with_default_assertions("Cancel Image Preview hover").with_action(
        |app, window_id, _| {
            single_terminal_view(app, window_id).update(app, |terminal, ctx| {
                terminal.integration_cancel_plugin_hover(ctx);
            });
        },
    )
}

pub fn assert_preview_cancelled() -> TestStep {
    assert_preview_cancelled_in_state(WorkerState::Idle)
}

pub fn assert_crashed_preview_cancelled() -> TestStep {
    assert_preview_cancelled_in_state(WorkerState::Failed)
}

fn assert_preview_cancelled_in_state(expected_state: WorkerState) -> TestStep {
    TestStep::new("Assert Image Preview cancellation released its surface")
        .set_timeout(Duration::from_secs(10))
        .add_named_assertion(
            "hover, container, surface, and worker are gone",
            move |app, _| {
                let (released, detail) = app.read(|ctx| {
                    let host = WarpeerPluginHost::as_ref(ctx);
                    let state = host.worker_state(&image_preview_id());
                    let surfaces = host.integration_surface_count();
                    let containers = host.integration_container_count();
                    let contexts = host.integration_hover_context_count();
                    let connections = host.integration_connection_count();
                    let released = surfaces == 0
                        && containers == 0
                        && contexts == 0
                        && connections == 0
                        && state == Some(expected_state.clone());
                    (
                        released,
                        format!(
                            "state={state:?}, surfaces={surfaces}, containers={containers}, contexts={contexts}, connections={connections}"
                        ),
                    )
                });
                async_assert!(
                    released,
                    "cancelled preview should release its context, container, surface, and worker: {detail}"
                )
            },
        )
}

pub fn terminate_image_preview_worker() -> TestStep {
    new_step_with_default_assertions("Terminate the Image Preview worker").with_action(
        |app, _, _| {
            app.read(|ctx| {
                WarpeerPluginHost::as_ref(ctx)
                    .integration_terminate_worker(&image_preview_id())
                    .expect("Image Preview worker should be connected");
            });
        },
    )
}

pub fn reap_image_preview_worker() -> TestStep {
    new_step_with_default_assertions("Reap the idle Image Preview worker").with_action(
        |app, _, _| {
            WarpeerPluginHost::handle(app).update(app, |host, ctx| {
                host.integration_reap_idle();
                ctx.notify();
            });
        },
    )
}

pub fn assert_plugin_resources_idle() -> TestStep {
    TestStep::new("Assert plugin resources returned to idle")
        .set_timeout(Duration::from_secs(10))
        .add_named_assertion(
            "worker, IPC, surfaces, frames, and cache are idle",
            |app, _| {
                let idle = app.read(|ctx| {
                    let host = WarpeerPluginHost::as_ref(ctx);
                    host.integration_connection_count() == 0
                        && host.integration_pending_request_count() == 0
                        && host.integration_surface_count() == 0
                        && host.integration_hover_context_count() == 0
                        && host.integration_preview_cache_bytes(&image_preview_id()) == 0
                });
                async_assert!(
                    idle && live_external_image_bytes() == 0,
                    "all plugin resources should return to the idle baseline"
                )
            },
        )
}

pub fn notify_terminal_frame() -> TestStep {
    TestStep::new("Render an idle terminal frame").with_action(|app, window_id, _| {
        single_terminal_view(app, window_id).update(app, |_, ctx| ctx.notify());
    })
}

pub fn capture_app_memory(probe: MemoryProbe) -> TestStep {
    TestStep::new("Capture Warpeer application memory").with_action(move |_, _, _| {
        probe.store(current_process_memory());
    })
}

pub fn assert_app_memory_plateau(probe: MemoryProbe, tolerance_bytes: u64) -> TestStep {
    TestStep::new("Assert Warpeer memory reached a bounded plateau").add_named_assertion(
        "application RSS remains near the warm baseline",
        move |_, _| {
            let current = current_process_memory();
            let baseline = probe.load();
            async_assert!(
                baseline > 0 && current <= baseline.saturating_add(tolerance_bytes),
                "Warpeer RSS grew beyond the preview-cycle plateau: baseline={baseline}, current={current}, tolerance={tolerance_bytes}"
            )
        },
    )
}

fn current_process_memory() -> u64 {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system.process(pid).map_or(0, |process| process.memory())
}

#[cfg(target_os = "macos")]
fn live_external_image_bytes() -> u64 {
    warpui::image_cache::live_external_image_bytes()
}

#[cfg(not(target_os = "macos"))]
fn live_external_image_bytes() -> u64 {
    0
}
