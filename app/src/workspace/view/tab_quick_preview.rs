use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    Dismiss, DropShadow, MouseStateHandle, ParentElement, Radius, ScrollbarWidth, Wrap,
};
use warpui::{AppContext, Element, EntityId, SingletonEntity as _};

use super::Workspace;
use crate::appearance::Appearance;
use crate::pane_group::{PanePreviewLayout, SplitDirection};
use crate::safe_triangle::SafeTriangle;
use crate::workspace::{PaneViewLocator, WorkspaceAction};

#[path = "tab_quick_preview_cards.rs"]
mod cards;

pub(super) const ANCHOR_POSITION_ID: &str = "workspace:toggle_vertical_tabs_panel";
pub(super) const PREVIEW_POSITION_ID: &str = "warpeer:tab_quick_preview";

const HOVER_IN_DELAY: Duration = Duration::from_millis(320);
const HOVER_OUT_DELAY: Duration = Duration::from_millis(220);
const PREVIEW_COLUMNS: usize = 3;
const PREVIEW_GAP: f32 = 10.;
const PREVIEW_PADDING: f32 = 12.;
const PREVIEW_BORDER_WIDTH: f32 = 1.;
const PREVIEW_MAX_HEIGHT: f32 = 520.;

fn preview_width(tab_count: usize) -> f32 {
    let columns = tab_count.clamp(1, PREVIEW_COLUMNS);
    cards::TAB_CARD_WIDTH * columns as f32
        + PREVIEW_GAP * columns.saturating_sub(1) as f32
        + PREVIEW_PADDING * 2.
        + PREVIEW_BORDER_WIDTH * 2.
}

pub(super) struct TabQuickPreviewState {
    button_hovered: bool,
    panel_hovered: bool,
    safe_triangle: RefCell<SafeTriangle>,
    scroll_state: ClippedScrollStateHandle,
    panel_mouse_state: MouseStateHandle,
    tab_mouse_states: RefCell<HashMap<EntityId, MouseStateHandle>>,
    pane_mouse_states: RefCell<Vec<(PaneViewLocator, MouseStateHandle)>>,
}

impl TabQuickPreviewState {
    pub(super) fn new() -> Self {
        Self {
            button_hovered: false,
            panel_hovered: false,
            safe_triangle: RefCell::new(SafeTriangle::new()),
            scroll_state: Default::default(),
            panel_mouse_state: Default::default(),
            tab_mouse_states: Default::default(),
            pane_mouse_states: Default::default(),
        }
    }

    pub(super) fn visible(&self, tabs_panel_open: bool) -> bool {
        !tabs_panel_open && (self.button_hovered || self.panel_hovered)
    }

    pub(super) fn set_button_hovered(
        &mut self,
        hovered: bool,
        position: Vector2F,
        preview_rect: Option<RectF>,
    ) {
        self.button_hovered = hovered;
        let mut safe_triangle = self.safe_triangle.borrow_mut();
        if hovered {
            safe_triangle.set_target_rect(None);
        } else {
            safe_triangle.set_target_rect(preview_rect);
            safe_triangle.update_position(position);
        }
    }

    pub(super) fn set_panel_hovered(&mut self, hovered: bool, position: Vector2F) {
        self.panel_hovered = hovered;
        let mut safe_triangle = self.safe_triangle.borrow_mut();
        if hovered {
            safe_triangle.should_suppress_hover(position);
        } else {
            safe_triangle.update_position(position);
        }
    }

    pub(super) fn dismiss(&mut self) {
        self.button_hovered = false;
        self.panel_hovered = false;
        self.safe_triangle.borrow_mut().set_target_rect(None);
    }
}

pub(super) fn hover_delays() -> (Duration, Duration) {
    (HOVER_IN_DELAY, HOVER_OUT_DELAY)
}

#[derive(Clone)]
struct PaneSummary {
    locator: PaneViewLocator,
    title: String,
    focused: bool,
}

#[derive(Clone)]
enum PaneLayoutSummary {
    Pane(PaneSummary),
    Split {
        direction: SplitDirection,
        children: Vec<(f32, PaneLayoutSummary)>,
    },
}

#[derive(Clone)]
struct TabSummary {
    id: EntityId,
    index: usize,
    title: String,
    active: bool,
    locked: bool,
    shortcut: Option<String>,
    layout: Option<PaneLayoutSummary>,
}

fn short_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.trim().chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn resolved_tab_title(value: &str, index: usize) -> String {
    if value.trim().is_empty() {
        format!("Tab {}", index + 1)
    } else {
        short_text(value, 42)
    }
}

fn summarize_layout(
    layout: PanePreviewLayout,
    pane_group: &crate::pane_group::PaneGroup,
    tab_id: EntityId,
    app: &AppContext,
) -> Option<PaneLayoutSummary> {
    match layout {
        PanePreviewLayout::Pane(pane_id) => {
            let pane = pane_group.pane_by_id(pane_id)?;
            let title = pane
                .pane_configuration()
                .as_ref(app)
                .title()
                .trim()
                .to_owned();
            Some(PaneLayoutSummary::Pane(PaneSummary {
                locator: PaneViewLocator {
                    pane_group_id: tab_id,
                    pane_id,
                },
                title: if title.is_empty() {
                    "Untitled pane".to_string()
                } else {
                    title
                },
                focused: pane_group.focused_pane_id(app) == pane_id,
            }))
        }
        PanePreviewLayout::Split {
            direction,
            children,
        } => {
            let children = children
                .into_iter()
                .filter_map(|(flex, child)| {
                    summarize_layout(child, pane_group, tab_id, app).map(|child| (flex, child))
                })
                .collect::<Vec<_>>();
            match children.as_slice() {
                [] => None,
                [(_, child)] => Some(child.clone()),
                _ => Some(PaneLayoutSummary::Split {
                    direction,
                    children,
                }),
            }
        }
    }
}

fn collect_pane_locators(layout: &PaneLayoutSummary, locators: &mut Vec<PaneViewLocator>) {
    match layout {
        PaneLayoutSummary::Pane(pane) => locators.push(pane.locator),
        PaneLayoutSummary::Split { children, .. } => {
            for (_, child) in children {
                collect_pane_locators(child, locators);
            }
        }
    }
}

fn summaries(workspace: &Workspace, app: &AppContext) -> Vec<TabSummary> {
    let tab_count = workspace.tabs.len();
    workspace
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let locked = tab.privacy_locked(app);
            let pane_group = tab.pane_group.as_ref(app);
            let tab_id = tab.pane_group.id();
            let layout = if locked {
                None
            } else {
                pane_group
                    .preview_layout()
                    .and_then(|layout| summarize_layout(layout, pane_group, tab_id, app))
            };
            let display_title = pane_group.display_title(app);
            TabSummary {
                id: tab_id,
                index,
                title: resolved_tab_title(&display_title, index),
                active: workspace.active_tab_index == index,
                locked,
                shortcut: crate::tab::tab_activate_binding_name(index, tab_count).and_then(
                    |binding_name| {
                        crate::util::bindings::keybinding_name_to_display_string(binding_name, app)
                    },
                ),
                layout,
            }
        })
        .collect()
}

pub(super) fn render(
    workspace: &Workspace,
    state: &TabQuickPreviewState,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let mut tabs = Wrap::row()
        .with_spacing(PREVIEW_GAP)
        .with_run_spacing(PREVIEW_GAP);
    let summaries = summaries(workspace, app);
    let panel_width = preview_width(summaries.len());
    state
        .tab_mouse_states
        .borrow_mut()
        .retain(|tab_id, _| summaries.iter().any(|summary| summary.id == *tab_id));
    let mut pane_locators = Vec::new();
    for summary in &summaries {
        if let Some(layout) = &summary.layout {
            collect_pane_locators(layout, &mut pane_locators);
        }
    }
    let mut pane_mouse_states = state.pane_mouse_states.borrow_mut();
    pane_mouse_states.retain(|(locator, _)| pane_locators.contains(locator));
    for locator in pane_locators {
        if !pane_mouse_states
            .iter()
            .any(|(existing, _)| *existing == locator)
        {
            pane_mouse_states.push((locator, MouseStateHandle::default()));
        }
    }
    for summary in summaries {
        let mouse_state = state
            .tab_mouse_states
            .borrow_mut()
            .entry(summary.id)
            .or_default()
            .clone();
        let pane_states = pane_mouse_states
            .iter()
            .filter(|(locator, _)| locator.pane_group_id == summary.id)
            .cloned()
            .collect();
        tabs.add_child(cards::render_tab(
            summary,
            mouse_state,
            pane_states,
            appearance,
        ));
    }
    let scrollable = ClippedScrollable::vertical(
        state.scroll_state.clone(),
        Container::new(tabs.finish())
            .with_uniform_padding(PREVIEW_PADDING)
            .finish(),
        ScrollbarWidth::Auto,
        theme.nonactive_ui_detail().into(),
        theme.active_ui_detail().into(),
        warpui::elements::Fill::None,
    )
    .with_overlayed_scrollbar()
    .finish();
    let panel = ConstrainedBox::new(
        Container::new(scrollable)
            .with_background(theme.surface_1().with_opacity(92))
            .with_border(
                Border::all(PREVIEW_BORDER_WIDTH)
                    .with_border_color(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .with_drop_shadow(DropShadow::default())
            .finish(),
    )
    .with_width(panel_width)
    .with_max_height(PREVIEW_MAX_HEIGHT)
    .finish();

    let panel = warpui::elements::Hoverable::new(state.panel_mouse_state.clone(), |_| panel)
        .with_hover_out_delay(HOVER_OUT_DELAY)
        .on_hover(|hovered, ctx, _, position| {
            ctx.dispatch_typed_action(WorkspaceAction::SetTabQuickPreviewPanelHovered {
                hovered,
                position,
            });
        })
        .finish();

    Dismiss::new(panel)
        .on_dismiss(|ctx, _| {
            ctx.dispatch_typed_action(WorkspaceAction::DismissTabQuickPreview);
        })
        .finish()
}

#[cfg(test)]
mod tests {
    use pathfinder_geometry::vector::Vector2F;

    use super::{TabQuickPreviewState, resolved_tab_title, short_text};

    #[test]
    fn short_text_preserves_short_titles_and_bounds_long_titles() {
        assert_eq!(short_text("terminal", 12), "terminal");
        assert_eq!(short_text("abcdefghijkl", 5), "abcde...");
    }

    #[test]
    fn resolved_tab_title_keeps_real_names_and_falls_back_for_empty_titles() {
        assert_eq!(resolved_tab_title("Private project", 3), "Private project");
        assert_eq!(resolved_tab_title("   ", 3), "Tab 4");
    }

    #[test]
    fn preview_requires_a_hover_source_and_dismiss_clears_both() {
        let mut state = TabQuickPreviewState::new();
        assert!(!state.visible(false));
        state.set_button_hovered(true, Vector2F::zero(), None);
        assert!(state.visible(false));
        assert!(!state.visible(true));
        state.set_panel_hovered(true, Vector2F::zero());
        state.set_button_hovered(false, Vector2F::zero(), None);
        assert!(state.visible(false));
        state.dismiss();
        assert!(!state.visible(false));
    }
}
