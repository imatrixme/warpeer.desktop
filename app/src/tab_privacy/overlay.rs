use std::time::{Duration, Instant};

#[cfg(feature = "integration_tests")]
use warpui::AppContext;
use warpui::elements::MouseStateHandle;
use warpui::keymap::Keystroke;
use warpui::{Entity, EntityId, TypedActionView, ViewContext, ViewHandle};

use crate::editor::EditorView;
use crate::view_components::action_button::{
    ActionButton, KeystrokeSource, NakedTheme, PrimaryTheme,
};
use crate::view_components::{Dropdown, DropdownItem};

#[path = "overlay_editors.rs"]
mod overlay_editors;
#[path = "overlay_render.rs"]
mod overlay_render;
#[path = "overlay_reveal.rs"]
mod overlay_reveal;
#[path = "overlay_submit.rs"]
mod overlay_submit;
#[path = "verification.rs"]
mod verification;
#[path = "verification_slider.rs"]
mod verification_slider;

use verification::PasswordVerificationAnimation;

pub(crate) const TAB_PRIVACY_REVEAL_DURATION: Duration = Duration::from_millis(420);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PasswordField {
    Password,
    NewPassword,
    Confirmation,
    RecoveryQuestion,
    RecoveryAnswer,
}

const PASSWORD_ONLY_FIELDS: &[PasswordField] = &[PasswordField::Password];
const PASSWORD_CONFIRMATION_FIELDS: &[PasswordField] =
    &[PasswordField::Password, PasswordField::Confirmation];
const NEW_PASSWORD_FIELDS: &[PasswordField] =
    &[PasswordField::NewPassword, PasswordField::Confirmation];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPrivacySetupCompletion {
    PasswordOnly,
    ProtectTab(EntityId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPrivacyPromptPurpose {
    CreatePassword,
    CreateAndProtect(EntityId),
    UnlockTab(EntityId),
    UnlockAll,
    ChangePassword,
    ResetPassword,
    SetNewPassword,
    RemoveProtection(EntityId),
    DisableProtection,
    ConfigureRecovery,
    OfferBiometrics(TabPrivacySetupCompletion),
    SetupRecovery(Option<TabPrivacySetupCompletion>),
    RecoveryChallenge,
    ResetRecoveredPassword,
}

impl TabPrivacyPromptPurpose {
    pub(crate) fn requires_authentication(self) -> bool {
        matches!(
            self,
            Self::UnlockTab(_)
                | Self::UnlockAll
                | Self::ChangePassword
                | Self::ResetPassword
                | Self::RemoveProtection(_)
                | Self::DisableProtection
                | Self::ConfigureRecovery
        )
    }

    pub(crate) fn is_unlock(self) -> bool {
        matches!(self, Self::UnlockTab(_) | Self::UnlockAll)
    }

    fn needs_new_password(self) -> bool {
        matches!(self, Self::SetNewPassword | Self::ResetRecoveredPassword)
    }

    fn needs_confirmation(self) -> bool {
        matches!(
            self,
            Self::CreatePassword
                | Self::CreateAndProtect(_)
                | Self::SetNewPassword
                | Self::ResetRecoveredPassword
        )
    }

    fn password_fields(self, custom_recovery: bool) -> Vec<PasswordField> {
        match self {
            Self::CreatePassword | Self::CreateAndProtect(_) => {
                PASSWORD_CONFIRMATION_FIELDS.to_vec()
            }
            Self::ChangePassword | Self::ResetPassword => PASSWORD_ONLY_FIELDS.to_vec(),
            Self::SetNewPassword | Self::ResetRecoveredPassword => NEW_PASSWORD_FIELDS.to_vec(),
            Self::UnlockTab(_)
            | Self::UnlockAll
            | Self::RemoveProtection(_)
            | Self::DisableProtection
            | Self::ConfigureRecovery => PASSWORD_ONLY_FIELDS.to_vec(),
            Self::SetupRecovery(_) if custom_recovery => vec![
                PasswordField::RecoveryQuestion,
                PasswordField::RecoveryAnswer,
            ],
            Self::SetupRecovery(_) | Self::RecoveryChallenge => {
                vec![PasswordField::RecoveryAnswer]
            }
            Self::OfferBiometrics(_) => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryQuestionChoice {
    FirstProject,
    FirstWorkCity,
    MemorableCommand,
    Custom,
}

impl RecoveryQuestionChoice {
    fn label(self) -> &'static str {
        match self {
            Self::FirstProject => "What was the name of your first project?",
            Self::FirstWorkCity => "In which city did you first work?",
            Self::MemorableCommand => "What terminal command is memorable to you?",
            Self::Custom => "Custom question...",
        }
    }
}

pub enum TabPrivacyOverlayEvent {
    Submit {
        purpose: TabPrivacyPromptPurpose,
        password: String,
        new_password: Option<String>,
        confirmation: Option<String>,
        recovery_question: Option<String>,
        recovery_answer: Option<String>,
    },
    Biometric(TabPrivacyPromptPurpose),
    BiometricSetup {
        completion: TabPrivacySetupCompletion,
        enable: bool,
    },
    SkipRecovery(Option<TabPrivacySetupCompletion>),
    ForgotPassword,
    PasswordVerificationCompleted(TabPrivacyPromptPurpose),
    Cancel(TabPrivacyPromptPurpose),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPrivacyOverlayAction {
    Submit,
    Biometric,
    Cancel,
    Skip,
    EnableBiometric,
    ForgotPassword,
    SelectRecoveryQuestion(RecoveryQuestionChoice),
}

pub struct TabPrivacyOverlayView {
    pub(super) purpose: Option<TabPrivacyPromptPurpose>,
    pub(super) password_editor: ViewHandle<EditorView>,
    pub(super) new_password_editor: ViewHandle<EditorView>,
    pub(super) confirmation_editor: ViewHandle<EditorView>,
    pub(super) recovery_question_editor: ViewHandle<EditorView>,
    pub(super) recovery_answer_editor: ViewHandle<EditorView>,
    pub(super) recovery_dropdown: ViewHandle<Dropdown<TabPrivacyOverlayAction>>,
    pub(super) submit_button: ViewHandle<ActionButton>,
    pub(super) cancel_button: ViewHandle<ActionButton>,
    pub(super) skip_button: ViewHandle<ActionButton>,
    pub(super) enable_biometric_button: ViewHandle<ActionButton>,
    pub(super) forgot_password_button: ViewHandle<ActionButton>,
    pub(super) inline_submit_mouse_state: MouseStateHandle,
    pub(super) inline_biometric_mouse_state: MouseStateHandle,
    pub(super) recovery_choice: RecoveryQuestionChoice,
    pub(super) recovery_prompt: Option<String>,
    pub(super) cancellable: bool,
    pub(super) biometric_available: bool,
    pub(super) recovery_available: bool,
    pub(super) error: Option<String>,
    pub(super) password_verification: Option<PasswordVerificationAnimation>,
    pub(super) verification_epoch: u64,
    pub(super) reveal_started_at: Option<Instant>,
    reveal_epoch: u64,
}

impl TabPrivacyOverlayView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let password_editor = Self::editor("Password", true, ctx);
        let new_password_editor = Self::editor("New password", true, ctx);
        let confirmation_editor = Self::editor("Confirm password", true, ctx);
        let recovery_question_editor = Self::editor("Custom recovery question", false, ctx);
        let recovery_answer_editor = Self::editor("Answer", true, ctx);
        for (editor, field) in [
            (&password_editor, PasswordField::Password),
            (&new_password_editor, PasswordField::NewPassword),
            (&confirmation_editor, PasswordField::Confirmation),
            (&recovery_question_editor, PasswordField::RecoveryQuestion),
            (&recovery_answer_editor, PasswordField::RecoveryAnswer),
        ] {
            ctx.subscribe_to_view(editor, move |view, _, event, ctx| {
                view.handle_editor_event(field, event, ctx);
            });
        }
        let recovery_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(360.);
            dropdown.set_menu_width(360., ctx);
            dropdown.add_items(
                [
                    RecoveryQuestionChoice::FirstProject,
                    RecoveryQuestionChoice::FirstWorkCity,
                    RecoveryQuestionChoice::MemorableCommand,
                    RecoveryQuestionChoice::Custom,
                ]
                .into_iter()
                .map(|choice| {
                    DropdownItem::new(
                        choice.label(),
                        TabPrivacyOverlayAction::SelectRecoveryQuestion(choice),
                    )
                })
                .collect(),
                ctx,
            );
            dropdown.set_selected_by_action(
                TabPrivacyOverlayAction::SelectRecoveryQuestion(
                    RecoveryQuestionChoice::FirstProject,
                ),
                ctx,
            );
            dropdown
        });
        let enter = Keystroke::parse("enter").expect("valid keystroke");
        let submit_button = ctx.add_typed_action_view(move |ctx| {
            ActionButton::new("Continue", PrimaryTheme)
                .with_keybinding(KeystrokeSource::Fixed(enter), ctx)
                .on_click(|ctx| ctx.dispatch_typed_action(TabPrivacyOverlayAction::Submit))
        });
        let cancel_button = Self::button("Cancel", TabPrivacyOverlayAction::Cancel, ctx);
        let skip_button = Self::button("Skip", TabPrivacyOverlayAction::Skip, ctx);
        let enable_biometric_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Enable Touch ID", PrimaryTheme)
                .on_click(|ctx| ctx.dispatch_typed_action(TabPrivacyOverlayAction::EnableBiometric))
        });
        let forgot_password_button = Self::button(
            "Forgot password?",
            TabPrivacyOverlayAction::ForgotPassword,
            ctx,
        );
        Self {
            purpose: None,
            password_editor,
            new_password_editor,
            confirmation_editor,
            recovery_question_editor,
            recovery_answer_editor,
            recovery_dropdown,
            submit_button,
            cancel_button,
            skip_button,
            enable_biometric_button,
            forgot_password_button,
            inline_submit_mouse_state: MouseStateHandle::default(),
            inline_biometric_mouse_state: MouseStateHandle::default(),
            recovery_choice: RecoveryQuestionChoice::FirstProject,
            recovery_prompt: None,
            cancellable: false,
            biometric_available: false,
            recovery_available: false,
            error: None,
            password_verification: None,
            verification_epoch: 0,
            reveal_started_at: None,
            reveal_epoch: 0,
        }
    }

    fn button(
        label: &'static str,
        action: TabPrivacyOverlayAction,
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<ActionButton> {
        ctx.add_typed_action_view(move |_| {
            ActionButton::new(label, NakedTheme)
                .on_click(move |ctx| ctx.dispatch_typed_action(action))
        })
    }

    pub fn show(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        cancellable: bool,
        biometric_available: bool,
        recovery_available: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.purpose = Some(purpose);
        self.cancellable = cancellable;
        self.biometric_available = biometric_available && purpose.requires_authentication();
        self.recovery_available = recovery_available;
        self.recovery_prompt = None;
        self.error = None;
        self.reset_password_verification();
        self.reveal_started_at = None;
        self.reveal_epoch = self.reveal_epoch.wrapping_add(1);
        self.clear_fields(ctx);
        self.focus_first_field(ctx);
        ctx.notify();
    }

    pub fn show_recovery_setup(
        &mut self,
        completion: Option<TabPrivacySetupCompletion>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.recovery_choice = RecoveryQuestionChoice::FirstProject;
        self.recovery_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_action(
                TabPrivacyOverlayAction::SelectRecoveryQuestion(
                    RecoveryQuestionChoice::FirstProject,
                ),
                ctx,
            );
        });
        self.show(
            TabPrivacyPromptPurpose::SetupRecovery(completion),
            true,
            false,
            false,
            ctx,
        );
    }

    pub fn show_recovery_challenge(&mut self, question: String, ctx: &mut ViewContext<Self>) {
        self.show(
            TabPrivacyPromptPurpose::RecoveryChallenge,
            true,
            false,
            true,
            ctx,
        );
        self.recovery_prompt = Some(question);
        ctx.notify();
    }

    pub fn hide(&mut self, ctx: &mut ViewContext<Self>) {
        self.purpose = None;
        self.reset_password_verification();
        self.reveal_started_at = None;
        self.reveal_epoch = self.reveal_epoch.wrapping_add(1);
        self.error = None;
        self.clear_fields(ctx);
        ctx.notify();
    }

    pub fn purpose(&self) -> Option<TabPrivacyPromptPurpose> {
        self.purpose
    }

    pub fn is_visible(&self) -> bool {
        self.purpose.is_some() || self.reveal_started_at.is_some()
    }

    pub fn set_error(&mut self, error: impl Into<String>, ctx: &mut ViewContext<Self>) {
        self.reset_password_verification();
        self.error = Some(error.into());
        self.clear_active_secret(ctx);
        self.focus_first_field(ctx);
        ctx.notify();
    }

    pub fn focus_password(&self, ctx: &mut ViewContext<Self>) {
        self.focus_first_field(ctx);
    }

    #[cfg(feature = "integration_tests")]
    pub fn editor_focus_for_integration(&self, ctx: &AppContext) -> (bool, bool) {
        (
            self.password_editor.as_ref(ctx).is_focused(),
            self.confirmation_editor.as_ref(ctx).is_focused(),
        )
    }
}

impl Entity for TabPrivacyOverlayView {
    type Event = TabPrivacyOverlayEvent;
}

impl TypedActionView for TabPrivacyOverlayView {
    type Action = TabPrivacyOverlayAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        let Some(purpose) = self.purpose else {
            return;
        };
        match action {
            TabPrivacyOverlayAction::Submit => self.submit(ctx),
            TabPrivacyOverlayAction::Biometric if self.biometric_available => {
                ctx.emit(TabPrivacyOverlayEvent::Biometric(purpose));
            }
            TabPrivacyOverlayAction::Biometric => {}
            TabPrivacyOverlayAction::Cancel => self.cancel(ctx),
            TabPrivacyOverlayAction::Skip => match purpose {
                TabPrivacyPromptPurpose::OfferBiometrics(completion) => {
                    ctx.emit(TabPrivacyOverlayEvent::BiometricSetup {
                        completion,
                        enable: false,
                    });
                }
                TabPrivacyPromptPurpose::SetupRecovery(completion) => {
                    ctx.emit(TabPrivacyOverlayEvent::SkipRecovery(completion));
                }
                _ => {}
            },
            TabPrivacyOverlayAction::EnableBiometric => {
                if let TabPrivacyPromptPurpose::OfferBiometrics(completion) = purpose {
                    ctx.emit(TabPrivacyOverlayEvent::BiometricSetup {
                        completion,
                        enable: true,
                    });
                }
            }
            TabPrivacyOverlayAction::ForgotPassword if self.recovery_available => {
                ctx.emit(TabPrivacyOverlayEvent::ForgotPassword);
            }
            TabPrivacyOverlayAction::ForgotPassword => {}
            TabPrivacyOverlayAction::SelectRecoveryQuestion(choice) => {
                self.recovery_choice = *choice;
                if *choice == RecoveryQuestionChoice::Custom {
                    ctx.focus(&self.recovery_question_editor);
                } else {
                    ctx.focus(&self.recovery_answer_editor);
                }
                ctx.notify();
            }
        }
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
