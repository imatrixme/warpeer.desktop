use warpui::elements::{ChildView, Element, SavePosition};
use warpui::ui_components::components::UiComponent;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{AppContext, SingletonEntity as _, ViewContext, ViewHandle};

use super::{
    SECURITY_BIOMETRIC_SWITCH_POSITION_ID, SECURITY_PRIVACY_SWITCH_POSITION_ID,
    SECURITY_SYSTEM_LOCK_SWITCH_POSITION_ID, SecurityPageAction, SecuritySettingsPageView,
};
use crate::appearance::Appearance;
use crate::settings_view::settings_page::{
    LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item, render_dropdown_item,
};
use crate::tab_privacy::{
    TabPrivacyCredentialModel, TabPrivacyRuntime, TabPrivacySettings, biometrics_available,
};
use crate::view_components::Dropdown;
use crate::view_components::action_button::{ActionButton, NakedTheme};

#[derive(Default)]
pub(super) struct StatusWidget {
    switch: SwitchStateHandle,
}

impl SettingsWidget for StatusWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        "privacy status protected tabs security"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let has_credential = TabPrivacyCredentialModel::as_ref(app).has_credential();
        let count = TabPrivacyRuntime::as_ref(app).protected_count();
        render_body_item::<SecurityPageAction>(
            "Tab privacy".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            SavePosition::new(
                appearance
                    .ui_builder()
                    .switch(self.switch.clone())
                    .check(has_credential)
                    .build()
                    .on_click(|ctx, _, _| {
                        ctx.dispatch_typed_action(SecurityPageAction::TogglePrivacy)
                    })
                    .finish(),
                SECURITY_PRIVACY_SWITCH_POSITION_ID,
            )
            .finish(),
            Some(
                if has_credential {
                    format!("Enabled with {count} protected tabs.")
                } else {
                    "Create a password to enable tab privacy.".to_string()
                }
                .into(),
            ),
        )
    }
}

pub(super) struct ActionWidget {
    pub(super) label: &'static str,
    pub(super) description: &'static str,
    pub(super) search: &'static str,
    pub(super) button: ViewHandle<ActionButton>,
    pub(super) position_id: &'static str,
}

impl SettingsWidget for ActionWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        self.search
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<SecurityPageAction>(
            self.label.into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            SavePosition::new(ChildView::new(&self.button).finish(), self.position_id).finish(),
            Some(self.description.into()),
        )
    }
}

#[derive(Default)]
pub(super) struct BiometricWidget {
    switch: SwitchStateHandle,
}

impl SettingsWidget for BiometricWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        "touch id biometric fingerprint quick unlock"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let has_credential = TabPrivacyCredentialModel::as_ref(app).has_credential();
        let available = biometrics_available();
        let description = if !has_credential {
            "Create a privacy password before enabling Touch ID."
        } else if !available {
            "Touch ID is unavailable on this device."
        } else {
            "Password entry remains available even when Touch ID is enabled."
        };
        render_body_item::<SecurityPageAction>(
            "Touch ID quick unlock".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            SavePosition::new(
                appearance
                    .ui_builder()
                    .switch(self.switch.clone())
                    .check(
                        has_credential
                            && available
                            && *TabPrivacySettings::as_ref(app).biometric_quick_unlock,
                    )
                    .build()
                    .on_click(|ctx, _, _| {
                        ctx.dispatch_typed_action(SecurityPageAction::ToggleBiometric)
                    })
                    .finish(),
                SECURITY_BIOMETRIC_SWITCH_POSITION_ID,
            )
            .finish(),
            Some(description.into()),
        )
    }
}

pub(super) struct RelockWidget {
    pub(super) dropdown: ViewHandle<Dropdown<SecurityPageAction>>,
}

impl SettingsWidget for RelockWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        "relock delay rolling timer leave tab frequency"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_dropdown_item(
            appearance,
            "Lock after leaving",
            Some(
                "The countdown resets when the tab is viewed again and restarts after the next leave.",
            ),
            None,
            LocalOnlyIconState::Hidden,
            None,
            &self.dropdown,
        )
    }
}

#[derive(Default)]
pub(super) struct SystemLockWidget {
    switch: SwitchStateHandle,
}

impl SettingsWidget for SystemLockWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        "sleep screen lock system inactivity"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<SecurityPageAction>(
            "Lock on sleep or screen lock".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            SavePosition::new(
                appearance
                    .ui_builder()
                    .switch(self.switch.clone())
                    .check(*TabPrivacySettings::as_ref(app).lock_on_system_inactivity)
                    .build()
                    .on_click(|ctx, _, _| {
                        ctx.dispatch_typed_action(SecurityPageAction::ToggleLockOnSystemInactivity)
                    })
                    .finish(),
                SECURITY_SYSTEM_LOCK_SWITCH_POSITION_ID,
            )
            .finish(),
            Some("Immediately covers protected tabs when macOS locks or sleeps.".into()),
        )
    }
}

pub(super) struct CommandWidget {
    label: &'static str,
    description: &'static str,
    search: &'static str,
    button: ViewHandle<ActionButton>,
    position_id: &'static str,
}

impl CommandWidget {
    pub(super) fn new(
        label: &'static str,
        description: &'static str,
        search: &'static str,
        button_label: &'static str,
        action: SecurityPageAction,
        position_id: &'static str,
        ctx: &mut ViewContext<SecuritySettingsPageView>,
    ) -> Self {
        let button = ctx.add_typed_action_view(move |_| {
            let action = action;
            ActionButton::new(button_label, NakedTheme).on_click(move |ctx| {
                ctx.dispatch_typed_action(action);
            })
        });
        Self {
            label,
            description,
            search,
            button,
            position_id,
        }
    }
}

impl SettingsWidget for CommandWidget {
    type View = SecuritySettingsPageView;

    fn search_terms(&self) -> &str {
        self.search
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<SecurityPageAction>(
            self.label.into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            SavePosition::new(ChildView::new(&self.button).finish(), self.position_id).finish(),
            Some(self.description.into()),
        )
    }
}
