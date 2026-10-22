use pathfinder_geometry::vector::vec2f;
use warpui::elements::{
    Align, Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, DropShadow, Empty, EventHandler, Expanded, Flex,
    MainAxisSize, MouseInBehavior, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::platform::Cursor;
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{AppContext, Element, SingletonEntity as _, View, ViewContext, ViewHandle};

use super::verification_slider::PasswordVerificationSlider;
use super::{
    RecoveryQuestionChoice, TAB_PRIVACY_REVEAL_DURATION, TabPrivacyOverlayAction,
    TabPrivacyOverlayView, TabPrivacyPromptPurpose,
};
use crate::appearance::Appearance;
use crate::editor::EditorView;
use crate::tab_privacy::reveal::CircularReveal;
use crate::ui_components::dialog::{Dialog, dialog_styles};
use crate::ui_components::icons::Icon;

const DIALOG_WIDTH: f32 = 420.;
const AUTH_DIALOG_WIDTH: f32 = DIALOG_WIDTH * 0.8;
const AUTH_CONTROL_HEIGHT: f32 = 40.;
const AUTH_SUBMIT_INSET: f32 = 4.;
const AUTH_SUBMIT_SIZE: f32 = AUTH_CONTROL_HEIGHT - AUTH_SUBMIT_INSET * 2.;

impl TabPrivacyPromptPurpose {
    fn title(self) -> &'static str {
        match self {
            Self::CreatePassword => "Create privacy password",
            Self::CreateAndProtect(_) => "Protect this tab",
            Self::UnlockTab(_) => "Unlock private tab",
            Self::UnlockAll => "Unlock private tabs",
            Self::ChangePassword => "Unlock to change password",
            Self::ResetPassword => "Unlock to reset password",
            Self::SetNewPassword | Self::ResetRecoveredPassword => "Choose a new password",
            Self::RemoveProtection(_) => "Remove tab protection",
            Self::DisableProtection => "Turn off tab privacy",
            Self::ConfigureRecovery => "Change recovery question",
            Self::OfferBiometrics(_) => "Enable Touch ID?",
            Self::SetupRecovery(_) => "Set a recovery question",
            Self::RecoveryChallenge => "Reset forgotten password",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::CreatePassword => {
                "Create a local password for tab privacy. No tab is protected until you choose to protect it."
            }
            Self::CreateAndProtect(_) => {
                "Create a local password. This tab will lock immediately after setup."
            }
            Self::UnlockTab(_) => "Authenticate to reveal this tab.",
            Self::UnlockAll => "Authenticate to reveal all protected tabs.",
            Self::ChangePassword => "Authenticate before opening the new password form.",
            Self::ResetPassword => "Authenticate before replacing the current password.",
            Self::SetNewPassword => "Choose the password used to unlock protected tabs.",
            Self::RemoveProtection(_) => "Authenticate to make this tab public again.",
            Self::DisableProtection => {
                "Authenticate to remove protection from every tab and clear the local credential."
            }
            Self::ConfigureRecovery => "Authenticate before replacing the existing question.",
            Self::OfferBiometrics(_) => {
                "Touch ID can unlock and authorize privacy changes without replacing your password."
            }
            Self::SetupRecovery(_) => {
                "Optional. You can skip this step and configure recovery later in Security settings."
            }
            Self::RecoveryChallenge => "Answer your recovery question to choose a new password.",
            Self::ResetRecoveredPassword => {
                "The previous password will stop working after this reset."
            }
        }
    }
}

impl TabPrivacyOverlayView {
    fn render_editor(editor: &ViewHandle<EditorView>, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(ChildView::new(editor).finish())
            .with_uniform_padding(8.)
            .with_background(appearance.theme().surface_1())
            .with_border(
                Border::all(1.).with_border_color(appearance.theme().nonactive_ui_detail().into()),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .finish()
    }

    fn render_capsule_editor(
        editor: &ViewHandle<EditorView>,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let line_height = editor.as_ref(app).line_height(app.font_cache(), appearance);
        let editor = Align::new(
            ConstrainedBox::new(ChildView::new(editor).finish())
                .with_height(line_height)
                .finish(),
        )
        .left()
        .finish();
        ConstrainedBox::new(
            Container::new(editor)
                .with_padding_left(12.)
                .with_padding_right(AUTH_CONTROL_HEIGHT)
                .with_background(appearance.theme().surface_1())
                .with_border(
                    Border::all(1.)
                        .with_border_color(appearance.theme().nonactive_ui_detail().into()),
                )
                .with_foreground_border(
                    Border::all(1.)
                        .with_border_color(appearance.theme().nonactive_ui_detail().into()),
                )
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                .finish(),
        )
        .with_height(AUTH_CONTROL_HEIGHT)
        .finish()
    }

    fn render_authentication_control(&self, app: &AppContext) -> Box<dyn Element> {
        if let Some(progress) = self.password_verification_progress() {
            return self.render_password_verification_slider(progress, app);
        }
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let submit = appearance
            .ui_builder()
            .button(
                ButtonVariant::Accent,
                self.inline_submit_mouse_state.clone(),
            )
            .with_icon_label(Icon::ArrowRight.to_warpui_icon(theme.font_color(theme.accent())))
            .with_style(UiComponentStyles {
                width: Some(AUTH_SUBMIT_SIZE),
                height: Some(AUTH_SUBMIT_SIZE),
                border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
                padding: Some(Coords::uniform(8.)),
                ..Default::default()
            })
            .build()
            .with_cursor(Cursor::PointingHand)
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(TabPrivacyOverlayAction::Submit);
            })
            .finish();
        let mut stack = Stack::new().with_child(Self::render_capsule_editor(
            &self.password_editor,
            appearance,
            app,
        ));
        stack.add_positioned_child(
            submit,
            OffsetPositioning::offset_from_parent(
                vec2f(-AUTH_SUBMIT_INSET, 0.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::MiddleRight,
                ChildAnchor::MiddleRight,
            ),
        );
        let password_control = ConstrainedBox::new(stack.finish())
            .with_height(AUTH_CONTROL_HEIGHT)
            .finish();
        if !self.biometric_available {
            return password_control;
        }
        let biometric_tooltip = appearance
            .ui_builder()
            .tool_tip("Use Touch ID".to_string())
            .build()
            .finish();
        let biometric = appearance
            .ui_builder()
            .button(
                ButtonVariant::Outlined,
                self.inline_biometric_mouse_state.clone(),
            )
            .with_icon_label(Icon::Fingerprint.to_warpui_icon(theme.active_ui_text_color().into()))
            .with_style(UiComponentStyles {
                width: Some(AUTH_CONTROL_HEIGHT),
                height: Some(AUTH_CONTROL_HEIGHT),
                border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
                padding: Some(Coords::uniform(9.)),
                ..Default::default()
            })
            .with_tooltip(move || biometric_tooltip)
            .build()
            .with_cursor(Cursor::PointingHand)
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(TabPrivacyOverlayAction::Biometric);
            })
            .finish();
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., password_control).finish())
            .with_child(Container::new(biometric).with_margin_left(8.).finish())
            .finish()
    }

    fn render_password_verification_slider(
        &self,
        progress: f32,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let slider = PasswordVerificationSlider::new(
            progress,
            theme.surface_2(),
            theme.accent(),
            theme.font_color(theme.accent()).into_solid(),
            theme.accent().into_solid(),
            theme.nonactive_ui_detail().into(),
        )
        .finish();
        let label = appearance
            .ui_builder()
            .span("Verifying...")
            .with_style(UiComponentStyles {
                font_color: Some(theme.active_ui_text_color().into()),
                ..Default::default()
            })
            .build()
            .finish();
        let mut stack = Stack::new().with_child(slider);
        stack.add_positioned_child(
            label,
            OffsetPositioning::offset_from_parent(
                vec2f(0., 0.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::Center,
                ChildAnchor::Center,
            ),
        );
        ConstrainedBox::new(stack.finish())
            .with_height(AUTH_CONTROL_HEIGHT)
            .finish()
    }

    fn render_fields(
        &self,
        purpose: TabPrivacyPromptPurpose,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut fields = Flex::column().with_main_axis_size(MainAxisSize::Min);
        match purpose {
            TabPrivacyPromptPurpose::OfferBiometrics(_) => {}
            TabPrivacyPromptPurpose::SetupRecovery(_) => {
                fields.add_child(ChildView::new(&self.recovery_dropdown).finish());
                if self.recovery_choice == RecoveryQuestionChoice::Custom {
                    fields.add_child(
                        Container::new(Self::render_editor(
                            &self.recovery_question_editor,
                            appearance,
                        ))
                        .with_margin_top(10.)
                        .finish(),
                    );
                }
                fields.add_child(
                    Container::new(Self::render_editor(
                        &self.recovery_answer_editor,
                        appearance,
                    ))
                    .with_margin_top(10.)
                    .finish(),
                );
            }
            TabPrivacyPromptPurpose::RecoveryChallenge => {
                if let Some(question) = &self.recovery_prompt {
                    fields.add_child(
                        Container::new(
                            appearance
                                .ui_builder()
                                .span(question.clone())
                                .build()
                                .finish(),
                        )
                        .with_padding_bottom(10.)
                        .finish(),
                    );
                }
                fields.add_child(Self::render_editor(
                    &self.recovery_answer_editor,
                    appearance,
                ));
            }
            _ => {
                if purpose.requires_authentication() {
                    fields.add_child(self.render_authentication_control(app));
                    if matches!(purpose, TabPrivacyPromptPurpose::UnlockTab(_))
                        && self.biometric_available
                        && let Some(shortcut) =
                            crate::util::bindings::keybinding_name_to_display_string(
                                "workspace:toggle_active_tab_privacy",
                                app,
                            )
                    {
                        fields.add_child(
                            Container::new(
                                Align::new(
                                    Text::new_inline(
                                        format!("Press {shortcut} again to use Touch ID"),
                                        appearance.ui_font_family(),
                                        11.,
                                    )
                                    .with_color(
                                        appearance
                                            .theme()
                                            .sub_text_color(appearance.theme().background())
                                            .into(),
                                    )
                                    .finish(),
                                )
                                .finish(),
                            )
                            .with_margin_top(8.)
                            .finish(),
                        );
                    }
                } else if !matches!(
                    purpose,
                    TabPrivacyPromptPurpose::SetNewPassword
                        | TabPrivacyPromptPurpose::ResetRecoveredPassword
                ) {
                    fields.add_child(Self::render_editor(&self.password_editor, appearance));
                }
                if purpose.needs_new_password() {
                    fields.add_child(
                        Container::new(Self::render_editor(&self.new_password_editor, appearance))
                            .with_margin_top(10.)
                            .finish(),
                    );
                }
                if purpose.needs_confirmation() {
                    fields.add_child(
                        Container::new(Self::render_editor(&self.confirmation_editor, appearance))
                            .with_margin_top(10.)
                            .finish(),
                    );
                }
            }
        }
        if let Some(error) = &self.error {
            fields.add_child(
                Container::new(appearance.ui_builder().span(error.clone()).build().finish())
                    .with_margin_top(10.)
                    .finish(),
            );
        }
        fields.finish()
    }

    fn render_dialog(
        &self,
        purpose: TabPrivacyPromptPurpose,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let width = if purpose.requires_authentication() {
            AUTH_DIALOG_WIDTH
        } else {
            DIALOG_WIDTH
        };
        let mut dialog = Dialog::new(
            purpose.title().to_string(),
            Some(purpose.description().to_string()),
            dialog_styles(appearance),
        )
        .with_child(self.render_fields(purpose, app))
        .with_width(width);
        match purpose {
            TabPrivacyPromptPurpose::OfferBiometrics(_) => {
                dialog = dialog
                    .with_bottom_row_child(ChildView::new(&self.skip_button).finish())
                    .with_bottom_row_child(ChildView::new(&self.enable_biometric_button).finish());
            }
            TabPrivacyPromptPurpose::SetupRecovery(_) => {
                dialog = dialog
                    .with_bottom_row_child(ChildView::new(&self.skip_button).finish())
                    .with_bottom_row_child(ChildView::new(&self.submit_button).finish());
            }
            _ => {
                if self.cancellable {
                    dialog =
                        dialog.with_bottom_row_child(ChildView::new(&self.cancel_button).finish());
                }
                if purpose.is_unlock() && self.recovery_available {
                    dialog = dialog.with_bottom_row_child(
                        ChildView::new(&self.forgot_password_button).finish(),
                    );
                }
                if !purpose.requires_authentication() {
                    dialog =
                        dialog.with_bottom_row_child(ChildView::new(&self.submit_button).finish());
                }
            }
        }
        Container::new(dialog.build().finish())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .with_drop_shadow(DropShadow::default())
            .finish()
    }
}

impl View for TabPrivacyOverlayView {
    fn ui_name() -> &'static str {
        "TabPrivacyOverlay"
    }

    fn on_focus(&mut self, focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        if self.purpose.is_some() && !self.is_verifying_password() && focus_ctx.is_self_focused() {
            self.focus_first_field(ctx);
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let content = if let Some(purpose) = self.purpose {
            let mut stack = Stack::new();
            stack.add_positioned_child(
                self.render_dialog(purpose, app),
                OffsetPositioning::offset_from_parent(
                    vec2f(0., 0.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
            let glass = Container::new(Align::new(stack.finish()).finish())
                .with_background(appearance.theme().surface_1().with_opacity(82))
                .finish();
            Container::new(glass)
                .with_background(appearance.theme().background().with_opacity(96))
                .finish()
        } else if let Some(started) = self.reveal_started_at {
            let progress = (started.elapsed().as_secs_f32()
                / TAB_PRIVACY_REVEAL_DURATION.as_secs_f32())
            .clamp(0., 1.);
            CircularReveal::new(progress, appearance.theme().surface_1().into()).finish()
        } else {
            return Empty::new().finish();
        };
        EventHandler::new(content)
            .on_mouse_in(
                |_, _, _| DispatchEventResult::StopPropagation,
                Some(MouseInBehavior {
                    fire_on_synthetic_events: true,
                    fire_when_covered: true,
                }),
            )
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .on_left_mouse_up(|_, _, _| DispatchEventResult::StopPropagation)
            .on_middle_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .on_right_mouse_down(|_, _, _, _| DispatchEventResult::StopPropagation)
            .on_forward_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .on_back_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .on_mouse_dragged(|_, _, _| DispatchEventResult::StopPropagation)
            .on_scroll_wheel(|_, _, _, _| DispatchEventResult::StopPropagation)
            .finish()
    }
}
