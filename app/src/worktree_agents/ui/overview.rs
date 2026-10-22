use std::collections::{HashMap, HashSet};

use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Border, ChildAnchor, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, Dismiss, DispatchEventResult, DropShadow, EventHandler,
    Expanded, Flex, Hoverable, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, ScrollbarWidth, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;
use warpui::{Element, EntityId};

use super::{HeaderAction, HeaderUiSnapshot, pane_connector, session_row, sorting, subagent_row};
use crate::appearance::Appearance;
use crate::worktree_agents::catalog::WorktreeDirtyState;
use crate::worktree_agents::observation::AgentKey;
use crate::worktree_agents::projections::{AgentProjection, WorktreeProjection};
use crate::worktree_agents::providers::AgentProvider;
use crate::worktree_agents::settings::WorktreeAgentSortKey;

const PANEL_WIDTH: f32 = 430.;
const PANEL_MAX_HEIGHT: f32 = 460.;
const PANEL_PADDING: f32 = 10.;

pub(super) fn render(
    projections: Vec<WorktreeProjection>,
    ui: HeaderUiSnapshot,
    connector: Option<(AgentKey, EntityId)>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let progress = ui.panel_progress();
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    column.add_child(render_sort_controls(&ui, appearance));
    for projection in projections {
        column.add_child(render_worktree(projection, &ui, appearance));
    }
    let scrollable = ClippedScrollable::vertical(
        ClippedScrollStateHandle::default(),
        Container::new(column.finish())
            .with_uniform_padding(PANEL_PADDING)
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
            .with_background(
                theme
                    .surface_1()
                    .with_opacity((88. + progress * 6.).round() as u8),
            )
            .with_border(
                Border::all(1.).with_border_color(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .with_drop_shadow(DropShadow::default())
            .finish(),
    )
    .with_width(PANEL_WIDTH * (0.96 + progress * 0.04))
    .with_max_height(PANEL_MAX_HEIGHT)
    .finish();
    let panel = Hoverable::new(ui.panel_mouse_state(), |_| panel)
        .with_hover_out_delay(super::HOVER_OUT_DELAY)
        .on_hover(move |hovered, ctx, _, _| {
            ctx.dispatch_typed_action(HeaderAction::SetPanelHovered(hovered));
        })
        .finish();

    let panel = EventHandler::new(panel)
        .on_keydown(move |ctx, _, keystroke| {
            if keystroke.key == "escape" {
                ctx.dispatch_typed_action(HeaderAction::Dismiss);
                DispatchEventResult::StopPropagation
            } else {
                DispatchEventResult::PropagateToParent
            }
        })
        .finish();

    let panel = Dismiss::new(panel)
        .on_dismiss(move |ctx, _| {
            ctx.dispatch_typed_action(HeaderAction::Dismiss);
        })
        .finish();
    let mut stack = Stack::new().with_child(panel);
    if let Some((key, target_pane)) = connector {
        stack.add_positioned_overlay_child(
            pane_connector::PaneConnector::new(
                &key,
                target_pane,
                super::status::current_pane_color(theme),
                ui.reduced_motion,
            )
            .finish(),
            OffsetPositioning::offset_from_parent(
                vec2f(0., 0.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
        );
    }
    stack.finish()
}

fn render_sort_controls(ui: &HeaderUiSnapshot, appearance: &Appearance) -> Box<dyn Element> {
    let direction = if ui.sort_descending() {
        "Descending"
    } else {
        "Ascending"
    };
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Text::new_inline("Agent order", appearance.ui_font_family(), 9.)
                .with_color(
                    appearance
                        .theme()
                        .sub_text_color(appearance.theme().surface_1())
                        .into(),
                )
                .finish(),
        )
        .with_child(Expanded::new(1., warpui::elements::Empty::new().finish()).finish())
        .with_child(sort_button(
            ui.sort_key().label().to_string(),
            HeaderAction::CycleSort,
            appearance,
        ));
    if ui.sort_key() != WorktreeAgentSortKey::Auto {
        row.add_child(
            Container::new(sort_button(
                direction.to_string(),
                HeaderAction::ToggleSortDirection,
                appearance,
            ))
            .with_margin_left(5.)
            .finish(),
        );
    }
    Container::new(row.finish()).with_margin_bottom(8.).finish()
}

fn sort_button(label: String, action: HeaderAction, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let button = Container::new(
        Text::new_inline(label, appearance.ui_font_family(), 9.)
            .with_color(theme.main_text_color(theme.surface_2()).into())
            .finish(),
    )
    .with_background(theme.surface_2().with_opacity(86))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .with_horizontal_padding(7.)
    .with_vertical_padding(4.)
    .finish();
    EventHandler::new(button)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn render_worktree(
    mut projection: WorktreeProjection,
    ui: &HeaderUiSnapshot,
    appearance: &Appearance,
) -> Box<dyn Element> {
    sorting::sort_agents(&mut projection.agents, ui.sort_key(), ui.sort_descending());
    let theme = appearance.theme();
    let title = Text::new_inline(worktree_name(&projection), appearance.ui_font_family(), 12.)
        .with_clip(ClipConfig::ellipsis())
        .with_style(Properties::default().weight(Weight::Semibold))
        .with_color(theme.main_text_color(theme.surface_1()).into())
        .finish();
    let mut header = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Expanded::new(1., title).finish());
    if projection.current {
        header.add_child(
            Text::new_inline("Current", appearance.ui_font_family(), 10.)
                .with_color(theme.accent().into())
                .with_style(Properties::default().weight(Weight::Semibold))
                .finish(),
        );
    }
    let branch = projection
        .branch
        .clone()
        .unwrap_or_else(|| detached_head_label(&projection));
    let branch_action = projection
        .branch
        .clone()
        .or_else(|| projection.detached_head.clone())
        .map(HeaderAction::CopyBranch);
    let branch_row = metadata_row(
        branch,
        ui.copy_branch_mouse_state(&projection.key),
        branch_action,
        appearance,
    );
    let path_row = metadata_row(
        worktree_detail(&projection),
        ui.copy_path_mouse_state(&projection.key),
        Some(HeaderAction::CopyPath(projection.key.clone())),
        appearance,
    );
    let mut group = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(header.finish())
        .with_child(Container::new(branch_row).with_margin_top(2.).finish())
        .with_child(
            Container::new(path_row)
                .with_margin_top(1.)
                .with_margin_bottom(5.)
                .finish(),
        );
    for (agent, depth) in agent_rows(projection.agents) {
        group.add_child(if depth == 0 {
            session_row::render(agent, projection.available, ui, appearance)
        } else {
            subagent_row::render(agent, depth, ui, appearance)
        });
    }
    Container::new(group.finish())
        .with_padding_bottom(8.)
        .finish()
}

fn agent_rows(agents: Vec<AgentProjection>) -> Vec<(AgentProjection, usize)> {
    let identities: HashSet<_> = agents
        .iter()
        .map(|agent| (agent.provider, agent.key.session_id.clone()))
        .collect();
    let mut roots = Vec::new();
    let mut children: HashMap<_, Vec<_>> = HashMap::new();
    for agent in agents {
        let parent = agent
            .relation
            .as_ref()
            .map(|relation| (agent.provider, relation.parent_session_id.clone()));
        if parent
            .as_ref()
            .is_some_and(|parent| identities.contains(parent))
        {
            children.entry(parent.unwrap()).or_default().push(agent);
        } else {
            roots.push(agent);
        }
    }
    let mut rows = Vec::new();
    let mut visited = HashSet::new();
    for root in roots {
        append_agent_rows(root, 0, &mut children, &mut visited, &mut rows);
    }
    while let Some(identity) = children.keys().next().cloned() {
        if let Some(orphans) = children.remove(&identity) {
            for orphan in orphans {
                append_agent_rows(orphan, 0, &mut children, &mut visited, &mut rows);
            }
        }
    }
    rows
}

fn append_agent_rows(
    agent: AgentProjection,
    depth: usize,
    children: &mut HashMap<(AgentProvider, String), Vec<AgentProjection>>,
    visited: &mut HashSet<(AgentProvider, String)>,
    rows: &mut Vec<(AgentProjection, usize)>,
) {
    let identity = (agent.provider, agent.key.session_id.clone());
    if !visited.insert(identity.clone()) {
        return;
    }
    rows.push((agent, depth));
    if let Some(descendants) = children.remove(&identity) {
        for descendant in descendants {
            append_agent_rows(descendant, depth + 1, children, visited, rows);
        }
    }
}

fn worktree_name(projection: &WorktreeProjection) -> String {
    projection
        .key
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "Worktree".to_string())
}

fn detached_head_label(projection: &WorktreeProjection) -> String {
    projection
        .detached_head
        .as_ref()
        .map(|head| format!("Detached {}", short_head(head)))
        .unwrap_or_else(|| "Branch unavailable".to_string())
}

fn metadata_row(
    value: String,
    mouse_state: warpui::elements::MouseStateHandle,
    action: Option<HeaderAction>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let copyable = action.is_some();
    let row = Hoverable::new(mouse_state, move |state| {
        Container::new(
            Text::new_inline(value.clone(), appearance.ui_font_family(), 10.)
                .with_clip(ClipConfig::ellipsis())
                .with_color(
                    appearance
                        .theme()
                        .sub_text_color(appearance.theme().surface_1())
                        .into(),
                )
                .finish(),
        )
        .with_background(if state.is_hovered() && copyable {
            appearance.theme().accent().with_opacity(10)
        } else {
            appearance.theme().surface_1().with_opacity(0)
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.)))
        .with_horizontal_padding(2.)
        .finish()
    })
    .finish();
    let Some(action) = action else {
        return row;
    };
    EventHandler::new(row)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn compact_path(path: &std::path::Path) -> String {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    components
        .iter()
        .rev()
        .take(2)
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join("/")
}

fn worktree_detail(projection: &WorktreeProjection) -> String {
    let availability = if projection.available {
        dirty_name(projection.dirty)
    } else {
        "unavailable"
    };
    format!("{} · {availability}", compact_path(projection.key.path()))
}

fn dirty_name(dirty: WorktreeDirtyState) -> &'static str {
    match dirty {
        WorktreeDirtyState::Unknown => "status pending",
        WorktreeDirtyState::Clean => "clean",
        WorktreeDirtyState::Dirty => "modified",
        WorktreeDirtyState::Unavailable => "status unavailable",
    }
}

fn short_head(head: &str) -> &str {
    head.get(..8).unwrap_or(head)
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
