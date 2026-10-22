use settings::Setting as _;
use warp_errors::report_if_error;
use warpui::elements::Element;
use warpui::{
    AppContext, Entity, SingletonEntity as _, TypedActionView, View, ViewContext, ViewHandle,
};

use super::SettingsSection;
use super::settings_page::{
    Category, MatchData, PageTitle, PageType, SettingsPageMeta, SettingsPageViewHandle,
};
use crate::tab_privacy::{
    TabPrivacyCredentialModel, TabPrivacyRuntime, TabPrivacySettings, TabRelockDelay,
    biometrics_available,
};
use crate::view_components::action_button::{ActionButton, NakedTheme};
use crate::view_components::{DismissibleToast, Dropdown, DropdownItem};
use crate::workspace::{ToastStack, WorkspaceAction, WorkspaceRegistry};

mod widgets;

use widgets::{
    ActionWidget, BiometricWidget, CommandWidget, RelockWidget, StatusWidget, SystemLockWidget,
};

pub(crate) const SECURITY_PRIVACY_SWITCH_POSITION_ID: &str = "security_privacy_switch";
pub(crate) const SECURITY_PASSWORD_BUTTON_POSITION_ID: &str = "security_password_button";
pub(crate) const SECURITY_RECOVERY_BUTTON_POSITION_ID: &str = "security_recovery_button";
pub(crate) const SECURITY_RECOVER_PASSWORD_BUTTON_POSITION_ID: &str =
    "security_recover_password_button";
pub(crate) const SECURITY_BIOMETRIC_SWITCH_POSITION_ID: &str = "security_biometric_switch";
pub(crate) const SECURITY_SYSTEM_LOCK_SWITCH_POSITION_ID: &str = "security_system_lock_switch";
pub(crate) const SECURITY_LOCK_ALL_BUTTON_POSITION_ID: &str = "security_lock_all_button";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SecurityPageAction {
    TogglePrivacy,
    ChangePassword,
    ConfigureRecovery,
    RecoverPassword,
    ToggleBiometric,
    SetRelockDelay(TabRelockDelay),
    ToggleLockOnSystemInactivity,
    LockAll,
}

pub struct SecuritySettingsPageView {
    page: PageType<Self>,
    password_button: ViewHandle<ActionButton>,
    recovery_button: ViewHandle<ActionButton>,
    recover_password_button: ViewHandle<ActionButton>,
    relock_dropdown: ViewHandle<Dropdown<SecurityPageAction>>,
}

impl SecuritySettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let password_button = ctx.add_typed_action_view(|ctx| {
            let label = if TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
                "Change password"
            } else {
                "Create password"
            };
            ActionButton::new(label, NakedTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(SecurityPageAction::ChangePassword);
            })
        });
        let relock_dropdown = Self::relock_dropdown(ctx);
        let recovery_button = ctx.add_typed_action_view(|ctx| {
            let label = if TabPrivacyCredentialModel::as_ref(ctx).has_recovery() {
                "Change question"
            } else {
                "Set question"
            };
            ActionButton::new(label, NakedTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(SecurityPageAction::ConfigureRecovery);
            })
        });
        let recover_password_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Reset password", NakedTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(SecurityPageAction::RecoverPassword);
            })
        });
        let page = PageType::new_categorized(
            vec![
                Category::new(
                    "Tab privacy",
                    vec![
                        Box::new(StatusWidget::default()),
                        Box::new(ActionWidget {
                            label: "Privacy password",
                            description: "The password is always available for unlocking. Touch ID is an optional shortcut.",
                            search: "password create change security",
                            button: password_button.clone(),
                            position_id: SECURITY_PASSWORD_BUTTON_POSITION_ID,
                        }),
                        Box::new(BiometricWidget::default()),
                        Box::new(ActionWidget {
                            label: "Recovery question",
                            description: "Optional. Use an answer to reset a forgotten privacy password.",
                            search: "password recovery question forgotten reset security",
                            button: recovery_button.clone(),
                            position_id: SECURITY_RECOVERY_BUTTON_POSITION_ID,
                        }),
                        Box::new(ActionWidget {
                            label: "Forgotten password",
                            description: "Use recovery when configured, or authenticate before replacing the password.",
                            search: "forgot forgotten password recover reset answer",
                            button: recover_password_button.clone(),
                            position_id: SECURITY_RECOVER_PASSWORD_BUTTON_POSITION_ID,
                        }),
                        Box::new(RelockWidget {
                            dropdown: relock_dropdown.clone(),
                        }),
                        Box::new(SystemLockWidget::default()),
                    ],
                ),
                Category::new(
                    "Protected tabs",
                    vec![Box::new(CommandWidget::new(
                        "Lock all protected tabs",
                        "Immediately cover every protected tab.",
                        "lock all protected tabs",
                        "Lock all",
                        SecurityPageAction::LockAll,
                        SECURITY_LOCK_ALL_BUTTON_POSITION_ID,
                        ctx,
                    ))],
                ),
            ],
            Some(PageTitle::new("Security")),
        );

        let credential_handle = TabPrivacyCredentialModel::handle(ctx);
        let password_button_for_event = password_button.clone();
        let recovery_button_for_event = recovery_button.clone();
        ctx.subscribe_to_model(&credential_handle, move |_, model, _, ctx| {
            let label = if model.as_ref(ctx).has_credential() {
                "Change password"
            } else {
                "Create password"
            };
            password_button_for_event.update(ctx, |button, ctx| button.set_label(label, ctx));
            let recovery_label = if model.as_ref(ctx).has_recovery() {
                "Change question"
            } else {
                "Set question"
            };
            recovery_button_for_event
                .update(ctx, |button, ctx| button.set_label(recovery_label, ctx));
            ctx.notify();
        });
        ctx.subscribe_to_model(&TabPrivacyRuntime::handle(ctx), |_, _, _, ctx| ctx.notify());
        let dropdown_for_event = relock_dropdown.clone();
        ctx.subscribe_to_model(
            &TabPrivacySettings::handle(ctx),
            move |_, settings, event, ctx| {
                if matches!(
                    event,
                    crate::tab_privacy::TabPrivacySettingsChangedEvent::TabPrivacyRelockDelay { .. }
                ) {
                    dropdown_for_event.update(ctx, |dropdown, ctx| {
                        dropdown.set_selected_by_action(
                            SecurityPageAction::SetRelockDelay(*settings.as_ref(ctx).relock_delay),
                            ctx,
                        );
                    });
                }
                ctx.notify();
            },
        );

        Self {
            page,
            password_button,
            recovery_button,
            recover_password_button,
            relock_dropdown,
        }
    }

    fn relock_dropdown(ctx: &mut ViewContext<Self>) -> ViewHandle<Dropdown<SecurityPageAction>> {
        ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(190.);
            dropdown.set_menu_width(190., ctx);
            dropdown.add_items(
                [
                    TabRelockDelay::Immediately,
                    TabRelockDelay::Seconds30,
                    TabRelockDelay::Minute1,
                    TabRelockDelay::Minutes5,
                    TabRelockDelay::Minutes15,
                    TabRelockDelay::Never,
                ]
                .into_iter()
                .map(|delay| {
                    DropdownItem::new(delay.label(), SecurityPageAction::SetRelockDelay(delay))
                })
                .collect(),
                ctx,
            );
            dropdown.set_selected_by_action(
                SecurityPageAction::SetRelockDelay(*TabPrivacySettings::as_ref(ctx).relock_delay),
                ctx,
            );
            dropdown
        })
    }

    fn route_workspace_action(action: WorkspaceAction, ctx: &mut ViewContext<Self>) -> bool {
        let window_id = ctx.window_id();
        let Some(workspace) = WorkspaceRegistry::as_ref(ctx).get(window_id, ctx) else {
            return false;
        };
        workspace.update(ctx, |workspace, ctx| workspace.handle_action(&action, ctx));
        true
    }

    fn show_message(message: &str, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::default(message.to_string()),
                window_id,
                ctx,
            );
        });
    }

    fn show_route_failure(ctx: &mut ViewContext<Self>) {
        Self::show_message("Tab privacy controls are unavailable in this window.", ctx);
    }
}

impl Entity for SecuritySettingsPageView {
    type Event = ();
}

impl View for SecuritySettingsPageView {
    fn ui_name() -> &'static str {
        "SecuritySettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = (
            &self.password_button,
            &self.recovery_button,
            &self.recover_password_button,
            &self.relock_dropdown,
        );
        self.page.render(self, app)
    }
}

impl TypedActionView for SecuritySettingsPageView {
    type Action = SecurityPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            SecurityPageAction::TogglePrivacy => {
                let action = if TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
                    WorkspaceAction::DisableTabPrivacy
                } else {
                    WorkspaceAction::ChangeTabPrivacyPassword
                };
                if !Self::route_workspace_action(action, ctx) {
                    Self::show_route_failure(ctx);
                }
            }
            SecurityPageAction::ChangePassword => {
                if !Self::route_workspace_action(WorkspaceAction::ChangeTabPrivacyPassword, ctx) {
                    Self::show_route_failure(ctx);
                }
            }
            SecurityPageAction::ConfigureRecovery => {
                if !TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
                    Self::show_message("Create a privacy password before setting recovery.", ctx);
                } else if !Self::route_workspace_action(
                    WorkspaceAction::ConfigureTabPrivacyRecovery,
                    ctx,
                ) {
                    Self::show_route_failure(ctx);
                }
            }
            SecurityPageAction::RecoverPassword => {
                if !TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
                    Self::show_message("Create a privacy password before resetting it.", ctx);
                } else if !Self::route_workspace_action(
                    WorkspaceAction::RecoverTabPrivacyPassword,
                    ctx,
                ) {
                    Self::show_route_failure(ctx);
                }
            }
            SecurityPageAction::ToggleBiometric => {
                if !TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
                    Self::show_message("Create a privacy password before enabling Touch ID.", ctx);
                    return;
                }
                if !biometrics_available() {
                    Self::show_message("Touch ID is unavailable on this device.", ctx);
                    return;
                }
                let enabled = !*TabPrivacySettings::as_ref(ctx).biometric_quick_unlock;
                let result = TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.biometric_quick_unlock.set_value(enabled, ctx)
                });
                if result.is_ok() {
                    Self::show_message(
                        if enabled {
                            "Touch ID quick unlock enabled."
                        } else {
                            "Touch ID quick unlock disabled."
                        },
                        ctx,
                    );
                } else {
                    report_if_error!(result);
                    Self::show_message("Unable to update Touch ID quick unlock.", ctx);
                }
            }
            SecurityPageAction::SetRelockDelay(delay) => {
                TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.relock_delay.set_value(*delay, ctx));
                });
            }
            SecurityPageAction::ToggleLockOnSystemInactivity => {
                let enabled = !*TabPrivacySettings::as_ref(ctx).lock_on_system_inactivity;
                let result = TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.lock_on_system_inactivity.set_value(enabled, ctx)
                });
                if result.is_ok() {
                    Self::show_message(
                        if enabled {
                            "Lock on sleep or screen lock enabled."
                        } else {
                            "Lock on sleep or screen lock disabled."
                        },
                        ctx,
                    );
                } else {
                    report_if_error!(result);
                    Self::show_message("Unable to update system lock protection.", ctx);
                }
            }
            SecurityPageAction::LockAll => {
                let protected_count = TabPrivacyRuntime::as_ref(ctx).protected_count();
                if !Self::route_workspace_action(WorkspaceAction::LockAllProtectedTabs, ctx) {
                    Self::show_route_failure(ctx);
                } else if protected_count > 0 {
                    Self::show_message("Protected tabs locked.", ctx);
                }
            }
        }
        ctx.notify();
    }
}

impl SettingsPageMeta for SecuritySettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Security
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        crate::product_profile::is_terminal_only()
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<SecuritySettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<SecuritySettingsPageView>) -> Self {
        SettingsPageViewHandle::Security(view_handle)
    }
}
