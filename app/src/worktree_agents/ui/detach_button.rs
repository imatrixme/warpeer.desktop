use warpui::Element;
use warpui::elements::{DispatchEventResult, EventHandler, MouseStateHandle};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};

use super::HeaderAction;
use crate::appearance::Appearance;
use crate::ui_components::buttons::icon_button;
use crate::ui_components::icons::Icon;
use crate::worktree_agents::observation::AgentKey;

pub(super) fn render(
    key: AgentKey,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder().clone();
    let button = icon_button(appearance, Icon::LogOut, false, mouse_state)
        .with_style(
            UiComponentStyles::default()
                .set_width(16.)
                .set_height(16.)
                .set_padding(Coords::uniform(2.)),
        )
        .with_tooltip(move || {
            ui_builder
                .tool_tip("Detach agent".to_string())
                .build()
                .finish()
        })
        .build()
        .finish();
    EventHandler::new(button)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(HeaderAction::Detach(key.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish()
}
