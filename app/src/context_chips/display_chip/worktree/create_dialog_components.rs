use warpui::elements::{ConstrainedBox, Text};
use warpui::fonts::{Properties, Weight};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{Element, ViewContext, ViewHandle};

use super::create_dialog::{CreateWorktreeDialog, CreateWorktreeDialogAction};
use crate::appearance::Appearance;
use crate::editor::{EditorView, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions};
use crate::view_components::action_button::{ActionButton, ActionButtonTheme};

pub(super) const FORM_FONT_SIZE: f32 = 12.;
pub(super) const SUPPORTING_FONT_SIZE: f32 = 11.;

pub(super) fn single_line_editor(
    placeholder: &'static str,
    ctx: &mut ViewContext<CreateWorktreeDialog>,
) -> ViewHandle<EditorView> {
    ctx.add_typed_action_view(move |ctx| {
        let options = SingleLineEditorOptions {
            propagate_and_no_op_vertical_navigation_keys: PropagateAndNoOpNavigationKeys::Always,
            ..Default::default()
        };
        let mut editor = EditorView::single_line(options, ctx);
        editor.set_placeholder_text(placeholder, ctx);
        editor
    })
}

pub(super) fn action_button<T: ActionButtonTheme + 'static>(
    label: &'static str,
    theme: T,
    action: CreateWorktreeDialogAction,
    ctx: &mut ViewContext<CreateWorktreeDialog>,
) -> ViewHandle<ActionButton> {
    ctx.add_typed_action_view(move |_| {
        ActionButton::new(label, theme).on_click(move |ctx| {
            ctx.dispatch_typed_action(action.clone());
        })
    })
}

pub(super) fn field_label(label: &'static str, appearance: &Appearance) -> Box<dyn Element> {
    Text::new_inline(label, appearance.ui_font_family(), FORM_FONT_SIZE)
        .with_style(Properties::default().weight(Weight::Semibold))
        .with_color(
            appearance
                .theme()
                .main_text_color(appearance.theme().surface_1())
                .into(),
        )
        .finish()
}

pub(super) fn render_editor(
    editor: &ViewHandle<EditorView>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    ConstrainedBox::new(
        appearance
            .ui_builder()
            .text_input(editor.clone())
            .with_style(UiComponentStyles {
                background: Some(appearance.theme().surface_2().into()),
                border_color: Some(appearance.theme().surface_3().into()),
                border_width: Some(1.),
                padding: Some(Coords::uniform(7.)),
                ..Default::default()
            })
            .build()
            .finish(),
    )
    .with_height(32.)
    .finish()
}
