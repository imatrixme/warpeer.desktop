use std::time::{Duration, Instant};

use warpeer_plugin_protocol::{
    AppearanceKind, Capability, ChromeActionSource, ContainerId, ContextId, DiagnosticLevel,
    HostToPlugin, PageId, PaneContext, PluginId, PluginSettingsContext, Revision, SurfaceBounds,
    SurfaceContainerMetrics, SurfaceId, SurfaceInput, SurfacePalette, SurfacePoint, SurfaceSlot,
    SurfaceTarget, TerminalHoverContext,
};
use warpeer_plugin_runtime::InstalledPlugin;

use super::PresentedContainer;
use super::host::{HostCore, PluginHostClient, terminal_trigger_enabled};
use super::surface_store::SurfaceSnapshot;

const PANE_SLOTS: [SurfaceSlot; 4] = [
    SurfaceSlot::PaneBackground,
    SurfaceSlot::PaneOverlay,
    SurfaceSlot::PaneHeader,
    SurfaceSlot::PanePopover,
];

impl PluginHostClient {
    pub fn terminal_surface(&self, context: &TerminalHoverContext) -> Option<SurfaceSnapshot> {
        self.core.surfaces.for_target(
            SurfaceSlot::TerminalHover,
            &SurfaceTarget::TerminalContext {
                context_id: context.context_id,
                pane_id: context.pane_id.clone(),
            },
            context.revision,
        )
    }

    pub fn terminal_container(&self, context: &TerminalHoverContext) -> Option<PresentedContainer> {
        self.core.container_for_context(
            context.context_id,
            context.revision,
            SurfaceSlot::TerminalHover,
        )
    }

    pub fn move_container(&self, id: ContainerId, origin: SurfacePoint) {
        self.core.move_container(id, origin);
    }

    pub fn set_container_drag_capture(&self, id: ContainerId, captured: bool) {
        self.core.set_container_drag_capture(id, captured);
    }

    pub fn container_action(
        &self,
        id: ContainerId,
        page_id: PageId,
        revision: Revision,
        action_id: String,
        menu_item_id: Option<String>,
        source: ChromeActionSource,
    ) -> bool {
        self.core
            .activate_container_action(id, page_id, revision, action_id, menu_item_id, source)
    }

    pub fn pane_context(&self, context: PaneContext) {
        self.core.dispatch_pane_context(context);
    }

    pub fn plugin_settings_context(&self, context: PluginSettingsContext) {
        self.core.dispatch_settings_context(context);
    }

    pub fn cancel_context(&self, context_id: ContextId, revision: Revision) {
        self.core.pane_contexts.lock().remove(&context_id);
        self.core.settings_contexts.lock().remove(&context_id);
        self.core.cancel_surface_context(context_id, revision);
    }

    pub fn pane_surfaces(&self, pane_id: &str) -> Vec<SurfaceSnapshot> {
        self.core.surfaces.for_pane(pane_id)
    }

    pub fn settings_surfaces(&self, context_id: ContextId) -> Vec<SurfaceSnapshot> {
        self.core.surfaces.for_settings(context_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present_surface(
        &self,
        id: SurfaceId,
        bounds: SurfaceBounds,
        container: SurfaceContainerMetrics,
        appearance: AppearanceKind,
        palette: SurfacePalette,
        reduced_motion: bool,
        focused: bool,
        visible: bool,
        occluded: bool,
    ) {
        let Some(delivery) = self.core.surfaces.present(
            id,
            bounds,
            container,
            appearance,
            palette,
            reduced_motion,
            focused,
            visible,
            occluded,
            Instant::now(),
        ) else {
            return;
        };
        self.core.diagnostic(
            &delivery.plugin_id,
            DiagnosticLevel::Info,
            "surface presentation environment delivered",
        );
        let _ = self.core.launcher.send(
            &delivery.plugin_id,
            HostToPlugin::SurfaceEnvironment(delivery.environment),
        );
    }

    pub fn surface_input(&self, id: SurfaceId, input: SurfaceInput) -> bool {
        let Some((plugin_id, descriptor, environment)) = self.core.surfaces.owner(id) else {
            return false;
        };
        if !input_allowed(&descriptor, environment.as_ref(), &input) {
            return false;
        }
        self.core
            .launcher
            .try_send(
                &plugin_id,
                HostToPlugin::SurfaceInput {
                    surface_id: id,
                    input,
                },
            )
            .is_ok()
    }
}

impl HostCore {
    pub(super) fn activate_plugin_contexts(&self, plugin: &InstalledPlugin) {
        if is_pane_surface_plugin(plugin) {
            for context in self.pane_contexts.lock().values().cloned() {
                self.trigger_plugin(plugin, HostToPlugin::PaneContext(context));
            }
        }
        if plugin.enabled
            && plugin
                .manifest
                .surfaces
                .contains(&SurfaceSlot::PluginSettings)
            && plugin.grants.contains_key(&Capability::PaneSurface)
        {
            for context in self
                .settings_contexts
                .lock()
                .values()
                .filter(|context| context.plugin_id == plugin.manifest.id)
                .cloned()
            {
                self.trigger_plugin(plugin, HostToPlugin::PluginSettingsContext(context));
            }
        }
        if plugin.enabled
            && plugin.grants.contains_key(&Capability::TerminalContext)
            && plugin
                .manifest
                .surfaces
                .contains(&SurfaceSlot::TerminalHover)
        {
            for context in self.hover_contexts.lock().values().filter(|context| {
                let target = context
                    .resolved_resource
                    .as_deref()
                    .unwrap_or(&context.resource_text);
                plugin.manifest.accepts_terminal_hover(target)
                    && terminal_trigger_enabled(plugin, context.explicit_preview)
            }) {
                self.trigger_plugin(plugin, HostToPlugin::TerminalHover(context.clone()));
            }
        }
    }

    pub(super) fn surface_target_active(
        &self,
        plugin_id: &PluginId,
        target: &SurfaceTarget,
        revision: Revision,
    ) -> bool {
        match target {
            SurfaceTarget::TerminalContext {
                context_id,
                pane_id,
            } => self
                .hover_contexts
                .lock()
                .get(context_id)
                .is_some_and(|context| context.revision == revision && &context.pane_id == pane_id),
            SurfaceTarget::Pane {
                context_id,
                pane_id,
            } => self
                .pane_contexts
                .lock()
                .get(context_id)
                .is_some_and(|context| context.revision == revision && &context.pane_id == pane_id),
            SurfaceTarget::PluginSettings { context_id } => self
                .settings_contexts
                .lock()
                .get(context_id)
                .is_some_and(|context| {
                    context.revision == revision && &context.plugin_id == plugin_id
                }),
        }
    }

    pub(super) fn cancel_surface_context(&self, context_id: ContextId, revision: Revision) {
        self.containers.remove_context(context_id);
        for plugin_id in self.surfaces.remove_context(context_id) {
            self.supervisor.release_surface(&plugin_id);
        }
        for pending in self
            .pending
            .lock()
            .values()
            .filter(|pending| pending.context_id == Some(context_id))
        {
            pending.cancellation.cancel();
        }
        if let Ok(plugins) = self.manager.inventory() {
            for plugin in plugins.into_iter().filter(|plugin| plugin.enabled) {
                let _ = self.launcher.send(
                    &plugin.manifest.id,
                    HostToPlugin::CancelContext {
                        context_id,
                        revision,
                    },
                );
            }
        }
    }

    pub(super) fn occlude_unpresented_surfaces(&self, now: Instant, timeout: Duration) {
        for delivery in self.surfaces.occlude_unpresented(now, timeout) {
            let _ = self.launcher.try_send(
                &delivery.plugin_id,
                HostToPlugin::SurfaceEnvironment(delivery.environment),
            );
        }
    }

    fn dispatch_pane_context(&self, context: PaneContext) {
        self.pane_contexts
            .lock()
            .insert(context.context_id, context.clone());
        let Ok(plugins) = self.manager.inventory() else {
            return;
        };
        for plugin in plugins.into_iter().filter(is_pane_surface_plugin) {
            self.trigger_plugin(&plugin, HostToPlugin::PaneContext(context.clone()));
        }
    }

    fn dispatch_settings_context(&self, context: PluginSettingsContext) {
        self.settings_contexts
            .lock()
            .insert(context.context_id, context.clone());
        let Ok(Some(plugin)) = self.manager.installed(&context.plugin_id) else {
            return;
        };
        if plugin.enabled
            && plugin
                .manifest
                .surfaces
                .contains(&SurfaceSlot::PluginSettings)
            && plugin.grants.contains_key(&Capability::PaneSurface)
        {
            self.trigger_plugin(&plugin, HostToPlugin::PluginSettingsContext(context));
        }
    }

    fn trigger_plugin(&self, plugin: &InstalledPlugin, message: HostToPlugin) {
        let Ok(triggered) = self.supervisor.trigger(&plugin.manifest.id) else {
            return;
        };
        if triggered.launched {
            let start = match self.start_message(plugin, triggered.worker_id) {
                Ok(start) => start,
                Err(error) => {
                    self.supervisor.note_crash(&plugin.manifest.id, error);
                    return;
                }
            };
            if let Err(error) = self.launcher.send(&plugin.manifest.id, start) {
                self.supervisor.note_crash(&plugin.manifest.id, error);
                return;
            }
            self.send_configuration(plugin);
        }
        let _ = self.launcher.send(&plugin.manifest.id, message);
    }
}

fn is_pane_surface_plugin(plugin: &InstalledPlugin) -> bool {
    plugin.enabled
        && plugin.grants.contains_key(&Capability::PaneSurface)
        && plugin.grants.contains_key(&Capability::PaneMetadata)
        && PANE_SLOTS
            .iter()
            .any(|slot| plugin.manifest.surfaces.contains(slot))
}

fn input_allowed(
    descriptor: &warpeer_plugin_protocol::SurfaceDescriptor,
    environment: Option<&warpeer_plugin_protocol::SurfaceEnvironment>,
    input: &SurfaceInput,
) -> bool {
    match input {
        SurfaceInput::PointerMove { point }
        | SurfaceInput::PointerDown { point, .. }
        | SurfaceInput::PointerUp { point, .. } => {
            let Some(environment) = environment else {
                return false;
            };
            descriptor.accepts_pointer
                && point.x >= 0.
                && point.y >= 0.
                && point.x < environment.bounds.width
                && point.y < environment.bounds.height
        }
        SurfaceInput::Scroll { .. } => descriptor.accepts_pointer,
        SurfaceInput::Key { .. } | SurfaceInput::Text { .. } | SurfaceInput::Focus { .. } => {
            descriptor.accepts_keyboard
                && matches!(
                    descriptor.slot,
                    SurfaceSlot::PanePopover | SurfaceSlot::PluginSettings
                )
        }
        SurfaceInput::Dismiss => true,
    }
}

#[cfg(test)]
mod tests {
    use warpeer_plugin_protocol::{SurfaceDescriptor, SurfaceEnvironment, SurfacePoint};

    use super::*;

    fn descriptor(slot: SurfaceSlot) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SurfaceId::new(),
            slot,
            target: SurfaceTarget::Pane {
                context_id: ContextId::new(),
                pane_id: "pane".into(),
            },
            revision: Revision(1),
            bounds: SurfaceBounds {
                x: 10.,
                y: 20.,
                width: 100.,
                height: 60.,
                scale: 1.,
            },
            accessibility_label: "Plugin surface".into(),
            accepts_pointer: true,
            accepts_keyboard: true,
        }
    }

    #[test]
    fn clipped_pointer_input_never_leaks_outside_surface() {
        let descriptor = descriptor(SurfaceSlot::PaneOverlay);
        let environment = SurfaceEnvironment {
            surface_id: descriptor.id,
            revision: Revision(1),
            bounds: descriptor.bounds,
            container: SurfaceContainerMetrics::default(),
            appearance: AppearanceKind::Dark,
            palette: SurfacePalette::dark(),
            reduced_motion: false,
            focused: true,
            visible: true,
            occluded: false,
        };
        assert!(input_allowed(
            &descriptor,
            Some(&environment),
            &SurfaceInput::PointerMove {
                point: SurfacePoint { x: 99., y: 59. }
            }
        ));
        assert!(!input_allowed(
            &descriptor,
            Some(&environment),
            &SurfaceInput::PointerMove {
                point: SurfacePoint { x: 100., y: 59. }
            }
        ));
        assert!(!input_allowed(
            &descriptor,
            Some(&environment),
            &SurfaceInput::PointerDown {
                point: SurfacePoint { x: -1., y: 0. },
                button: 0,
            }
        ));
    }

    #[test]
    fn terminal_authority_blocks_keyboard_for_non_popover_slots() {
        let overlay = descriptor(SurfaceSlot::PaneOverlay);
        let popover = descriptor(SurfaceSlot::PanePopover);
        let key = SurfaceInput::Key {
            key: "a".into(),
            modifiers: 0,
            down: true,
        };
        assert!(!input_allowed(&overlay, None, &key));
        assert!(input_allowed(&popover, None, &key));
    }
}
