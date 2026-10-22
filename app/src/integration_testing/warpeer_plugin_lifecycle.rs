use std::sync::{Arc, Mutex};

use warpeer_plugin_protocol::{PluginId, WorkerId, WorkerState};
use warpui::integration::TestStep;
use warpui::{App, SingletonEntity as _, ViewHandle, WindowId, async_assert};

use super::step::new_step_with_default_assertions;
use crate::settings_view::{PluginsPageAction, PluginsSettingsPageView};
use crate::warpeer_plugin::WarpeerPluginHost;

const IMAGE_PREVIEW_ID: &str = "dev.warpeer.image-preview";

#[derive(Clone, Default)]
pub struct WorkerIdentityProbe(Arc<Mutex<Option<WorkerId>>>);

fn image_preview_id() -> PluginId {
    PluginId::parse(IMAGE_PREVIEW_ID).expect("image preview plugin id is valid")
}

fn plugins_page(app: &App, window_id: WindowId) -> ViewHandle<PluginsSettingsPageView> {
    app.views_of_type::<PluginsSettingsPageView>(window_id)
        .and_then(|views| views.first().cloned())
        .expect("Plugins settings page should be mounted")
}

fn dispatch(action: PluginsPageAction) -> TestStep {
    new_step_with_default_assertions(&format!("Dispatch plugin lifecycle action: {action:?}"))
        .with_action(move |app, window_id, _| {
            let page = plugins_page(app, window_id);
            app.dispatch_typed_action(window_id, &[page.id()], &action);
        })
}

pub fn capture_image_preview_worker(probe: WorkerIdentityProbe) -> TestStep {
    TestStep::new("Capture the Image Preview worker identity").with_action(move |app, _, _| {
        let worker_id = app
            .read(|ctx| WarpeerPluginHost::as_ref(ctx).integration_worker_id(&image_preview_id()));
        *probe.0.lock().unwrap() = worker_id;
    })
}

pub fn reload_image_preview() -> TestStep {
    dispatch(PluginsPageAction::Reload(image_preview_id()))
}

pub fn force_restart_image_preview() -> TestStep {
    dispatch(PluginsPageAction::ForceRestart(image_preview_id()))
}

pub fn assert_image_preview_worker_replaced(probe: WorkerIdentityProbe) -> TestStep {
    TestStep::new("Assert Image Preview replaced its complete runtime").add_named_assertion(
        "worker identity and process are fresh",
        move |app, _| {
            let previous = *probe.0.lock().unwrap();
            let replaced = app.read(|ctx| {
                let host = WarpeerPluginHost::as_ref(ctx);
                let current = host.integration_worker_id(&image_preview_id());
                current.is_some()
                    && current != previous
                    && host.integration_connection_count() == 1
                    && host.worker_state(&image_preview_id()) == Some(WorkerState::Running)
            });
            async_assert!(replaced, "plugin worker should be fully replaced and ready")
        },
    )
}
