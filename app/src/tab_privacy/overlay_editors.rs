use warp_editor::editor::NavigationKey;
use warpui::{SingletonEntity as _, ViewContext, ViewHandle};

use super::{
    PasswordField, RecoveryQuestionChoice, TabPrivacyOverlayView, TabPrivacyPromptPurpose,
};
use crate::appearance::Appearance;
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextColors, TextOptions,
};

impl TabPrivacyOverlayView {
    pub(super) fn editor(
        placeholder: &'static str,
        is_password: bool,
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<EditorView> {
        let appearance = Appearance::as_ref(ctx);
        let text_colors = TextColors {
            default_color: appearance.theme().active_ui_text_color(),
            disabled_color: appearance.theme().disabled_ui_text_color(),
            hint_color: appearance.theme().disabled_ui_text_color(),
        };
        let font_family = appearance.ui_font_family();
        ctx.add_typed_action_view(move |ctx| {
            let options = SingleLineEditorOptions {
                is_password,
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                text: TextOptions {
                    font_family_override: Some(font_family),
                    text_colors_override: Some(text_colors.clone()),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut editor = EditorView::single_line(options, ctx);
            editor.set_placeholder_text(placeholder, ctx);
            editor
        })
    }

    pub(super) fn clear_fields(&self, ctx: &mut ViewContext<Self>) {
        for editor in [
            &self.password_editor,
            &self.new_password_editor,
            &self.confirmation_editor,
            &self.recovery_question_editor,
            &self.recovery_answer_editor,
        ] {
            editor.update(ctx, |editor, ctx| editor.set_buffer_text("", ctx));
        }
    }

    pub(super) fn clear_active_secret(&self, ctx: &mut ViewContext<Self>) {
        let Some(purpose) = self.purpose else {
            return;
        };
        match purpose {
            TabPrivacyPromptPurpose::RecoveryChallenge
            | TabPrivacyPromptPurpose::SetupRecovery(_) => {
                self.recovery_answer_editor
                    .update(ctx, |editor, ctx| editor.set_buffer_text("", ctx));
            }
            _ => self
                .password_editor
                .update(ctx, |editor, ctx| editor.set_buffer_text("", ctx)),
        }
    }

    pub(super) fn focus_first_field(&self, ctx: &mut ViewContext<Self>) {
        let Some(purpose) = self.purpose else {
            return;
        };
        let custom = self.recovery_choice == RecoveryQuestionChoice::Custom;
        if let Some(field) = purpose.password_fields(custom).first() {
            self.focus_field(*field, ctx);
        }
    }

    fn focus_field(&self, field: PasswordField, ctx: &mut ViewContext<Self>) {
        match field {
            PasswordField::Password => ctx.focus(&self.password_editor),
            PasswordField::NewPassword => ctx.focus(&self.new_password_editor),
            PasswordField::Confirmation => ctx.focus(&self.confirmation_editor),
            PasswordField::RecoveryQuestion => ctx.focus(&self.recovery_question_editor),
            PasswordField::RecoveryAnswer => ctx.focus(&self.recovery_answer_editor),
        }
        ctx.notify();
    }

    fn focus_adjacent_field(
        &self,
        current: PasswordField,
        forward: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(purpose) = self.purpose else {
            return;
        };
        let fields =
            purpose.password_fields(self.recovery_choice == RecoveryQuestionChoice::Custom);
        let Some(index) = fields.iter().position(|field| *field == current) else {
            return;
        };
        let next = if forward {
            (index + 1) % fields.len()
        } else {
            (index + fields.len() - 1) % fields.len()
        };
        self.focus_field(fields[next], ctx);
    }

    pub(super) fn handle_editor_event(
        &mut self,
        field: PasswordField,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            EditorEvent::Navigate(NavigationKey::Tab) => {
                self.focus_adjacent_field(field, true, ctx)
            }
            EditorEvent::Navigate(NavigationKey::ShiftTab) => {
                self.focus_adjacent_field(field, false, ctx)
            }
            EditorEvent::Enter => self.submit(ctx),
            EditorEvent::Escape if self.cancellable => self.cancel(ctx),
            _ => {}
        }
    }
}
