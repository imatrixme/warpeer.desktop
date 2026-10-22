use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warp_core::ui::color::blend::Blend;
use warp_core::ui::theme::color::internal_colors;
use warpui::Element;
use warpui::elements::{
    Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Empty, EventHandler, Expanded, Flex, MainAxisSize, MouseStateHandle,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;
use warpui::ui_components::components::UiComponent;

use super::{PaneLayoutSummary, PaneSummary, TabSummary};
use crate::appearance::Appearance;
use crate::pane_group::SplitDirection;
use crate::ui_components::icons::Icon as UiIcon;
use crate::workspace::WorkspaceAction;

pub(super) const TAB_CARD_WIDTH: f32 = 246.;
const TAB_CARD_HEIGHT: f32 = 126.;
const TAB_PREVIEW_WIDTH: f32 = TAB_CARD_WIDTH - 24.;
const TAB_PREVIEW_HEIGHT: f32 = 70.;
const LOCK_ICON_SIZE: f32 = 14.;
const PANE_GAP: f32 = 1.;

fn render_pane(
    summary: PaneSummary,
    size: Vector2F,
    tab_index: usize,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let locator = summary.locator;
    let title = summary.title.clone();
    let pane = warpui::elements::Hoverable::new(mouse_state, move |state| {
        let theme = appearance.theme();
        let background = if state.is_hovered() {
            theme.surface_2().blend(&theme.accent().with_opacity(10))
        } else {
            theme.surface_2()
        };
        let pane = Container::new(Empty::new().finish())
            .with_background(background)
            .with_border(
                Border::all(if summary.focused { 2. } else { 1. }).with_border_color(
                    if summary.focused {
                        theme.accent().into()
                    } else {
                        internal_colors::fg_overlay_3(theme).into()
                    },
                ),
            )
            .finish();
        let mut pane_with_tooltip = Stack::new().with_child(pane);
        if state.is_hovered() {
            pane_with_tooltip.add_positioned_overlay_child(
                appearance
                    .ui_builder()
                    .tool_tip(title.clone())
                    .build()
                    .finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(0., -6.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopMiddle,
                    ChildAnchor::BottomMiddle,
                ),
            );
        }
        pane_with_tooltip.finish()
    })
    .finish();
    EventHandler::new(
        ConstrainedBox::new(pane)
            .with_width(size.x())
            .with_height(size.y())
            .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(WorkspaceAction::ActivateTab(tab_index));
        ctx.dispatch_typed_action(WorkspaceAction::FocusPane(locator));
        ctx.dispatch_typed_action(WorkspaceAction::DismissTabQuickPreview);
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn collect_pane_rects(
    summary: &PaneLayoutSummary,
    bounds: RectF,
    panes: &mut Vec<(PaneSummary, RectF)>,
) {
    match summary {
        PaneLayoutSummary::Pane(pane) => panes.push((pane.clone(), bounds)),
        PaneLayoutSummary::Split {
            direction,
            children,
        } => {
            let total_flex = children.iter().map(|(flex, _)| flex.max(0.01)).sum::<f32>();
            let main_size = match direction {
                SplitDirection::Horizontal => bounds.width(),
                SplitDirection::Vertical => bounds.height(),
            };
            let available =
                (main_size - PANE_GAP * children.len().saturating_sub(1) as f32).max(0.);
            let mut allocated = 0.;
            let mut cumulative_flex = 0.;
            for (index, (flex, child)) in children.iter().enumerate() {
                cumulative_flex += flex.max(0.01);
                let child_main_size = if index + 1 == children.len() {
                    (available - allocated).max(0.)
                } else {
                    (available * cumulative_flex / total_flex).round() - allocated
                };
                let offset = allocated + PANE_GAP * index as f32;
                let child_bounds = match direction {
                    SplitDirection::Horizontal => RectF::new(
                        bounds.origin() + vec2f(offset, 0.),
                        vec2f(child_main_size, bounds.height()),
                    ),
                    SplitDirection::Vertical => RectF::new(
                        bounds.origin() + vec2f(0., offset),
                        vec2f(bounds.width(), child_main_size),
                    ),
                };
                collect_pane_rects(child, child_bounds, panes);
                allocated += child_main_size;
            }
        }
    }
}

fn render_layout(
    summary: PaneLayoutSummary,
    tab_index: usize,
    pane_mouse_states: &[(crate::workspace::PaneViewLocator, MouseStateHandle)],
    appearance: &Appearance,
) -> Box<dyn Element> {
    let size = vec2f(TAB_PREVIEW_WIDTH, TAB_PREVIEW_HEIGHT);
    let mut panes = Vec::new();
    collect_pane_rects(&summary, RectF::new(Vector2F::zero(), size), &mut panes);
    let base = ConstrainedBox::new(Empty::new().finish())
        .with_width(size.x())
        .with_height(size.y())
        .finish();
    let mut layout = Stack::new().with_child(base);
    for (pane, bounds) in panes {
        let mouse_state = pane_mouse_states
            .iter()
            .find(|(locator, _)| *locator == pane.locator)
            .map(|(_, state)| state.clone())
            .unwrap_or_default();
        layout.add_positioned_child(
            render_pane(pane, bounds.size(), tab_index, mouse_state, appearance),
            OffsetPositioning::offset_from_parent(
                bounds.origin(),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
        );
    }
    layout.finish()
}

fn render_tab_card(
    summary: &TabSummary,
    hovered: bool,
    pane_mouse_states: &[(crate::workspace::PaneViewLocator, MouseStateHandle)],
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let title_color = if summary.active {
        theme.main_text_color(theme.accent())
    } else {
        theme.main_text_color(theme.background())
    };
    let title_text = Text::new_inline(summary.title.clone(), appearance.ui_font_family(), 12.)
        .with_clip(ClipConfig::ellipsis())
        .with_style(Properties::default().weight(if summary.active {
            Weight::Bold
        } else {
            Weight::Normal
        }))
        .with_color(title_color.into())
        .finish();
    let mut title = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Expanded::new(1., title_text).finish());
    if let Some(shortcut) = &summary.shortcut {
        title.add_child(
            Container::new(
                Text::new_inline(shortcut.clone(), appearance.ui_font_family(), 10.)
                    .with_color(title_color.into())
                    .finish(),
            )
            .with_margin_left(8.)
            .finish(),
        );
    }
    let title = Container::new(title.finish())
        .with_horizontal_padding(12.)
        .with_vertical_padding(9.)
        .finish();

    let mut tab = Flex::column()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(title);
    if summary.locked {
        let locked_preview = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(
                    ConstrainedBox::new(
                        UiIcon::LockClosed
                            .to_warpui_icon(theme.sub_text_color(theme.background()))
                            .finish(),
                    )
                    .with_width(LOCK_ICON_SIZE)
                    .with_height(LOCK_ICON_SIZE)
                    .finish(),
                )
                .with_margin_right(6.)
                .finish(),
            )
            .with_child(
                Text::new_inline("Locked", appearance.ui_font_family(), 11.)
                    .with_color(theme.sub_text_color(theme.background()).into())
                    .finish(),
            )
            .finish();
        tab.add_child(
            Container::new(
                ConstrainedBox::new(
                    Container::new(locked_preview)
                        .with_background(theme.surface_2())
                        .with_border(
                            Border::all(1.)
                                .with_border_color(internal_colors::fg_overlay_3(theme).into()),
                        )
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .with_uniform_padding(12.)
                        .finish(),
                )
                .with_width(TAB_PREVIEW_WIDTH)
                .with_height(TAB_PREVIEW_HEIGHT)
                .finish(),
            )
            .with_horizontal_padding(12.)
            .with_padding_bottom(10.)
            .finish(),
        );
    } else if let Some(layout) = summary.layout.clone() {
        tab.add_child(
            Container::new(
                ConstrainedBox::new(render_layout(
                    layout,
                    summary.index,
                    pane_mouse_states,
                    appearance,
                ))
                .with_width(TAB_PREVIEW_WIDTH)
                .with_height(TAB_PREVIEW_HEIGHT)
                .finish(),
            )
            .with_horizontal_padding(12.)
            .with_padding_bottom(10.)
            .finish(),
        );
    }
    let mut card = Container::new(tab.finish())
        .with_border(
            Border::all(if summary.active { 2. } else { 1. }).with_border_color(
                if summary.active {
                    theme.accent().into()
                } else {
                    internal_colors::fg_overlay_3(theme).into()
                },
            ),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)));
    if summary.active {
        card = card.with_background(theme.accent());
    } else if hovered {
        card = card.with_background(theme.accent().with_opacity(14));
    } else {
        card = card.with_background(theme.surface_1().with_opacity(88));
    }
    let index = summary.index;
    EventHandler::new(
        ConstrainedBox::new(card.finish())
            .with_width(TAB_CARD_WIDTH)
            .with_height(TAB_CARD_HEIGHT)
            .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(WorkspaceAction::ActivateTab(index));
        ctx.dispatch_typed_action(WorkspaceAction::DismissTabQuickPreview);
        DispatchEventResult::StopPropagation
    })
    .finish()
}

pub(super) fn render_tab(
    summary: TabSummary,
    mouse_state: MouseStateHandle,
    pane_mouse_states: Vec<(crate::workspace::PaneViewLocator, MouseStateHandle)>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    warpui::elements::Hoverable::new(mouse_state, move |state| {
        render_tab_card(&summary, state.is_hovered(), &pane_mouse_states, appearance)
    })
    .finish()
}

#[cfg(test)]
mod tests {
    use warpui::EntityId;

    use super::*;
    use crate::pane_group::TerminalPaneId;
    use crate::workspace::PaneViewLocator;

    fn pane(index: usize) -> PaneLayoutSummary {
        PaneLayoutSummary::Pane(PaneSummary {
            locator: PaneViewLocator {
                pane_group_id: EntityId::from_usize(index + 1),
                pane_id: TerminalPaneId::dummy_terminal_pane_id().into(),
            },
            title: format!("Pane {}", index + 1),
            focused: false,
        })
    }

    #[test]
    fn one_pane_fills_the_fixed_preview_canvas() {
        let bounds = RectF::new(Vector2F::zero(), vec2f(222., 70.));
        let mut panes = Vec::new();
        collect_pane_rects(&pane(0), bounds, &mut panes);

        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].1, bounds);
    }

    #[test]
    fn split_projection_preserves_direction_and_flex_ratio() {
        let layout = PaneLayoutSummary::Split {
            direction: SplitDirection::Horizontal,
            children: vec![(1., pane(0)), (3., pane(1))],
        };
        let mut panes = Vec::new();
        collect_pane_rects(
            &layout,
            RectF::new(Vector2F::zero(), vec2f(201., 70.)),
            &mut panes,
        );

        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].1.size(), vec2f(50., 70.));
        assert_eq!(panes[1].1.origin(), vec2f(51., 0.));
        assert_eq!(panes[1].1.size(), vec2f(150., 70.));
    }

    #[test]
    fn vertical_split_uses_a_pixel_aligned_one_point_gap() {
        let layout = PaneLayoutSummary::Split {
            direction: SplitDirection::Vertical,
            children: vec![(1., pane(0)), (1., pane(1))],
        };
        let mut panes = Vec::new();
        collect_pane_rects(
            &layout,
            RectF::new(Vector2F::zero(), vec2f(222., 70.)),
            &mut panes,
        );

        assert_eq!(panes[0].1.size(), vec2f(222., 35.));
        assert_eq!(panes[1].1.origin(), vec2f(0., 36.));
        assert_eq!(panes[1].1.size(), vec2f(222., 34.));
    }
}
