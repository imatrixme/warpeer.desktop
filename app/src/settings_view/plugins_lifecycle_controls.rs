use warpeer_plugin_runtime::InstallReview;
use warpui::AppContext;
use warpui::elements::{
    Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, ParentElement,
};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};

use super::plugins_page::{PluginsPageAction, PluginsSettingsPageView};
use super::settings_page::{LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item};
use crate::appearance::Appearance;

pub fn install_review_widget(
    review: &InstallReview,
) -> Box<dyn SettingsWidget<View = PluginsSettingsPageView>> {
    Box::new(InstallReviewWidget::new(review))
}

pub fn remove_review_widget(
    name: String,
) -> Box<dyn SettingsWidget<View = PluginsSettingsPageView>> {
    Box::new(RemoveReviewWidget::new(name))
}

fn button(
    appearance: &Appearance,
    label: &str,
    state: MouseStateHandle,
    action: PluginsPageAction,
) -> Box<dyn Element> {
    Container::new(
        appearance
            .ui_builder()
            .button(ButtonVariant::Secondary, state)
            .with_text_label(label.to_string())
            .with_style(UiComponentStyles {
                padding: Some(Coords::default().top(5.).bottom(5.).left(10.).right(10.)),
                ..Default::default()
            })
            .build()
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
            .finish(),
    )
    .with_margin_left(8.)
    .finish()
}

struct InstallReviewWidget {
    summary: String,
    confirm: MouseStateHandle,
    cancel: MouseStateHandle,
}

impl InstallReviewWidget {
    fn new(review: &InstallReview) -> Self {
        let package = &review.package.manifest;
        let permissions = package
            .capabilities
            .iter()
            .map(|capability| format!("{:?}", capability.capability))
            .collect::<Vec<_>>()
            .join(", ");
        let mut warnings = Vec::new();
        if review.downgrade {
            warnings.push("This is a version downgrade.");
        }
        if !review.expanded_capabilities.is_empty() {
            warnings.push("This update expands plugin permissions.");
        }
        let warning = if warnings.is_empty() {
            String::new()
        } else {
            format!(" {}", warnings.join(" "))
        };
        Self {
            summary: format!(
                "{} {} by {}. Surfaces: {}. Permissions: {}.{warning}",
                package.name,
                package.version,
                package
                    .publisher
                    .as_ref()
                    .map_or("Unknown publisher", |publisher| publisher.name.as_str()),
                package
                    .surfaces
                    .iter()
                    .map(|surface| format!("{surface:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                permissions,
            ),
            confirm: Default::default(),
            cancel: Default::default(),
        }
    }
}

impl SettingsWidget for InstallReviewWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin install permission review downgrade publisher"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let controls = Flex::row()
            .with_child(button(
                appearance,
                "Cancel",
                self.cancel.clone(),
                PluginsPageAction::CancelInstall,
            ))
            .with_child(button(
                appearance,
                "Install",
                self.confirm.clone(),
                PluginsPageAction::ConfirmInstall,
            ))
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .finish();
        render_body_item::<PluginsPageAction>(
            "Approve plugin package".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            controls,
            Some(self.summary.clone()),
        )
    }
}

struct RemoveReviewWidget {
    name: String,
    delete: MouseStateHandle,
    retain: MouseStateHandle,
    cancel: MouseStateHandle,
}

impl RemoveReviewWidget {
    fn new(name: String) -> Self {
        Self {
            name,
            delete: Default::default(),
            retain: Default::default(),
            cancel: Default::default(),
        }
    }
}

impl SettingsWidget for RemoveReviewWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin uninstall remove delete retain data"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let controls = Flex::row()
            .with_child(button(
                appearance,
                "Cancel",
                self.cancel.clone(),
                PluginsPageAction::CancelRemove,
            ))
            .with_child(button(
                appearance,
                "Keep data",
                self.retain.clone(),
                PluginsPageAction::ConfirmRemove { retain_data: true },
            ))
            .with_child(button(
                appearance,
                "Delete data",
                self.delete.clone(),
                PluginsPageAction::ConfirmRemove { retain_data: false },
            ))
            .finish();
        render_body_item::<PluginsPageAction>(
            format!("Remove {}?", self.name),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            controls,
            Some("The worker stops before its package is removed. Private plugin data can be kept for a later reinstall.".into()),
        )
    }
}
