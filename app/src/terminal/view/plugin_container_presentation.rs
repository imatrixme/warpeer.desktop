use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::icons::Icon;
use warp_core::ui::theme::Fill;
use warpeer_plugin_protocol::{
    ChromeAction, ChromeActionSource, ChromeVisibility, ContainerSnapshot, SurfaceBounds,
    SurfaceContainerMetrics,
};
use warpui::SingletonEntity as _;
use warpui::elements::{
    ChildAnchor, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Hoverable,
    MainAxisAlignment, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, SavePosition, Stack, Text,
};
use warpui::platform::Cursor;

use super::{PluginHoverState, SurfacePresentationOptions};
use crate::appearance::Appearance;
use crate::warpeer_plugin::{
    PLUGIN_WINDOW_TITLEBAR_HEIGHT, PresentedContainer, container_bounds,
    plugin_window_container_metrics,
};

const ACTION_SIZE: f32 = 18.0;
const ACTION_GAP: f32 = 3.0;
const TITLE_HORIZONTAL_PADDING: f32 = 8.0;
const HOST_CLOSE_RESERVE: f32 = 24.0;

pub(super) fn render(
    state: &PluginHoverState,
    presented: PresentedContainer,
    app: &warpui::AppContext,
) -> Box<dyn Element> {
    let container_id = presented.container.container_id;
    let container_revision = presented.container.revision;
    let transition = presented.container.transition;
    let transition_surface = presented.transition_surface;
    let page = presented
        .container
        .active_page()
        .expect("presented container has an active page")
        .clone();
    let metrics = match page.chrome.visibility {
        ChromeVisibility::Visible => plugin_window_container_metrics(),
        ChromeVisibility::Hidden => SurfaceContainerMetrics {
            border_width: plugin_window_container_metrics().border_width,
            titlebar_height: 0.0,
        },
    };
    let viewport = viewport_bounds(presented.container.viewport);
    let options = SurfacePresentationOptions::standard(
        app,
        false,
        warpui::elements::CornerRadius::with_all(warpui::elements::Radius::Pixels(0.)),
    )
    .with_container_metrics(metrics)
    .with_container_drag(
        presented.container.container_id,
        presented.container.origin,
        viewport,
        page.chrome.drag_regions.clone(),
    );
    let outer = container_bounds(presented.surface.descriptor.bounds, metrics);
    let mut stack = Stack::new();
    stack.add_child(state.presentation.container_element(
        presented.surface,
        transition_surface,
        options,
        container_id,
        container_revision,
        transition,
    ));
    if page.chrome.visibility == ChromeVisibility::Visible {
        add_title(&mut stack, &presented.container, outer.width, app);
        add_actions(state, &mut stack, &presented.container, app);
    }
    stack.finish()
}

fn add_title(
    stack: &mut Stack,
    container: &ContainerSnapshot,
    width: f32,
    app: &warpui::AppContext,
) {
    let Some(title) = container
        .active_page()
        .and_then(|page| page.chrome.title.clone())
    else {
        return;
    };
    let page = container.active_page().expect("container page exists");
    let leading_actions = page
        .chrome
        .leading
        .iter()
        .map(|group| group.actions.len())
        .sum::<usize>();
    let trailing_actions = page
        .chrome
        .trailing
        .iter()
        .map(|group| group.actions.len())
        .sum::<usize>();
    let leading = 8.0 + leading_actions as f32 * (ACTION_SIZE + ACTION_GAP);
    let trailing = HOST_CLOSE_RESERVE
        + trailing_actions as f32 * (ACTION_SIZE + ACTION_GAP)
        + TITLE_HORIZONTAL_PADDING;
    let title = Text::new(title, Appearance::as_ref(app).ui_font_family(), 11.0)
        .soft_wrap(false)
        .with_color(rgba(container_palette(container, app).text));
    stack.add_positioned_overlay_child(
        ConstrainedBox::new(
            Flex::row()
                .with_child(title.finish())
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::Start)
                .with_main_axis_size(MainAxisSize::Max)
                .finish(),
        )
        .with_width((width - leading - trailing).max(1.0))
        .with_height(PLUGIN_WINDOW_TITLEBAR_HEIGHT)
        .finish(),
        OffsetPositioning::offset_from_parent(
            vec2f(leading, 0.0),
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        ),
    );
}

fn add_actions(
    state: &PluginHoverState,
    stack: &mut Stack,
    container: &ContainerSnapshot,
    app: &warpui::AppContext,
) {
    let Some(page) = container.active_page() else {
        return;
    };
    let mut leading_x = 4.0;
    for action in page.chrome.leading.iter().flat_map(|group| &group.actions) {
        let element = action_button(state, container, action, app);
        add_leading(stack, element, leading_x);
        leading_x += ACTION_SIZE + ACTION_GAP;
    }
    let mut trailing_x = HOST_CLOSE_RESERVE;
    for action in page
        .chrome
        .trailing
        .iter()
        .flat_map(|group| &group.actions)
        .rev()
    {
        let element = action_button(state, container, action, app);
        stack.add_positioned_overlay_child(
            element,
            OffsetPositioning::offset_from_parent(
                vec2f(-trailing_x, 3.0),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
        trailing_x += ACTION_SIZE + ACTION_GAP;
    }
}

fn action_button(
    state: &PluginHoverState,
    container: &ContainerSnapshot,
    action: &ChromeAction,
    app: &warpui::AppContext,
) -> Box<dyn Element> {
    let id = container.container_id;
    let page_id = container.active_page_id;
    let revision = container.revision;
    let action_id = action.id.clone();
    let mouse_state = state.action_mouse_state(format!("{id}:{page_id}:{action_id}"));
    let position_id = format!("warpeer-plugin-chrome-action-{id}-{page_id}-{action_id}");
    let mut button = icon_button(
        mouse_state.clone(),
        semantic_icon(&action.icon),
        container_palette(container, app),
        action.selected,
        action.enabled,
    );
    if !action.enabled {
        button = button.disable();
    }
    let label = action.label.clone();
    let menu = action.menu.clone();
    let action_position_id = position_id.clone();
    let button = button.on_click(move |ctx, _, _| {
        if menu.is_empty() {
            ctx.dispatch_typed_action(super::TerminalAction::ActivatePluginChromeAction {
                container_id: id,
                page_id,
                revision,
                action_id: action_id.clone(),
                menu_item_id: None,
                label: label.clone(),
                source: ChromeActionSource::Pointer,
            });
        } else {
            ctx.dispatch_typed_action(super::TerminalAction::OpenPluginChromeMenu {
                container_id: id,
                page_id,
                revision,
                action_id: action_id.clone(),
                label: label.clone(),
                position_id: action_position_id.clone(),
                items: menu.clone(),
            });
        }
    });
    let tooltip = action
        .tooltip
        .clone()
        .unwrap_or_else(|| action.label.clone());
    Appearance::as_ref(app)
        .ui_builder()
        .overlay_tool_tip_on_element(
            tooltip,
            mouse_state,
            SavePosition::new(button.finish(), &position_id).finish(),
            ParentAnchor::BottomLeft,
            ChildAnchor::TopLeft,
            vec2f(0.0, 4.0),
        )
}

fn icon_button(
    mouse_state: warpui::elements::MouseStateHandle,
    icon: Icon,
    palette: warpeer_plugin_protocol::SurfacePalette,
    selected: bool,
    enabled: bool,
) -> Hoverable {
    let text = rgba(if enabled {
        palette.text
    } else {
        palette.muted_text
    });
    let hover = rgba(if selected {
        palette.accent
    } else {
        palette.surface
    });
    Hoverable::new(mouse_state, move |state| {
        let icon = ConstrainedBox::new(icon.to_warpui_icon(Fill::Solid(text)).finish())
            .with_width(12.0)
            .with_height(12.0)
            .finish();
        let mut body = Container::new(icon)
            .with_padding_left(3.0)
            .with_padding_top(3.0)
            .with_padding_right(3.0)
            .with_padding_bottom(3.0);
        if selected || state.is_mouse_over_element() || state.is_clicked() {
            body = body.with_background(Fill::Solid(hover));
        }
        ConstrainedBox::new(body.finish())
            .with_width(ACTION_SIZE)
            .with_height(ACTION_SIZE)
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
}

fn add_leading(stack: &mut Stack, element: Box<dyn Element>, x: f32) {
    stack.add_positioned_overlay_child(
        element,
        OffsetPositioning::offset_from_parent(
            vec2f(x, 3.0),
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        ),
    );
}

fn semantic_icon(name: &str) -> Icon {
    match name {
        "image" => Icon::Image,
        "settings" | "gear" => Icon::Settings,
        "more" | "ellipsis" => Icon::DotsHorizontal,
        "close" => Icon::X,
        "back" => Icon::ArrowLeft,
        _ => Icon::Info,
    }
}

fn viewport_bounds(viewport: [f32; 4]) -> SurfaceBounds {
    SurfaceBounds {
        x: viewport[0],
        y: viewport[1],
        width: viewport[2],
        height: viewport[3],
        scale: 1.0,
    }
}

fn container_palette(
    _container: &ContainerSnapshot,
    app: &warpui::AppContext,
) -> warpeer_plugin_protocol::SurfacePalette {
    let theme = Appearance::as_ref(app).theme();
    warpeer_plugin_protocol::SurfacePalette {
        background: color_array(theme.background().into_solid()),
        surface: color_array(theme.surface_1().into_solid()),
        text: color_array(theme.main_text_color(theme.surface_1()).into_solid()),
        muted_text: color_array(theme.sub_text_color(theme.surface_1()).into_solid()),
        border: color_array(theme.outline().into_solid()),
        accent: color_array(theme.accent().into_solid()),
    }
}

fn color_array(color: ColorU) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn rgba(value: [u8; 4]) -> ColorU {
    ColorU::new(value[0], value[1], value[2], value[3])
}
