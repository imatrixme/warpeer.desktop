use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, Dismiss, DropShadow, Flex, Hoverable,
    MouseStateHandle, ParentElement, Radius, Rect, SavePosition, Shrinkable, Text, Wrap,
};
use warpui::{AppContext, Element, SingletonEntity as _};

use super::Workspace;
use crate::appearance::Appearance;
use crate::pane_group::{PaneLayoutPreset, SplitDirection};
use crate::ui_components::icons::Icon;
use crate::workspace::WorkspaceAction;

pub(super) const ANCHOR_POSITION_ID: &str = "warpeer:pane_layout_picker_button";
pub(super) const PANEL_POSITION_ID: &str = "warpeer:pane_layout_picker";

const HOVER_IN_DELAY: Duration = Duration::from_millis(180);
const HOVER_OUT_DELAY: Duration = Duration::from_millis(220);
const PANEL_PADDING: f32 = 10.;
const TILE_WIDTH: f32 = 76.;
const TILE_HEIGHT: f32 = 68.;
const TILE_GAP: f32 = 6.;
const PREVIEW_WIDTH: f32 = 54.;
const PREVIEW_HEIGHT: f32 = 34.;

pub(super) struct PaneLayoutPickerState {
    button_hovered: bool,
    panel_hovered: bool,
    pinned: bool,
    panel_mouse_state: MouseStateHandle,
    item_mouse_states: RefCell<HashMap<PaneLayoutPreset, MouseStateHandle>>,
}

impl PaneLayoutPickerState {
    pub(super) fn new() -> Self {
        Self {
            button_hovered: false,
            panel_hovered: false,
            pinned: false,
            panel_mouse_state: Default::default(),
            item_mouse_states: Default::default(),
        }
    }

    pub(super) fn visible(&self) -> bool {
        self.button_hovered || self.panel_hovered || self.pinned
    }

    pub(super) fn set_button_hovered(&mut self, hovered: bool) {
        self.button_hovered = hovered;
    }

    pub(super) fn set_panel_hovered(&mut self, hovered: bool) {
        self.panel_hovered = hovered;
    }

    pub(super) fn toggle_pinned(&mut self) {
        self.pinned = !self.pinned;
    }

    pub(super) fn dismiss(&mut self) {
        self.button_hovered = false;
        self.panel_hovered = false;
        self.pinned = false;
    }
}

pub(super) fn hover_delays() -> (Duration, Duration) {
    (HOVER_IN_DELAY, HOVER_OUT_DELAY)
}

pub(super) fn render_button(workspace: &Workspace, appearance: &Appearance) -> Box<dyn Element> {
    let (hover_in, hover_out) = hover_delays();
    let button = workspace
        .render_tab_bar_icon_button(
            appearance,
            Icon::LayoutAlt01,
            &workspace.mouse_states.pane_layout_picker,
            WorkspaceAction::TogglePaneLayoutPicker,
            "Pane layouts".to_string(),
            None,
            workspace.pane_layout_picker.visible(),
            false,
        )
        .additional_on_hover(|hovered, ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::SetPaneLayoutPickerButtonHovered(hovered));
        })
        .with_hover_in_delay(hover_in)
        .with_hover_out_delay(hover_out)
        .finish();

    SavePosition::new(Align::new(button).finish(), ANCHOR_POSITION_ID).finish()
}

enum PreviewNode {
    Pane,
    Split {
        direction: SplitDirection,
        children: Vec<(f32, PreviewNode)>,
    },
}

fn preview_node(preset: PaneLayoutPreset) -> PreviewNode {
    let pane = || PreviewNode::Pane;
    let split = |direction, children| PreviewNode::Split {
        direction,
        children,
    };
    match preset {
        PaneLayoutPreset::TwoColumns => {
            split(SplitDirection::Horizontal, vec![(1., pane()), (1., pane())])
        }
        PaneLayoutPreset::TwoRows => {
            split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())])
        }
        PaneLayoutPreset::ThreeColumns => split(
            SplitDirection::Horizontal,
            vec![(1., pane()), (1., pane()), (1., pane())],
        ),
        PaneLayoutPreset::ThreeRows => split(
            SplitDirection::Vertical,
            vec![(1., pane()), (1., pane()), (1., pane())],
        ),
        PaneLayoutPreset::RightStack => split(
            SplitDirection::Horizontal,
            vec![
                (1., pane()),
                (
                    1.,
                    split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())]),
                ),
            ],
        ),
        PaneLayoutPreset::LeftStack => split(
            SplitDirection::Horizontal,
            vec![
                (
                    1.,
                    split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())]),
                ),
                (1., pane()),
            ],
        ),
        PaneLayoutPreset::BottomStack => split(
            SplitDirection::Vertical,
            vec![
                (1., pane()),
                (
                    1.,
                    split(SplitDirection::Horizontal, vec![(1., pane()), (1., pane())]),
                ),
            ],
        ),
        PaneLayoutPreset::TopStack => split(
            SplitDirection::Vertical,
            vec![
                (
                    1.,
                    split(SplitDirection::Horizontal, vec![(1., pane()), (1., pane())]),
                ),
                (1., pane()),
            ],
        ),
        PaneLayoutPreset::MainLeft => split(
            SplitDirection::Horizontal,
            vec![
                (2., pane()),
                (
                    1.,
                    split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())]),
                ),
            ],
        ),
        PaneLayoutPreset::MainTop => split(
            SplitDirection::Vertical,
            vec![
                (2., pane()),
                (
                    1.,
                    split(SplitDirection::Horizontal, vec![(1., pane()), (1., pane())]),
                ),
            ],
        ),
        PaneLayoutPreset::Grid => split(
            SplitDirection::Horizontal,
            vec![
                (
                    1.,
                    split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())]),
                ),
                (
                    1.,
                    split(SplitDirection::Vertical, vec![(1., pane()), (1., pane())]),
                ),
            ],
        ),
    }
}

fn render_preview_node(node: PreviewNode, pane_color: warpui::elements::Fill) -> Box<dyn Element> {
    match node {
        PreviewNode::Pane => Rect::new().with_background(pane_color).finish(),
        PreviewNode::Split {
            direction,
            children,
        } => {
            let mut flex = match direction {
                SplitDirection::Horizontal => Flex::row(),
                SplitDirection::Vertical => Flex::column(),
            }
            .with_spacing(1.);
            for (weight, child) in children {
                flex.add_child(
                    Shrinkable::new(weight, render_preview_node(child, pane_color)).finish(),
                );
            }
            flex.finish()
        }
    }
}

fn render_tile(
    preset: PaneLayoutPreset,
    enabled: bool,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let pane_color = if enabled {
        theme.active_ui_detail().into()
    } else {
        theme.disabled_text_color(theme.background()).into()
    };
    let text_color = if enabled {
        theme.main_text_color(theme.background())
    } else {
        theme.disabled_text_color(theme.background())
    };
    let preview = ConstrainedBox::new(render_preview_node(preview_node(preset), pane_color))
        .with_width(PREVIEW_WIDTH)
        .with_height(PREVIEW_HEIGHT)
        .finish();
    let contents = Flex::column()
        .with_spacing(6.)
        .with_child(preview)
        .with_child(
            Text::new_inline(preset.label().to_string(), appearance.ui_font_family(), 11.)
                .with_color(text_color.into())
                .finish(),
        )
        .finish();
    let tile = move |state: &warpui::elements::MouseState| {
        let background = if state.is_hovered() && enabled {
            internal_colors::fg_overlay_2(theme)
        } else {
            theme.surface_1().into()
        };
        ConstrainedBox::new(
            Container::new(contents)
                .with_background(background)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_uniform_padding(6.)
                .finish(),
        )
        .with_width(TILE_WIDTH)
        .with_height(TILE_HEIGHT)
        .finish()
    };

    let hoverable = Hoverable::new(mouse_state, tile);
    if enabled {
        hoverable
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::ApplyPaneLayoutPreset(preset));
            })
            .finish()
    } else {
        hoverable.finish()
    }
}

pub(super) fn render(
    workspace: &Workspace,
    state: &PaneLayoutPickerState,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let pane_group = workspace.active_tab_pane_group().as_ref(app);
    let mut tiles = Wrap::row()
        .with_spacing(TILE_GAP)
        .with_run_spacing(TILE_GAP);
    let mut item_mouse_states = state.item_mouse_states.borrow_mut();
    for preset in PaneLayoutPreset::ALL {
        let mouse_state = item_mouse_states.entry(preset).or_default().clone();
        tiles.add_child(render_tile(
            preset,
            pane_group.can_apply_layout_preset(),
            mouse_state,
            appearance,
        ));
    }

    let panel = ConstrainedBox::new(
        Container::new(tiles.finish())
            .with_background(theme.surface_1().with_opacity(96))
            .with_border(
                Border::all(1.).with_border_color(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .with_drop_shadow(DropShadow::default())
            .with_uniform_padding(PANEL_PADDING)
            .finish(),
    )
    .with_width(TILE_WIDTH * 3. + TILE_GAP * 2. + PANEL_PADDING * 2. + 2.)
    .finish();
    let panel = Hoverable::new(state.panel_mouse_state.clone(), |_| panel)
        .with_hover_out_delay(HOVER_OUT_DELAY)
        .on_hover(|hovered, ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::SetPaneLayoutPickerPanelHovered(hovered));
        })
        .finish();

    Dismiss::new(panel)
        .on_dismiss(|ctx, _| {
            ctx.dispatch_typed_action(WorkspaceAction::DismissPaneLayoutPicker);
        })
        .finish()
}
