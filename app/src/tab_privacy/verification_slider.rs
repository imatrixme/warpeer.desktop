use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warp_core::ui::icons::Icon as WarpIcon;
use warp_core::ui::theme::Fill;
use warpui::elements::{Border, CornerRadius, Icon as WarpUiIcon, Point, Radius};
use warpui::event::DispatchedEvent;
use warpui::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext,
    SizeConstraint,
};

const TRACK_INSET: f32 = 3.;
const THUMB_ICON_SIZE: f32 = 16.;

pub(super) struct PasswordVerificationSlider {
    progress: f32,
    track_fill: Fill,
    progress_fill: Fill,
    thumb_color: ColorU,
    locked_icon: WarpUiIcon,
    unlocked_icon: WarpUiIcon,
    border_color: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl PasswordVerificationSlider {
    pub(super) fn new(
        progress: f32,
        track_fill: Fill,
        progress_fill: Fill,
        thumb_color: ColorU,
        thumb_icon_color: ColorU,
        border_color: ColorU,
    ) -> Self {
        let progress = progress.clamp(0., 1.);
        let unlock_progress = ((progress - 0.68) / 0.32).clamp(0., 1.);
        Self {
            progress,
            track_fill,
            progress_fill,
            thumb_color,
            locked_icon: WarpIcon::LockClosed
                .to_warpui_icon(Fill::Solid(thumb_icon_color))
                .with_opacity(1. - unlock_progress),
            unlocked_icon: WarpIcon::Lock
                .to_warpui_icon(Fill::Solid(thumb_icon_color))
                .with_opacity(unlock_progress),
            border_color,
            size: None,
            origin: None,
        }
    }
}

impl Element for PasswordVerificationSlider {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        let icon_size = Vector2F::splat(THUMB_ICON_SIZE);
        let icon_constraint = SizeConstraint::new(icon_size, icon_size);
        self.locked_icon.layout(icon_constraint, _ctx, _app);
        self.unlocked_icon.layout(icon_constraint, _ctx, _app);
        self.size = Some(constraint.max);
        constraint.max
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.locked_icon.after_layout(ctx, app);
        self.unlocked_icon.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _app: &AppContext) {
        let size = self
            .size
            .expect("verification slider layout precedes paint");
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let radius = CornerRadius::with_all(Radius::Pixels(size.y() * 0.5));
        ctx.scene
            .draw_rect_with_hit_recording(RectF::new(origin, size))
            .with_background(self.track_fill)
            .with_border(Border::all(1.).with_border_color(self.border_color))
            .with_corner_radius(radius);

        let thumb_size = (size.y() - TRACK_INSET * 2.).max(0.);
        let thumb_travel = (size.x() - thumb_size - TRACK_INSET * 2.).max(0.);
        let thumb_x = origin.x() + TRACK_INSET + thumb_travel * self.progress;
        let fill_width = (thumb_x + thumb_size * 0.5 - origin.x()).clamp(0., size.x());
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                origin,
                Vector2F::new(fill_width, size.y()),
            ))
            .with_background(self.progress_fill)
            .with_corner_radius(radius);
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                Vector2F::new(thumb_x, origin.y() + TRACK_INSET),
                Vector2F::splat(thumb_size),
            ))
            .with_background(self.thumb_color)
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)));
        let icon_origin = Vector2F::new(
            thumb_x + (thumb_size - THUMB_ICON_SIZE) * 0.5,
            origin.y() + (size.y() - THUMB_ICON_SIZE) * 0.5,
        );
        self.locked_icon.paint(icon_origin, ctx, _app);
        self.unlocked_icon.paint(icon_origin, ctx, _app);
    }

    fn dispatch_event(
        &mut self,
        _event: &DispatchedEvent,
        _ctx: &mut EventContext,
        _app: &AppContext,
    ) -> bool {
        false
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
