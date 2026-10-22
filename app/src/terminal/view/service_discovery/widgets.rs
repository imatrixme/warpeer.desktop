use warpui::Element;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    Empty, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;

use super::ServiceDiscoveryAction;
use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;

pub(super) fn service_row(
    title: &str,
    detail: &str,
    action_label: &str,
    action: ServiceDiscoveryAction,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let content = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(
            Text::new_inline(title.to_string(), appearance.ui_font_family(), 11.)
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.main_text_color(theme.surface_2()).into())
                .with_clip(ClipConfig::ellipsis())
                .finish(),
        )
        .with_child(
            Text::new_inline(detail.to_string(), appearance.ui_font_family(), 9.)
                .with_color(theme.sub_text_color(theme.surface_2()).into())
                .with_clip(ClipConfig::ellipsis())
                .finish(),
        )
        .finish();
    Container::new(
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., content).finish())
            .with_child(
                Container::new(action_button(action_label, None, action, appearance))
                    .with_margin_left(8.)
                    .finish(),
            )
            .finish(),
    )
    .with_background(theme.surface_2().with_opacity(70))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.)))
    .with_uniform_padding(8.)
    .with_margin_bottom(4.)
    .finish()
}

pub(super) fn info_row(
    title: &str,
    detail: &str,
    value: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let content = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(
            Text::new_inline(title.to_string(), appearance.ui_font_family(), 10.)
                .with_color(theme.main_text_color(theme.surface_1()).into())
                .with_clip(ClipConfig::ellipsis())
                .finish(),
        )
        .with_child(
            Text::new_inline(detail.to_string(), appearance.ui_font_family(), 9.)
                .with_color(theme.sub_text_color(theme.surface_1()).into())
                .with_clip(ClipConfig::ellipsis())
                .finish(),
        )
        .finish();
    Container::new(
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., content).finish())
            .with_child(
                Container::new(metadata_text(value, appearance))
                    .with_margin_left(8.)
                    .finish(),
            )
            .finish(),
    )
    .with_vertical_padding(5.)
    .finish()
}

pub(super) fn section_header(
    title: &str,
    count: usize,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Text::new_inline(title.to_string(), appearance.ui_font_family(), 11.)
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.main_text_color(theme.surface_1()).into())
                .finish(),
        )
        .with_child(Expanded::new(1., Empty::new().finish()).finish())
        .with_child(metadata_text(&count.to_string(), appearance))
        .finish()
}

pub(super) fn action_button(
    label: &str,
    icon: Option<Icon>,
    action: ServiceDiscoveryAction,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let color = theme.main_text_color(theme.surface_2());
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);
    if let Some(icon) = icon {
        row.add_child(
            ConstrainedBox::new(icon.to_warpui_icon(color).finish())
                .with_width(12.)
                .with_height(12.)
                .finish(),
        );
    }
    if !label.is_empty() {
        row.add_child(
            Container::new(
                Text::new_inline(label.to_string(), appearance.ui_font_family(), 10.)
                    .with_color(color.into())
                    .finish(),
            )
            .with_margin_left(icon.is_some().then_some(4.).unwrap_or_default())
            .finish(),
        );
    }
    let button = Container::new(row.finish())
        .with_background(theme.surface_2().with_opacity(84))
        .with_border(Border::all(1.).with_border_color(theme.surface_3().into()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_horizontal_padding(7.)
        .with_vertical_padding(5.)
        .finish();
    EventHandler::new(button)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

pub(super) fn metadata_text(value: &str, appearance: &Appearance) -> Box<dyn Element> {
    Text::new_inline(value.to_string(), appearance.ui_font_family(), 9.)
        .with_color(
            appearance
                .theme()
                .sub_text_color(appearance.theme().surface_1())
                .into(),
        )
        .finish()
}

pub(super) fn empty_state(message: &str, appearance: &Appearance) -> Box<dyn Element> {
    Container::new(metadata_text(message, appearance))
        .with_vertical_padding(18.)
        .finish()
}

pub(super) fn empty_row(message: &str, appearance: &Appearance) -> Box<dyn Element> {
    Container::new(metadata_text(message, appearance))
        .with_vertical_padding(7.)
        .finish()
}

pub(super) fn section_gap() -> Box<dyn Element> {
    ConstrainedBox::new(Empty::new().finish())
        .with_height(10.)
        .finish()
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.;
    const MIB: f64 = KIB * 1024.;
    const GIB: f64 = MIB * 1024.;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}
