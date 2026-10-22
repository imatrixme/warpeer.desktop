use warpui::elements::{ChildView, Container, Element};
use warpui::ui_components::components::UiComponent;
use warpui::{
    AppContext, Entity, FocusContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::appearance::Appearance;
use crate::context_chips::runtime_sessions::RuntimeSessionItem;
use crate::ui_components::dialog::{Dialog, dialog_styles};
use crate::view_components::action_button::{
    ActionButton, ButtonSize, DangerPrimaryTheme, NakedTheme,
};

const DIALOG_WIDTH: f32 = 360.;

pub(super) struct RuntimeSessionKillDialog {
    cancel_button: ViewHandle<ActionButton>,
    confirm_button: ViewHandle<ActionButton>,
    session: Option<RuntimeSessionItem>,
}

impl RuntimeSessionKillDialog {
    pub(super) fn new(ctx: &mut ViewContext<Self>) -> Self {
        let cancel_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Cancel", NakedTheme)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| ctx.dispatch_typed_action(RuntimeSessionKillDialogAction::Cancel))
        });
        let confirm_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Close session", DangerPrimaryTheme)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| ctx.dispatch_typed_action(RuntimeSessionKillDialogAction::Confirm))
        });

        Self {
            cancel_button,
            confirm_button,
            session: None,
        }
    }

    pub(super) fn set_session(&mut self, session: RuntimeSessionItem) {
        self.session = Some(session);
    }

    pub(super) fn clear_session(&mut self) {
        self.session = None;
    }
}

impl Entity for RuntimeSessionKillDialog {
    type Event = RuntimeSessionKillDialogEvent;
}

impl View for RuntimeSessionKillDialog {
    fn ui_name() -> &'static str {
        "RuntimeSessionKillDialog"
    }

    fn on_focus(&mut self, _focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let (title, body) = self.session.as_ref().map_or_else(
            || (
                "Close terminal session?".to_string(),
                "This will terminate the session and all processes running in it.".to_string(),
            ),
            |session| {
                (
                    format!("Close {} session?", session.runtime().label()),
                    format!(
                        "This will terminate '{}' and all processes running in it. This action cannot be undone.",
                        session.session_name()
                    ),
                )
            },
        );

        Dialog::new(title, Some(body), dialog_styles(appearance))
            .with_bottom_row_child(ChildView::new(&self.cancel_button).finish())
            .with_bottom_row_child(
                Container::new(ChildView::new(&self.confirm_button).finish())
                    .with_margin_left(8.)
                    .finish(),
            )
            .with_width(DIALOG_WIDTH)
            .build()
            .prevent_interaction_with_other_elements()
            .on_dismiss(|ctx, _| ctx.dispatch_typed_action(RuntimeSessionKillDialogAction::Cancel))
            .finish()
    }
}

pub(super) enum RuntimeSessionKillDialogEvent {
    Confirm(RuntimeSessionItem),
    Cancel,
}

#[derive(Debug)]
pub(super) enum RuntimeSessionKillDialogAction {
    Confirm,
    Cancel,
}

impl TypedActionView for RuntimeSessionKillDialog {
    type Action = RuntimeSessionKillDialogAction;

    fn handle_action(
        &mut self,
        action: &RuntimeSessionKillDialogAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            RuntimeSessionKillDialogAction::Confirm => {
                let Some(session) = self.session.clone() else {
                    log::error!("Runtime session close confirmed without a selected session");
                    return;
                };
                ctx.emit(RuntimeSessionKillDialogEvent::Confirm(session));
            }
            RuntimeSessionKillDialogAction::Cancel => {
                ctx.emit(RuntimeSessionKillDialogEvent::Cancel);
            }
        }
    }
}
