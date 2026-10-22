use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{SurfacePoint, TerminalHoverContext};
use warpui::ViewContext;

use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::WithinModel;
use crate::terminal::view::{TerminalAction, TerminalEditor, TerminalView};

impl TerminalView {
    pub(crate) fn integration_start_plugin_hover(
        &mut self,
        context: TerminalHoverContext,
        ctx: &mut ViewContext<Self>,
    ) {
        self.plugin_hover.integration_update(context);
        ctx.notify();
    }

    pub(crate) fn integration_cancel_plugin_hover(&mut self, ctx: &mut ViewContext<Self>) {
        self.cancel_plugin_hover(ctx);
    }

    pub(crate) fn integration_hover_terminal_resource(
        &mut self,
        position: WithinModel<Point>,
        pointer: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        self.maybe_link_hover(&Some(position), Some(pointer), TerminalEditor::No, ctx);
    }

    pub(crate) fn integration_leave_terminal_resource(&mut self, ctx: &mut ViewContext<Self>) {
        self.maybe_link_hover(&None, None, TerminalEditor::No, ctx);
    }

    pub(crate) fn integration_pin_current_plugin_preview(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(link) = self.highlighted_link.clone_inner() else {
            return;
        };
        let action = self.preview_grid_image_action(&link, None);
        self.plugin_hover.clear_position();
        let TerminalAction::PreviewGridImage {
            link,
            position,
            pointer,
        } = action
        else {
            unreachable!("image preview helper must construct an image preview action");
        };
        self.preview_grid_image(&link, position, pointer, ctx);
    }

    pub(crate) fn integration_plugin_hover_context(&self) -> Option<TerminalHoverContext> {
        self.plugin_hover
            .active
            .as_ref()
            .map(|active| active.context.clone())
    }

    pub(crate) fn integration_move_plugin_container(&mut self, origin: SurfacePoint) {
        let Some(active) = self.plugin_hover.active.as_ref() else {
            return;
        };
        let Some(container) = self.plugin_hover.client.terminal_container(&active.context) else {
            return;
        };
        self.plugin_hover
            .client
            .move_container(container.container.container_id, origin);
    }
}
