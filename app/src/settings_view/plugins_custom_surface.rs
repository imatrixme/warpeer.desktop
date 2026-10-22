use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use warpeer_plugin_protocol::{ContextId, PluginSettingsContext, Revision, SurfaceId, SurfaceSlot};
use warpeer_plugin_runtime::InstalledPlugin;
use warpui::elements::{Clipped, ConstrainedBox, Container, Element, Padding, Text};
use warpui::{AppContext, SingletonEntity as _, ViewContext};

use super::plugins_page::PluginsSettingsPageView;
use super::settings_page::SettingsWidget;
use crate::appearance::Appearance;
use crate::warpeer_plugin::{
    PluginHostClient, PluginSurfacePresentation, SurfacePresentationOptions, WarpeerPluginHost,
};

static NEXT_SETTINGS_REVISION: AtomicU64 = AtomicU64::new(1);

pub fn build_custom_surfaces(
    inventory: &[InstalledPlugin],
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>> {
    inventory
        .iter()
        .filter(|plugin| {
            plugin.enabled
                && plugin
                    .manifest
                    .surfaces
                    .contains(&SurfaceSlot::PluginSettings)
        })
        .cloned()
        .map(|plugin| {
            Box::new(PluginCustomSurfaceWidget::new(plugin, ctx))
                as Box<dyn SettingsWidget<View = PluginsSettingsPageView>>
        })
        .collect()
}

struct PluginCustomSurfaceWidget {
    plugin_name: String,
    client: PluginHostClient,
    context: PluginSettingsContext,
    presentations: RefCell<HashMap<SurfaceId, PluginSurfacePresentation>>,
}

impl PluginCustomSurfaceWidget {
    fn new(plugin: InstalledPlugin, app: &AppContext) -> Self {
        let client = WarpeerPluginHost::as_ref(app).client();
        let context = PluginSettingsContext {
            context_id: ContextId::new(),
            revision: Revision(NEXT_SETTINGS_REVISION.fetch_add(1, Ordering::Relaxed)),
            plugin_id: plugin.manifest.id,
        };
        client.plugin_settings_context(context.clone());
        Self {
            plugin_name: plugin.manifest.name,
            client,
            context,
            presentations: RefCell::default(),
        }
    }
}

impl SettingsWidget for PluginCustomSurfaceWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin custom interface pocketjs settings"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let Some(snapshot) = self
            .client
            .settings_surfaces(self.context.context_id)
            .into_iter()
            .find(|surface| surface.descriptor.slot == SurfaceSlot::PluginSettings)
        else {
            self.presentations.borrow_mut().clear();
            return Container::new(
                Text::new(
                    format!("{} custom interface is unavailable.", self.plugin_name),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(appearance.theme().foreground().into())
                .finish(),
            )
            .with_padding(Padding::uniform(12.))
            .finish();
        };
        let height = snapshot.descriptor.bounds.height.clamp(120., 360.);
        let mut presentations = self.presentations.borrow_mut();
        let presentation = presentations
            .entry(snapshot.descriptor.id)
            .or_insert_with(|| PluginSurfacePresentation::new(self.client.clone()));
        Container::new(
            Clipped::new(
                ConstrainedBox::new(presentation.element(
                    snapshot,
                    SurfacePresentationOptions::standard(
                        app,
                        true,
                        warpui::elements::CornerRadius::with_all(warpui::elements::Radius::Pixels(
                            8.,
                        )),
                    ),
                ))
                .with_height(height)
                .finish(),
            )
            .finish(),
        )
        .with_padding(Padding::uniform(8.))
        .finish()
    }
}

impl Drop for PluginCustomSurfaceWidget {
    fn drop(&mut self) {
        self.client
            .cancel_context(self.context.context_id, self.context.revision);
    }
}
