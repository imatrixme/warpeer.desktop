use warp_core::ui::color::coloru_with_opacity;
use warpui::elements::{Element, Empty, Fill as ElementFill};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{AppContext, SingletonEntity as _};

use super::Workspace;
use crate::appearance::Appearance;
use crate::tab_privacy::{TabPrivacyCredentialModel, TabPrivacyRuntime, TabPrivacyState};
use crate::ui_components::buttons::icon_button_with_color;
use crate::ui_components::icons;
use crate::workspace::WorkspaceAction;

const ICON_PADDING: f32 = 5.;

impl Workspace {
    pub(in crate::workspace::view) fn render_tab_privacy_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        if !TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
            return Empty::new().finish();
        }
        let tab_id = self.active_tab_pane_group().id();
        let supports_privacy = self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(|tab| tab.supports_privacy(ctx));
        let state = supports_privacy
            .then(|| TabPrivacyRuntime::as_ref(ctx).state(tab_id))
            .flatten();
        let index = self.active_tab_index;
        let (label, icon, color, action) = match state {
            None => (
                if supports_privacy {
                    "Protect tab"
                } else {
                    "Tab privacy is unavailable in Settings"
                },
                icons::Icon::Lock,
                appearance
                    .theme()
                    .sub_text_color(appearance.theme().background()),
                WorkspaceAction::ProtectAndLockTab(index),
            ),
            Some(TabPrivacyState::Locked | TabPrivacyState::Authenticating { .. }) => (
                "Unlock tab",
                icons::Icon::LockClosed,
                coloru_with_opacity(appearance.theme().ui_error_color(), 180).into(),
                WorkspaceAction::UnlockTab(index),
            ),
            Some(TabPrivacyState::UnlockedViewed | TabPrivacyState::UnlockedAway { .. }) => (
                "Lock tab",
                icons::Icon::Lock,
                appearance.theme().ui_green_color().into(),
                WorkspaceAction::LockTab(index),
            ),
        };
        let style = UiComponentStyles {
            background: Some(ElementFill::None),
            font_color: Some(color.into_solid()),
            ..UiComponentStyles::default()
        };
        let button = icon_button_with_color(
            appearance,
            icon,
            false,
            self.mouse_states.tab_privacy.clone(),
            color,
        )
        .with_style(UiComponentStyles {
            padding: Some(Coords::uniform(ICON_PADDING)),
            ..UiComponentStyles::default()
        })
        .with_hovered_styles(style)
        .with_clicked_styles(style);
        if !supports_privacy {
            return button
                .with_disabled_styles(style)
                .build()
                .disable()
                .finish();
        }
        button
            .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                appearance,
                label.to_string(),
                crate::util::bindings::keybinding_name_to_display_string(
                    "workspace:toggle_active_tab_privacy",
                    ctx,
                ),
            ))
            .build()
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
            .finish()
    }
}
