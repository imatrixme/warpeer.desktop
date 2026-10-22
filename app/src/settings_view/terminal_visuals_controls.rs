use settings::Setting as _;
use warp_errors::report_if_error;
use warpui::elements::Element;
use warpui::ui_components::components::UiComponent;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{AppContext, SingletonEntity as _, ViewContext, ViewHandle};

use super::settings_page::{
    LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item, render_dropdown_item,
};
use super::terminal_visuals_page::{TerminalVisualsPageAction, TerminalVisualsSettingsPageView};
use super::terminal_visuals_spread_control::particle_spread_widget;
use crate::appearance::Appearance;
use crate::settings::{
    BlockFoldMotion, CommitMotion, CursorMotion, EraseMotion, KeypressMotion, NewLineMotion,
    ReducedMotionPolicy, TerminalMaterial, TerminalVisualSettings,
};
use crate::view_components::{Dropdown, DropdownItem};

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalVisualAction {
    ToggleEnabled,
    SetDuration(f32),
    SetReducedMotion(ReducedMotionPolicy),
    SetNewLine(NewLineMotion),
    SetCommit(CommitMotion),
    SetKeypress(KeypressMotion),
    SetCursor(CursorMotion),
    SetErase(EraseMotion),
    SetParticleSpread(f32),
    SetBlockFold(BlockFoldMotion),
    ToggleScrollGlide,
    SetMaterial(TerminalMaterial),
    ToggleSubstrate,
    ToggleCommandGutter,
    ToggleBlockDepth,
}

pub fn handle_action(
    action: &TerminalVisualAction,
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
) {
    TerminalVisualSettings::handle(ctx).update(ctx, |settings, ctx| {
        macro_rules! set {
            ($field:ident, $value:expr) => {{
                report_if_error!(settings.$field.set_value($value, ctx));
            }};
        }
        match action {
            TerminalVisualAction::ToggleEnabled => set!(enabled, !*settings.enabled),
            TerminalVisualAction::SetDuration(value) => set!(duration_ms, *value),
            TerminalVisualAction::SetReducedMotion(value) => set!(reduced_motion, *value),
            TerminalVisualAction::SetNewLine(value) => set!(new_line_motion, *value),
            TerminalVisualAction::SetCommit(value) => set!(commit_motion, *value),
            TerminalVisualAction::SetKeypress(value) => set!(keypress_motion, *value),
            TerminalVisualAction::SetCursor(value) => set!(cursor_motion, *value),
            TerminalVisualAction::SetErase(value) => set!(erase_motion, *value),
            TerminalVisualAction::SetParticleSpread(value) => set!(
                particle_spread,
                crate::settings::normalized_terminal_particle_spread(*value)
            ),
            TerminalVisualAction::SetBlockFold(value) => set!(block_fold_motion, *value),
            TerminalVisualAction::ToggleScrollGlide => {
                set!(scroll_glide, !*settings.scroll_glide)
            }
            TerminalVisualAction::SetMaterial(value) => set!(material, *value),
            TerminalVisualAction::ToggleSubstrate => {
                set!(substrate_enabled, !*settings.substrate_enabled)
            }
            TerminalVisualAction::ToggleCommandGutter => {
                set!(command_gutter, !*settings.command_gutter)
            }
            TerminalVisualAction::ToggleBlockDepth => {
                set!(block_depth, !*settings.block_depth)
            }
        }
    });
    ctx.notify();
}

struct ToggleWidget {
    label: &'static str,
    description: &'static str,
    search: &'static str,
    value: fn(&AppContext) -> bool,
    action: TerminalVisualAction,
    switch_state: SwitchStateHandle,
}

impl SettingsWidget for ToggleWidget {
    type View = TerminalVisualsSettingsPageView;

    fn search_terms(&self) -> &str {
        self.search
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let action = self.action.clone();
        render_body_item::<TerminalVisualsPageAction>(
            self.label.into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch_state.clone())
                .check((self.value)(app))
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(TerminalVisualsPageAction::Apply(action.clone()));
                })
                .finish(),
            Some(self.description.into()),
        )
    }
}

struct DropdownWidget {
    label: &'static str,
    description: &'static str,
    search: &'static str,
    dropdown: ViewHandle<Dropdown<TerminalVisualsPageAction>>,
}

impl SettingsWidget for DropdownWidget {
    type View = TerminalVisualsSettingsPageView;

    fn search_terms(&self) -> &str {
        self.search
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_dropdown_item(
            appearance,
            self.label,
            Some(self.description),
            None,
            LocalOnlyIconState::Hidden,
            None,
            &self.dropdown,
        )
    }
}

fn dropdown(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
    items: Vec<(&'static str, TerminalVisualAction)>,
    selected: TerminalVisualAction,
) -> ViewHandle<Dropdown<TerminalVisualsPageAction>> {
    ctx.add_typed_action_view(|ctx| {
        let mut dropdown = Dropdown::new(ctx);
        dropdown.set_top_bar_max_width(190.);
        dropdown.set_menu_width(190., ctx);
        dropdown.add_items(
            items
                .into_iter()
                .map(|(label, action)| {
                    DropdownItem::new(label, TerminalVisualsPageAction::Apply(action))
                })
                .collect(),
            ctx,
        );
        dropdown.set_selected_by_action(TerminalVisualsPageAction::Apply(selected), ctx);
        dropdown
    })
}

fn dropdown_widget(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
    label: &'static str,
    description: &'static str,
    search: &'static str,
    items: Vec<(&'static str, TerminalVisualAction)>,
    selected: TerminalVisualAction,
) -> Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>> {
    Box::new(DropdownWidget {
        label,
        description,
        search,
        dropdown: dropdown(ctx, items, selected),
    })
}

fn toggle_widget(
    label: &'static str,
    description: &'static str,
    search: &'static str,
    value: fn(&AppContext) -> bool,
    action: TerminalVisualAction,
) -> Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>> {
    Box::new(ToggleWidget {
        label,
        description,
        search,
        value,
        action,
        switch_state: SwitchStateHandle::default(),
    })
}

macro_rules! choices {
    ($action:path; $($label:literal => $value:path),+ $(,)?) => {
        vec![$(($label, $action($value))),+]
    };
}

pub fn build_motion_widgets(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>>> {
    let (duration, reduced_motion, block_fold, new_line, commit) = {
        let settings = TerminalVisualSettings::as_ref(ctx);
        (
            *settings.duration_ms,
            *settings.reduced_motion,
            *settings.block_fold_motion,
            *settings.new_line_motion,
            *settings.commit_motion,
        )
    };
    let mut widgets = vec![toggle_widget(
        "Terminal visual runtime",
        "Animate editable input and add restrained material depth.",
        "terminal motion animation visual metal",
        |app| *TerminalVisualSettings::as_ref(app).enabled,
        TerminalVisualAction::ToggleEnabled,
    )];
    widgets.push(dropdown_widget(
        ctx,
        "Motion duration",
        "One shared timing control keeps the motion system coherent.",
        "terminal animation duration speed",
        vec![
            ("Fast - 120 ms", TerminalVisualAction::SetDuration(120.)),
            ("Quick - 180 ms", TerminalVisualAction::SetDuration(180.)),
            ("Balanced - 220 ms", TerminalVisualAction::SetDuration(220.)),
            ("Classic - 240 ms", TerminalVisualAction::SetDuration(240.)),
            ("Relaxed - 320 ms", TerminalVisualAction::SetDuration(320.)),
            ("Slow - 480 ms", TerminalVisualAction::SetDuration(480.)),
        ],
        TerminalVisualAction::SetDuration(nearest_duration(duration)),
    ));
    widgets.push(dropdown_widget(
        ctx,
        "Reduced motion",
        "Follow macOS, always reduce motion, or explicitly keep it enabled.",
        "terminal reduced motion accessibility",
        choices!(TerminalVisualAction::SetReducedMotion;
            "Follow macOS" => ReducedMotionPolicy::System,
            "Always reduce" => ReducedMotionPolicy::Always,
            "Never reduce" => ReducedMotionPolicy::Never),
        TerminalVisualAction::SetReducedMotion(reduced_motion),
    ));
    widgets.push(toggle_widget(
        "Scroll glide",
        "Smooth terminal viewport movement while output scrolls.",
        "terminal scroll glide smooth motion",
        |app| *TerminalVisualSettings::as_ref(app).scroll_glide,
        TerminalVisualAction::ToggleScrollGlide,
    ));
    widgets.extend(output_motion_widgets(ctx, block_fold, new_line, commit));
    widgets
}

pub fn build_cursor_widgets(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>>> {
    let (keypress, erase, cursor) = {
        let settings = TerminalVisualSettings::as_ref(ctx);
        (
            *settings.keypress_motion,
            *settings.erase_motion,
            *settings.cursor_motion,
        )
    };
    cursor_widgets(ctx, keypress, erase, cursor)
}

pub fn build_material_widgets(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>>> {
    let material = *TerminalVisualSettings::as_ref(ctx).material;
    let mut widgets = vec![dropdown_widget(
        ctx,
        "Terminal material",
        "Tint the existing window glass without replacing the terminal theme.",
        "terminal material glass depth",
        choices!(TerminalVisualAction::SetMaterial;
            "Flat" => TerminalMaterial::Flat,
            "Smoked glass" => TerminalMaterial::SmokedGlass,
            "Obsidian" => TerminalMaterial::Obsidian,
            "Frosted" => TerminalMaterial::Frosted,
            "Carbon" => TerminalMaterial::Carbon),
        TerminalVisualAction::SetMaterial(material),
    )];
    widgets.extend([
        toggle_widget(
            "Material substrate",
            "Render the theme-derived terminal substrate.",
            "terminal substrate",
            |app| *TerminalVisualSettings::as_ref(app).substrate_enabled,
            TerminalVisualAction::ToggleSubstrate,
        ),
        toggle_widget(
            "Command gutter",
            "Add a restrained guide beside command blocks.",
            "terminal command gutter",
            |app| *TerminalVisualSettings::as_ref(app).command_gutter,
            TerminalVisualAction::ToggleCommandGutter,
        ),
        toggle_widget(
            "Command block depth",
            "Separate command blocks with subtle ambient depth.",
            "terminal block depth",
            |app| *TerminalVisualSettings::as_ref(app).block_depth,
            TerminalVisualAction::ToggleBlockDepth,
        ),
    ]);
    widgets
}

fn nearest_duration(value: f32) -> f32 {
    [120., 180., 220., 240., 320., 480.]
        .into_iter()
        .min_by(|left, right| (value - left).abs().total_cmp(&(value - right).abs()))
        .unwrap_or(240.)
}

fn output_motion_widgets(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
    block_fold: BlockFoldMotion,
    new_line: NewLineMotion,
    commit: CommitMotion,
) -> Vec<Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>>> {
    vec![
        dropdown_widget(
            ctx,
            "Block fold",
            "Animation used when a command output block is collapsed or expanded.",
            "terminal block fold motion",
            choices!(TerminalVisualAction::SetBlockFold; "Off" => BlockFoldMotion::Off, "Height" => BlockFoldMotion::Height, "Stagger" => BlockFoldMotion::Stagger, "Wipe" => BlockFoldMotion::Wipe, "Converge" => BlockFoldMotion::Converge, "Seam" => BlockFoldMotion::Seam, "Drain" => BlockFoldMotion::Drain),
            TerminalVisualAction::SetBlockFold(block_fold),
        ),
        dropdown_widget(
            ctx,
            "New line",
            "Animation used when terminal output arrives on a new line.",
            "terminal new line output motion",
            choices!(TerminalVisualAction::SetNewLine; "Off" => NewLineMotion::Off, "Slide" => NewLineMotion::Slide, "Stagger" => NewLineMotion::Stagger, "Snap" => NewLineMotion::Snap, "Glow" => NewLineMotion::Glow, "Focus" => NewLineMotion::Focus, "Recede" => NewLineMotion::Recede),
            TerminalVisualAction::SetNewLine(new_line),
        ),
        dropdown_widget(
            ctx,
            "Commit",
            "Animation used when editable input becomes a committed command row.",
            "terminal command commit motion",
            choices!(TerminalVisualAction::SetCommit; "Off" => CommitMotion::Off, "Carry" => CommitMotion::Carry, "Arc" => CommitMotion::Arc, "Lean" => CommitMotion::Lean, "Ripple" => CommitMotion::Ripple, "Streak" => CommitMotion::Streak, "Quantize" => CommitMotion::Quantize),
            TerminalVisualAction::SetCommit(commit),
        ),
    ]
}

fn cursor_widgets(
    ctx: &mut ViewContext<TerminalVisualsSettingsPageView>,
    keypress: KeypressMotion,
    erase: EraseMotion,
    cursor: CursorMotion,
) -> Vec<Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>>> {
    let mut widgets = vec![
        dropdown_widget(
            ctx,
            "Keypress",
            "Animation for committed grapheme clusters; IME composition is excluded.",
            "terminal keypress text motion ime",
            choices!(TerminalVisualAction::SetKeypress; "Off" => KeypressMotion::Off, "Fade" => KeypressMotion::Fade, "Rise" => KeypressMotion::Rise, "Pop" => KeypressMotion::Pop, "Extrude" => KeypressMotion::Extrude, "Heat" => KeypressMotion::Heat, "Echo" => KeypressMotion::Echo, "Drop" => KeypressMotion::Drop, "Ink" => KeypressMotion::Ink, "Squeeze" => KeypressMotion::Squeeze),
            TerminalVisualAction::SetKeypress(keypress),
        ),
        dropdown_widget(
            ctx,
            "Erase",
            "Particle shape, motion, and color used when a character is erased.",
            "terminal erase delete particle flame sand water ripple",
            choices!(TerminalVisualAction::SetErase;
                "Off" => EraseMotion::Off,
                "Shatter" => EraseMotion::Shatter,
                "Flame" => EraseMotion::Flame,
                "Sandstorm" => EraseMotion::Sandstorm,
                "Droplet" => EraseMotion::Droplet,
                "Space ripple" => EraseMotion::SpaceRipple,
                "Ash" => EraseMotion::Ash,
                "Crystal" => EraseMotion::Crystal,
                "Ink bloom" => EraseMotion::InkBloom,
                "Vapor" => EraseMotion::Vapor,
                "Pixel dust" => EraseMotion::PixelDust),
            TerminalVisualAction::SetErase(visible_erase_motion(erase)),
        ),
    ];
    widgets.push(particle_spread_widget());
    widgets.push(dropdown_widget(
        ctx,
        "Cursor motion",
        "Cursor retargeting and trail style.",
        "terminal cursor motion caret",
        choices!(TerminalVisualAction::SetCursor; "Snap" => CursorMotion::Snap, "Ease" => CursorMotion::Ease, "Spring" => CursorMotion::Spring, "Smear" => CursorMotion::Smear, "Squash" => CursorMotion::Squash, "Phosphor" => CursorMotion::Phosphor, "Arc" => CursorMotion::Arc),
        TerminalVisualAction::SetCursor(visible_cursor_motion(cursor)),
    ));
    widgets
}

fn visible_erase_motion(value: EraseMotion) -> EraseMotion {
    match value {
        EraseMotion::Off
        | EraseMotion::Shatter
        | EraseMotion::Flame
        | EraseMotion::Sandstorm
        | EraseMotion::Droplet
        | EraseMotion::SpaceRipple
        | EraseMotion::Ash
        | EraseMotion::Crystal
        | EraseMotion::InkBloom
        | EraseMotion::Vapor
        | EraseMotion::PixelDust => value,
        _ => EraseMotion::Shatter,
    }
}

fn visible_cursor_motion(value: CursorMotion) -> CursorMotion {
    match value {
        CursorMotion::Warp | CursorMotion::Beam => CursorMotion::Ease,
        _ => value,
    }
}
