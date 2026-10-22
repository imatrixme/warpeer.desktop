//! Running Warpeer coverage for the PocketJS plugin host and Image Preview.

use std::path::PathBuf;

use serde_json::json;
use warp::integration_testing::settings::{
    assert_settings_section, open_settings_page, press_settings_nav_down,
};
use warp::integration_testing::terminal::util::{ExactLine, ExpectedExitStatus};
use warp::integration_testing::terminal::{
    execute_command_for_single_terminal_in_tab, wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::warpeer_plugin_hover::*;
use warp::integration_testing::warpeer_plugin_lifecycle::*;
use warp::integration_testing::warpeer_plugins::*;
use warp::settings_view::SettingsSection;
use warpeer_plugin_protocol::{Capability, WorkerState};
use warpeer_plugin_runtime::PluginSource;
use warpui_core::integration::TestStep;

use super::{Builder, new_builder};

const ONE_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xff, 0x1f, 0x00,
    0x02, 0xeb, 0x01, 0xf5, 0x16, 0x15, 0x14, 0x76, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

fn test_image_path() -> PathBuf {
    std::env::temp_dir().join("warpeer-image-preview-integration.png")
}

fn plugin_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/image-preview/dist/image-preview.warpeerplugin")
}

fn create_test_image() -> TestStep {
    TestStep::new("Create the local preview fixture").with_action(|_, _, _| {
        std::fs::write(test_image_path(), ONE_PIXEL_PNG)
            .expect("local preview fixture should be written");
    })
}

/// Proves the top-level native Settings experience and its complete lifecycle
/// controls in the real Warpeer window.
pub fn test_warpeer_plugins_settings_lifecycle() -> Builder {
    let reloaded = WorkerIdentityProbe::default();
    let force_restarted = WorkerIdentityProbe::default();
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(open_settings_page(SettingsSection::TerminalEffects))
        // The native sidebar's keyboard order places Plugins directly after
        // Terminal effects in the Warpeer profile.
        .with_step(press_settings_nav_down())
        .with_step(assert_settings_section(SettingsSection::Plugins))
        .with_step(assert_plugins_page_ready())
        .with_step(set_image_preview_setting("hover_delay_ms", json!(1500)))
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Idle,
            None,
            Some(("hover_delay_ms", json!(1500))),
            None,
        ))
        .with_step(capture_image_preview_worker(reloaded.clone()))
        .with_step(reload_image_preview())
        .with_step(assert_image_preview_worker_replaced(reloaded))
        .with_step(capture_image_preview_worker(force_restarted.clone()))
        .with_step(force_restart_image_preview())
        .with_step(assert_image_preview_worker_replaced(force_restarted))
        .with_step(set_image_preview_permission(Capability::NetworkFetch, true))
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Running,
            Some((Capability::NetworkFetch, true)),
            None,
            None,
        ))
        .with_step(set_image_preview_permission(
            Capability::NetworkFetch,
            false,
        ))
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Running,
            Some((Capability::NetworkFetch, false)),
            None,
            None,
        ))
        .with_step(toggle_image_preview())
        .with_step(assert_image_preview_state(
            false,
            WorkerState::Stopped,
            None,
            None,
            None,
        ))
        .with_step(assert_plugin_resources_idle())
        .with_step(toggle_image_preview())
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Idle,
            None,
            None,
            None,
        ))
        .with_step(simulate_image_preview_crash(
            "integration worker crash at /Users/private/source.png",
        ))
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Failed,
            None,
            None,
            Some("integration worker crash"),
        ))
        .with_step(retry_image_preview())
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Idle,
            None,
            None,
            Some("plugin worker recovered"),
        ))
        .with_step(request_remove_image_preview())
        .with_step(cancel_remove_image_preview())
        .with_step(assert_image_preview_source(PluginSource::Bundled))
        .with_step(request_remove_image_preview())
        .with_step(confirm_remove_image_preview(false))
        .with_step(assert_plugins_empty_state())
        .with_step(install_image_preview(plugin_package_path()))
        .with_step(confirm_install_image_preview())
        .with_step(assert_image_preview_source(PluginSource::Local))
        .with_step(assert_plugins_page_ready())
        .with_step(request_remove_image_preview())
        .with_step(confirm_remove_image_preview(false))
        .with_step(assert_plugins_empty_state())
}

/// Runs local and HTTPS previews through the real worker and renderer, then
/// kills the worker to prove diagnostics and recovery use the same path as a
/// production crash.
pub fn test_warpeer_image_preview_end_to_end() -> Builder {
    let local = PreviewProbe::local(test_image_path());
    let remote = PreviewProbe::remote("https://httpbin.org/image/png");
    let after_recovery = PreviewProbe::local(test_image_path());
    let mut builder = new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(create_test_image())
        .with_step(update_image_preview_direct(
            Some(("trigger_mode", json!("Hover"))),
            None,
        ))
        .with_step(update_image_preview_direct(
            Some(("hover_delay_ms", json!(1500))),
            Some((Capability::NetworkFetch, true)),
        ))
        .with_step(start_preview(local.clone()))
        .with_step(assert_preview_surface_open(local.clone()))
        .with_step(assert_preview_ready(local))
        .with_step(cancel_preview())
        .with_step(assert_preview_cancelled())
        .with_step(start_preview(remote.clone()))
        .with_step(assert_preview_surface_open(remote.clone()))
        .with_step(assert_preview_ready(remote))
        .with_step(cancel_preview())
        .with_step(assert_preview_cancelled())
        .with_step(start_preview(after_recovery.clone()))
        .with_step(assert_preview_surface_open(after_recovery.clone()))
        .with_step(assert_preview_ready(after_recovery.clone()))
        .with_step(terminate_image_preview_worker())
        .with_step(assert_image_preview_state(
            true,
            WorkerState::Failed,
            None,
            None,
            Some("exited unexpectedly"),
        ))
        .with_step(cancel_preview())
        .with_step(assert_crashed_preview_cancelled())
        .with_step(recover_image_preview_direct())
        .with_step(start_preview(after_recovery.clone()))
        .with_step(assert_preview_surface_open(after_recovery.clone()))
        .with_step(assert_preview_ready(after_recovery))
        .with_step(cancel_preview())
        .with_step(assert_preview_cancelled())
        .with_step(set_image_preview_enabled_direct(false));
    for _ in 0..12 {
        builder = builder.with_step(notify_terminal_frame());
    }
    builder.with_step(assert_plugin_resources_idle())
}

/// Covers the real terminal path: `ls` output, async file scanning, pointer anchoring,
/// PocketJS rendering, and delayed hover dismissal.
pub fn test_warpeer_image_preview_from_ls_hover() -> Builder {
    let path = test_image_path();
    let pointer = [320., 240.];
    let mut builder = new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(create_test_image())
        .with_step(update_image_preview_direct(
            Some(("trigger_mode", json!("Hover"))),
            None,
        ))
        .with_step(update_image_preview_direct(
            Some(("hover_delay_ms", json!(1500))),
            None,
        ))
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            format!("ls -1 '{}'", path.display()),
            ExpectedExitStatus::Success,
            ExactLine::from(path.to_string_lossy().into_owned()),
        ))
        .with_step(hover_last_block_image_output(pointer))
        .with_step(assert_detected_preview_ready(path.clone(), pointer))
        .with_step(pin_current_image_preview())
        .with_step(assert_pinned_preview_ready(path, pointer))
        .with_step(move_pinned_image_preview([72.0, 54.0]))
        .with_step(assert_pinned_image_page(
            "Image Preview",
            1,
            1,
            [72.0, 54.0],
        ))
        .with_step(leave_image_output_hover());
    for _ in 0..70 {
        builder = builder.with_step(notify_terminal_frame());
    }
    builder
        .with_step(assert_pinned_preview_retained())
        .with_step(close_pinned_image_preview())
        .with_step(assert_preview_cancelled())
        .with_step(set_image_preview_enabled_direct(false))
}

/// Repeatedly creates and destroys actual plugin surfaces. A warm cycle is the
/// baseline so allocator high-water behavior does not masquerade as a leak.
pub fn test_warpeer_image_preview_resource_plateau() -> Builder {
    let path = test_image_path();
    let warm = PreviewProbe::local(&path);
    let memory = MemoryProbe::default();
    let mut builder = new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(create_test_image())
        .with_step(update_image_preview_direct(
            Some(("trigger_mode", json!("Hover"))),
            None,
        ))
        .with_step(update_image_preview_direct(
            Some(("hover_delay_ms", json!(1500))),
            None,
        ))
        .with_step(start_preview(warm.clone()))
        .with_step(assert_preview_surface_open(warm.clone()))
        .with_step(assert_preview_ready(warm))
        .with_step(cancel_preview())
        .with_step(assert_preview_cancelled())
        .with_step(capture_app_memory(memory.clone()));

    for _ in 0..12 {
        let probe = PreviewProbe::local(&path);
        builder = builder
            .with_step(start_preview(probe.clone()))
            .with_step(assert_preview_surface_open(probe.clone()))
            .with_step(assert_preview_ready(probe))
            .with_step(cancel_preview())
            .with_step(assert_preview_cancelled());
    }

    builder = builder
        .with_step(set_image_preview_enabled_direct(false))
        .with_step(reap_image_preview_worker());
    for _ in 0..12 {
        builder = builder.with_step(notify_terminal_frame());
    }
    builder
        .with_step(assert_plugin_resources_idle())
        .with_step(assert_app_memory_plateau(memory, 24 * 1024 * 1024))
}
