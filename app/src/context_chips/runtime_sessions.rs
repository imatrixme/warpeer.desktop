use serde::Deserialize;
use warp_completer::completer::GeneratorContext as _;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill;
use warp_core::ui::theme::color::internal_colors;
use warp_terminal::shell::ShellType;
use warpui::r#async::SpawnedFutureHandle;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, DispatchEventResult, EventHandler, Hoverable,
    MouseStateHandle, Radius,
};
use warpui::platform::Cursor;
use warpui::{AppContext, Element, Entity, ModelContext, SingletonEntity};

use super::context_chip::{
    ChipFingerprintInput, ChipRuntimePolicy, ContextChip, GeneratorContext, RefreshConfig,
};
use super::display_menu::{DisplayChipMenuAction, GenericMenuItem};
use super::{ChipValue, ContextChipKind};
use crate::appearance::Appearance;
use crate::completer::SessionContext;
use crate::terminal::model::session::command_executor::shell_quote_arg;
use crate::ui_components::icons::Icon;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeKind {
    Tmux,
    Herdr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeSessionFilter {
    All,
    Tmux,
    Herdr,
}

impl RuntimeSessionFilter {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Tmux => "Tmux",
            Self::Herdr => "Herdr",
        }
    }

    fn runtime(self) -> Option<RuntimeKind> {
        match self {
            Self::All => None,
            Self::Tmux => Some(RuntimeKind::Tmux),
            Self::Herdr => Some(RuntimeKind::Herdr),
        }
    }
}

impl RuntimeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            Self::Herdr => "Herdr",
        }
    }

    pub(crate) fn executable(self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            Self::Herdr => "herdr",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Self::Tmux => Icon::Tmux,
            Self::Herdr => Icon::Herdr,
        }
    }

    fn list_command(self) -> &'static str {
        match self {
            Self::Tmux => {
                "tmux list-sessions -F '#{session_name}|#{session_windows}|#{session_attached}'"
            }
            Self::Herdr => "herdr session list --json",
        }
    }

    pub(crate) fn from_executable(executable: &str) -> Option<Self> {
        match executable.rsplit('/').next()? {
            "tmux" => Some(Self::Tmux),
            "herdr" => Some(Self::Herdr),
            _ => None,
        }
    }

    pub(crate) fn session_name_from_command(self, command: &str) -> Option<String> {
        let tokens = shell_words::split(command).ok()?;
        let executable_index = tokens
            .iter()
            .position(|token| Self::from_executable(token) == Some(self))?;
        let args = &tokens[executable_index + 1..];

        match self {
            Self::Tmux => option_value(args, &["-t", "-s"]),
            Self::Herdr => option_value(args, &["--session"]).or_else(|| {
                args.windows(3)
                    .find(|parts| parts[0] == "session" && parts[1] == "attach")
                    .map(|parts| parts[2].clone())
            }),
        }
    }

    fn termination_command(
        self,
        session_name: &str,
        is_running: bool,
        shell_type: ShellType,
    ) -> String {
        let session_name = shell_quote_arg(session_name, shell_type);
        match (self, is_running) {
            (Self::Tmux, _) => format!("tmux kill-session -t {session_name}"),
            (Self::Herdr, true) => format!(
                "herdr session stop --json {session_name} && herdr session delete --json {session_name}"
            ),
            (Self::Herdr, false) => {
                format!("herdr session delete --json {session_name}")
            }
        }
    }
}

pub(crate) fn context_chip() -> ContextChip {
    let runtime_policy = ChipRuntimePolicy::new(
        std::iter::empty::<&str>(),
        false,
        None,
        [
            ChipFingerprintInput::SessionId,
            ChipFingerprintInput::SessionIsLocal,
            ChipFingerprintInput::ExternalCommandsState,
        ],
    )
    .with_hide_when_unavailable();
    ContextChip::builtin_with_runtime_policy(
        "tty sessions",
        tty_value,
        RefreshConfig::OnDemandOnly,
        runtime_policy,
    )
}

pub(crate) fn prepend_runtime_chips(chips: &mut Vec<ContextChipKind>) {
    if crate::product_profile::is_terminal_only() {
        chips.insert(0, ContextChipKind::Tty);
    }
}

fn tty_value(context: &GeneratorContext) -> Option<ChipValue> {
    let session = context.active_session?;
    if !session.has_loaded_external_commands()
        || !session
            .executable_names()
            .any(|executable| RuntimeKind::from_executable(executable).is_some())
    {
        return None;
    }
    Some(ChipValue::Text("tty".to_string()))
}

fn option_value(args: &[String], options: &[&str]) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if options.contains(&arg.as_str()) {
            return args.get(index + 1).cloned();
        }
        for option in options {
            if let Some(value) = arg
                .strip_prefix(option)
                .map(|value| value.strip_prefix('=').unwrap_or(value))
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) struct RuntimeSessionFetcher {
    cached_sessions: Vec<RuntimeSessionItem>,
    available_runtimes: Vec<RuntimeKind>,
    session_context: Option<SessionContext>,
    fetch_handle: Option<SpawnedFutureHandle>,
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeSessionFetcherEvent {
    SessionsUpdated,
}

impl RuntimeSessionFetcher {
    pub(crate) fn new(
        session_context: Option<SessionContext>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let mut fetcher = Self {
            cached_sessions: Vec::new(),
            available_runtimes: Vec::new(),
            session_context,
            fetch_handle: None,
        };
        fetcher.refetch(ctx);
        fetcher
    }

    pub(crate) fn refetch(&mut self, ctx: &mut ModelContext<Self>) {
        if self.fetch_handle.is_some() {
            return;
        }
        let Some(session_context) = self.session_context.clone() else {
            return;
        };
        let available_runtimes = available_runtimes(&session_context);
        self.fetch_handle = Some(ctx.spawn(
            async move {
                let mut sessions = Vec::new();
                for runtime in &available_runtimes {
                    let Ok(output) = session_context
                        .execute_command_at_pwd(runtime.list_command(), None)
                        .await
                    else {
                        continue;
                    };
                    if !output.success() {
                        continue;
                    }
                    let Ok(output) = output.to_string() else {
                        continue;
                    };
                    sessions.extend(parse_runtime_sessions(*runtime, &output));
                }
                (available_runtimes, sessions)
            },
            |fetcher, (available_runtimes, sessions), ctx| {
                fetcher.fetch_handle = None;
                fetcher.available_runtimes = available_runtimes;
                fetcher.cached_sessions = sessions;
                ctx.emit(RuntimeSessionFetcherEvent::SessionsUpdated);
                ctx.notify();
            },
        ));
    }

    pub(crate) fn update_session_context(
        &mut self,
        session_context: Option<SessionContext>,
        ctx: &mut ModelContext<Self>,
    ) {
        if let Some(handle) = self.fetch_handle.take() {
            handle.abort();
        }
        self.session_context = session_context;
        self.refetch(ctx);
    }

    pub(crate) fn cached_sessions(&self) -> &[RuntimeSessionItem] {
        &self.cached_sessions
    }

    pub(crate) fn available_runtimes(&self) -> &[RuntimeKind] {
        &self.available_runtimes
    }
}

impl Entity for RuntimeSessionFetcher {
    type Event = RuntimeSessionFetcherEvent;
}

impl Drop for RuntimeSessionFetcher {
    fn drop(&mut self) {
        if let Some(handle) = self.fetch_handle.take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSessionItem {
    runtime: RuntimeKind,
    session_name: String,
    display_name: String,
    show_runtime_label: bool,
    is_running: bool,
    close_mouse_state: MouseStateHandle,
}

impl RuntimeSessionItem {
    pub(crate) fn runtime(&self) -> RuntimeKind {
        self.runtime
    }

    pub(crate) fn session_name(&self) -> &str {
        &self.session_name
    }

    pub(crate) fn termination_command(&self, shell_type: ShellType) -> String {
        self.runtime
            .termination_command(&self.session_name, self.is_running, shell_type)
    }

    fn with_runtime_label(mut self, show_runtime_label: bool) -> Self {
        self.show_runtime_label = show_runtime_label;
        self
    }
}

impl PartialEq for RuntimeSessionItem {
    fn eq(&self, other: &Self) -> bool {
        self.runtime == other.runtime
            && self.session_name == other.session_name
            && self.display_name == other.display_name
            && self.show_runtime_label == other.show_runtime_label
            && self.is_running == other.is_running
    }
}

impl Eq for RuntimeSessionItem {}

impl GenericMenuItem for RuntimeSessionItem {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> String {
        if self.show_runtime_label {
            format!("{} - {}", self.runtime.label(), self.display_name)
        } else {
            self.display_name.clone()
        }
    }

    fn icon(&self, _app: &AppContext) -> Option<Icon> {
        Some(self.runtime.icon())
    }

    fn action_data(&self) -> String {
        self.session_name.clone()
    }

    fn right_side_element(&self, index: usize, app: &AppContext) -> Option<Box<dyn Element>> {
        let theme = Appearance::as_ref(app).theme();
        let meta_color = theme.sub_text_color(theme.surface_2()).into_solid();
        let idle_color = coloru_with_opacity(meta_color, 140);
        let button = Hoverable::new(self.close_mouse_state.clone(), move |state| {
            let mut container = Container::new(
                ConstrainedBox::new(
                    Icon::X
                        .to_warpui_icon(Fill::Solid(if state.is_hovered() {
                            meta_color
                        } else {
                            idle_color
                        }))
                        .finish(),
                )
                .with_width(10.)
                .with_height(10.)
                .finish(),
            )
            .with_uniform_padding(3.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
            if state.is_hovered() {
                container = container.with_background(internal_colors::fg_overlay_2(theme));
            }
            container.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        Some(
            EventHandler::new(button)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DisplayChipMenuAction::SelectItemSecondary { index });
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
    }
}

fn parse_runtime_sessions(runtime: RuntimeKind, output: &str) -> Vec<RuntimeSessionItem> {
    match runtime {
        RuntimeKind::Tmux => parse_tmux_sessions(output),
        RuntimeKind::Herdr => parse_herdr_sessions(output),
    }
}

pub(crate) fn filter_runtime_sessions(
    sessions: &[RuntimeSessionItem],
    filter: RuntimeSessionFilter,
) -> Vec<RuntimeSessionItem> {
    sessions
        .iter()
        .filter(|session| {
            filter
                .runtime()
                .is_none_or(|runtime| session.runtime == runtime)
        })
        .cloned()
        .map(|session| session.with_runtime_label(filter == RuntimeSessionFilter::All))
        .collect()
}

pub(crate) fn runtime_filters_for(available_runtimes: &[RuntimeKind]) -> Vec<RuntimeSessionFilter> {
    if available_runtimes.len() > 1 {
        return vec![
            RuntimeSessionFilter::All,
            RuntimeSessionFilter::Tmux,
            RuntimeSessionFilter::Herdr,
        ];
    }
    match available_runtimes.first() {
        Some(RuntimeKind::Tmux) => vec![RuntimeSessionFilter::Tmux],
        Some(RuntimeKind::Herdr) => vec![RuntimeSessionFilter::Herdr],
        None => vec![RuntimeSessionFilter::All],
    }
}

fn available_runtimes(session_context: &SessionContext) -> Vec<RuntimeKind> {
    let session = &session_context.session;
    if !session.has_loaded_external_commands() {
        return Vec::new();
    }
    [RuntimeKind::Tmux, RuntimeKind::Herdr]
        .into_iter()
        .filter(|runtime| {
            session
                .executable_names()
                .any(|executable| runtime.executable() == executable)
        })
        .collect()
}

fn parse_tmux_sessions(output: &str) -> Vec<RuntimeSessionItem> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '|');
            let session_name = fields.next()?.trim();
            let windows = fields.next()?.trim().parse::<usize>().ok()?;
            let attached = fields.next()?.trim().parse::<usize>().ok()?;
            let window_label = if windows == 1 { "window" } else { "windows" };
            let status = if attached > 0 { "attached" } else { "detached" };
            Some(RuntimeSessionItem {
                runtime: RuntimeKind::Tmux,
                session_name: session_name.to_string(),
                display_name: format!("{session_name} - {windows} {window_label}, {status}"),
                show_runtime_label: false,
                is_running: true,
                close_mouse_state: Default::default(),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct HerdrSessionList {
    sessions: Vec<HerdrSession>,
}

#[derive(Deserialize)]
struct HerdrSession {
    name: String,
    running: bool,
    #[serde(default)]
    default: bool,
}

fn parse_herdr_sessions(output: &str) -> Vec<RuntimeSessionItem> {
    let Ok(list) = serde_json::from_str::<HerdrSessionList>(output) else {
        return Vec::new();
    };
    list.sessions
        .into_iter()
        .map(|session| {
            let status = if session.running {
                "running"
            } else {
                "stopped"
            };
            let default = if session.default { ", default" } else { "" };
            RuntimeSessionItem {
                runtime: RuntimeKind::Herdr,
                session_name: session.name.clone(),
                display_name: format!("{} - {status}{default}", session.name),
                show_runtime_label: false,
                is_running: session.running,
                close_mouse_state: Default::default(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "runtime_sessions_tests.rs"]
mod tests;
