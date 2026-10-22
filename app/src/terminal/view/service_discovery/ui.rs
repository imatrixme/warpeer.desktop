use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Border, ChildAnchor, ChildView, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Dismiss, DispatchEventResult, DropShadow, Empty, EventHandler, Expanded,
    Flex, Hoverable, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, ScrollbarWidth, Shrinkable, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;
use warpui::{AppContext, Element, SingletonEntity as _};

use super::collector::{CacheUsage, ServicePort, ServiceProcess};
use super::widgets::{
    action_button, empty_row, empty_state, format_bytes, info_row, metadata_text, section_gap,
    section_header,
};
use super::{COMPACT_WIDTH, ProjectPanelTab, ServiceDiscoveryAction, ServiceDiscoveryView};
use crate::appearance::Appearance;
use crate::service_discovery_settings::{ServiceDiscoverySettings, is_service_discovery_available};
use crate::ui_components::icons::Icon;

const PANEL_WIDTH: f32 = 470.;
const PANEL_MAX_HEIGHT: f32 = 560.;
const PANEL_PADDING: f32 = 10.;
const PANEL_OFFSET_Y: f32 = 5.;
const COMPACT_HEIGHT: f32 = 24.;

pub(super) fn render(view: &ServiceDiscoveryView, app: &AppContext) -> Box<dyn Element> {
    if !is_service_discovery_available() {
        return Empty::new().finish();
    }
    let settings = ServiceDiscoverySettings::as_ref(app);
    let Some(directory) = view.current_directory(app) else {
        return Empty::new().finish();
    };
    if !*settings.enabled {
        return Empty::new().finish();
    }
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let directory_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("/")
        .to_owned();
    let icon_color = theme.sub_text_color(theme.background());
    let folder = ConstrainedBox::new(Icon::Folder.to_warpui_icon(icon_color).finish())
        .with_width(14.)
        .with_height(14.)
        .finish();
    let label = Shrinkable::new(
        1.,
        Text::new_inline(directory_name, appearance.ui_font_family(), 11.)
            .with_color(theme.main_text_color(theme.background()).into())
            .with_clip(ClipConfig::ellipsis())
            .finish(),
    )
    .finish();
    let chevron = ConstrainedBox::new(Icon::ChevronDown.to_warpui_icon(icon_color).finish())
        .with_width(10.)
        .with_height(10.)
        .finish();
    let contents = Flex::row()
        .with_spacing(5.)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(folder)
        .with_child(label)
        .with_child(chevron)
        .finish();
    let compact = ConstrainedBox::new(
        Container::new(contents)
            .with_padding_left(6.)
            .with_padding_right(6.)
            .with_background(if view.is_visible() {
                theme.surface_3()
            } else {
                theme.surface_2()
            })
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish(),
    )
    .with_width(COMPACT_WIDTH)
    .with_height(COMPACT_HEIGHT)
    .finish();
    let compact = EventHandler::new(compact)
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ServiceDiscoveryAction::TogglePinned);
            DispatchEventResult::StopPropagation
        })
        .finish();
    let mut compact = Hoverable::new(view.compact_mouse_state.clone(), |_| compact)
        .with_hover_in_delay(std::time::Duration::from_millis(240))
        .with_hover_out_delay(std::time::Duration::from_millis(180));
    if *settings.open_on_hover {
        compact = compact.on_hover(|hovered, ctx, _, _| {
            ctx.dispatch_typed_action(ServiceDiscoveryAction::SetCompactHovered(hovered));
        });
    }
    let compact = compact.finish();
    let mut stack = Stack::new().with_child(compact);
    if view.is_visible() {
        stack.add_positioned_overlay_child(
            render_panel(view, appearance, app),
            OffsetPositioning::offset_from_parent(
                vec2f(0., PANEL_OFFSET_Y),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::BottomRight,
                ChildAnchor::TopRight,
            ),
        );
    }
    stack.finish()
}

fn render_panel(
    view: &ServiceDiscoveryView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_project_header(view, appearance, app))
        .with_child(
            Container::new(ChildView::new(&view.tab_control).finish())
                .with_margin_top(9.)
                .with_margin_bottom(9.)
                .finish(),
        );
    match view.selected_tab {
        ProjectPanelTab::General => column.add_child(render_general(view, appearance, app)),
        ProjectPanelTab::Server => {
            column.add_child(super::server_ui::render(view, appearance, app))
        }
    }
    let scrollable = ClippedScrollable::vertical(
        view.scroll_state.clone(),
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
            .with_background(theme.surface_1().with_opacity(96))
            .with_border(
                Border::all(1.).with_border_color(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .with_drop_shadow(DropShadow::default())
            .finish(),
    )
    .with_width(PANEL_WIDTH)
    .with_max_height(PANEL_MAX_HEIGHT)
    .finish();
    let panel = Hoverable::new(view.panel_mouse_state.clone(), |_| panel)
        .with_hover_out_delay(std::time::Duration::from_millis(180))
        .on_hover(|hovered, ctx, _, _| {
            ctx.dispatch_typed_action(ServiceDiscoveryAction::SetPanelHovered(hovered));
        })
        .finish();
    let panel = EventHandler::new(panel)
        .on_keydown(|ctx, _, keystroke| {
            if keystroke.key == "escape" {
                ctx.dispatch_typed_action(ServiceDiscoveryAction::Dismiss);
                DispatchEventResult::StopPropagation
            } else {
                DispatchEventResult::PropagateToParent
            }
        })
        .finish();
    Dismiss::new(panel)
        .on_dismiss(|ctx, _| ctx.dispatch_typed_action(ServiceDiscoveryAction::Dismiss))
        .finish()
}

fn render_project_header(
    view: &ServiceDiscoveryView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let directory = view.current_directory(app).unwrap_or_default();
    let title = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Project")
        .to_string();
    let mut title_row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Expanded::new(
                1.,
                Text::new_inline(title, appearance.ui_font_family(), 13.)
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .with_color(theme.main_text_color(theme.surface_1()).into())
                    .with_clip(ClipConfig::ellipsis())
                    .finish(),
            )
            .finish(),
        );
    if view.loading || view.action_busy {
        title_row.add_child(metadata_text("Refreshing...", appearance));
    }
    title_row.add_child(
        Container::new(action_button(
            "",
            Some(Icon::Refresh),
            ServiceDiscoveryAction::Refresh,
            appearance,
        ))
        .with_margin_left(5.)
        .finish(),
    );
    let path = Text::new_inline(
        directory.to_string_lossy().into_owned(),
        appearance.ui_font_family(),
        10.,
    )
    .with_color(theme.sub_text_color(theme.surface_1()).into())
    .with_clip(ClipConfig::ellipsis())
    .finish();
    let actions = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Expanded::new(
                1.,
                action_button(
                    "Finder",
                    Some(Icon::Folder),
                    ServiceDiscoveryAction::OpenFinder,
                    appearance,
                ),
            )
            .finish(),
        )
        .with_child(
            Container::new(
                Expanded::new(
                    1.,
                    action_button(
                        "VS Code / Zed",
                        Some(Icon::Code2),
                        ServiceDiscoveryAction::OpenEditor,
                        appearance,
                    ),
                )
                .finish(),
            )
            .with_margin_left(6.)
            .finish(),
        )
        .with_child(
            Container::new(
                Expanded::new(
                    1.,
                    action_button(
                        "Copy path",
                        Some(Icon::Copy),
                        ServiceDiscoveryAction::CopyPath,
                        appearance,
                    ),
                )
                .finish(),
            )
            .with_margin_left(6.)
            .finish(),
        )
        .finish();
    Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(title_row.finish())
        .with_child(Container::new(path).with_margin_top(2.).finish())
        .with_child(Container::new(actions).with_margin_top(8.).finish())
        .finish()
}

fn render_general(
    view: &ServiceDiscoveryView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let Some(snapshot) = view.snapshot.as_ref() else {
        return empty_state("Collecting project information...", appearance);
    };
    let settings = ServiceDiscoverySettings::as_ref(app);
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    if *settings.show_cache_usage {
        column.add_child(section_header(
            "Cache usage",
            snapshot.caches.len(),
            appearance,
        ));
        if snapshot.caches.is_empty() {
            column.add_child(empty_row("No common build caches", appearance));
        } else {
            for cache in &snapshot.caches {
                column.add_child(render_cache(cache, appearance));
            }
        }
    }
    if *settings.show_runtime_details {
        if *settings.show_cache_usage {
            column.add_child(section_gap());
        }
        column.add_child(section_header(
            "Related processes",
            snapshot.processes.len(),
            appearance,
        ));
        if snapshot.processes.is_empty() {
            column.add_child(empty_row("No related runtime processes", appearance));
        } else {
            for process in &snapshot.processes {
                column.add_child(render_process(process, appearance));
            }
        }
        column.add_child(section_gap());
        column.add_child(section_header("Ports", snapshot.ports.len(), appearance));
        if snapshot.ports.is_empty() {
            column.add_child(empty_row("No listening ports", appearance));
        } else {
            for port in &snapshot.ports {
                column.add_child(render_port(port, appearance));
            }
        }
    }
    if !*settings.show_cache_usage && !*settings.show_runtime_details {
        column.add_child(empty_row(
            "General monitoring is disabled in Settings",
            appearance,
        ));
    }
    column.finish()
}

fn render_cache(cache: &CacheUsage, appearance: &Appearance) -> Box<dyn Element> {
    let relative = cache
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    info_row(
        &cache.label,
        relative,
        &format_bytes(cache.size_bytes),
        appearance,
    )
}

fn render_process(process: &ServiceProcess, appearance: &Appearance) -> Box<dyn Element> {
    let ports = if process.ports.is_empty() {
        "No port".to_string()
    } else {
        process
            .ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    info_row(
        &format!("{} · {}", process.environment, process.name),
        &format!("PID {} · {ports}", process.pid),
        &format!(
            "{:.1}% · {}",
            process.cpu_percent,
            format_bytes(process.memory_bytes)
        ),
        appearance,
    )
}

fn render_port(port: &ServicePort, appearance: &Appearance) -> Box<dyn Element> {
    let row = info_row(
        &format!("localhost:{}", port.port),
        &format!("{} · PID {}", port.environment, port.pid),
        "Open",
        appearance,
    );
    EventHandler::new(Container::new(row).finish())
        .on_left_mouse_down({
            let port = port.port;
            move |ctx, _, _| {
                ctx.dispatch_typed_action(ServiceDiscoveryAction::OpenPort(port));
                DispatchEventResult::StopPropagation
            }
        })
        .finish()
}
