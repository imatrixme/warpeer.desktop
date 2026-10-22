use std::path::PathBuf;
use std::time::{Duration, Instant};

use warpui::r#async::SpawnedFutureHandle;
use warpui::clipboard::ClipboardContent;
use warpui::elements::{
    ClippedScrollStateHandle, CornerRadius, Fill as UiFill, MouseStateHandle, Radius,
};
use warpui::ui_components::components::UiComponentStyles;
use warpui::ui_components::segmented_control::{
    LabelConfig, RenderableOptionConfig, SegmentedControl, SegmentedControlEvent,
};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
    WeakViewHandle,
};

use self::collector::{CollectionOptions, ServiceSnapshot};
use self::managed::{
    DockerAction, ManagedServiceController, ManagedServiceId, ManagedServiceSnapshot,
};
use crate::appearance::Appearance;
use crate::service_discovery_settings::{ServiceDiscoverySettings, is_service_discovery_available};
use crate::terminal::view::TerminalView;
use crate::util::file::external_editor::{Editor, open_file_path_with_editor};
use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;

#[path = "service_discovery/collector.rs"]
mod collector;

#[path = "service_discovery/managed.rs"]
mod managed;

#[path = "service_discovery/ui.rs"]
mod ui;

#[path = "service_discovery/server_ui.rs"]
mod server_ui;

#[path = "service_discovery/widgets.rs"]
mod widgets;

const SNAPSHOT_TTL: Duration = Duration::from_secs(4);
pub(super) const COMPACT_WIDTH: f32 = 132.;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectPanelTab {
    General,
    Server,
}

impl ProjectPanelTab {
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Server => "Server",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ServiceDiscoveryAction {
    SetCompactHovered(bool),
    SetPanelHovered(bool),
    TogglePinned,
    Dismiss,
    Refresh,
    OpenFinder,
    OpenEditor,
    CopyPath,
    OpenPort(u16),
    StartStatic,
    StartScript(String),
    StopManaged(ManagedServiceId),
    RunDocker(DockerAction),
}

pub(crate) struct ServiceDiscoveryView {
    terminal_view: WeakViewHandle<TerminalView>,
    tab_control: warpui::ViewHandle<SegmentedControl<ProjectPanelTab>>,
    selected_tab: ProjectPanelTab,
    snapshot: Option<ServiceSnapshot>,
    last_refresh: Option<Instant>,
    refresh_handle: Option<SpawnedFutureHandle>,
    action_handle: Option<SpawnedFutureHandle>,
    managed: ManagedServiceController,
    managed_snapshots: Vec<ManagedServiceSnapshot>,
    compact_hovered: bool,
    panel_hovered: bool,
    pinned: bool,
    loading: bool,
    action_busy: bool,
    compact_mouse_state: MouseStateHandle,
    panel_mouse_state: MouseStateHandle,
    scroll_state: ClippedScrollStateHandle,
}

impl ServiceDiscoveryView {
    pub(crate) fn new(
        terminal_view: WeakViewHandle<TerminalView>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let tabs = if server_tab_enabled(ctx) {
            vec![ProjectPanelTab::General, ProjectPanelTab::Server]
        } else {
            vec![ProjectPanelTab::General]
        };
        let tab_control = ctx.add_typed_action_view(move |ctx| {
            SegmentedControl::new(
                tabs,
                |tab, selected, app| {
                    let appearance = Appearance::as_ref(app);
                    let theme = appearance.theme();
                    let text_color = theme.main_text_color(theme.surface_2()).into_solid();
                    Some(RenderableOptionConfig {
                        icon_path: "",
                        icon_color: text_color,
                        label: Some(LabelConfig {
                            label: tab.label().into(),
                            width_override: Some(84.),
                            color: text_color,
                        }),
                        tooltip: None,
                        background: if selected {
                            UiFill::Solid(theme.surface_3().into())
                        } else {
                            UiFill::None
                        },
                    })
                },
                ProjectPanelTab::General,
                tab_styles(ctx),
            )
        });
        ctx.subscribe_to_view(&tab_control, |view, _, event, ctx| {
            let SegmentedControlEvent::OptionSelected(tab) = event;
            view.selected_tab = *tab;
            ctx.notify();
        });
        ctx.observe(&ServiceDiscoverySettings::handle(ctx), |view, _, ctx| {
            if !is_service_discovery_available() || !*ServiceDiscoverySettings::as_ref(ctx).enabled
            {
                view.disable(ctx);
            } else {
                if !*ServiceDiscoverySettings::as_ref(ctx).enable_server_controls {
                    view.managed.stop_all();
                    view.managed_snapshots.clear();
                }
                view.last_refresh = None;
                view.sync_tab_options(ctx);
                if view.is_visible() {
                    view.refresh(true, ctx);
                }
            }
            ctx.notify();
        });
        Self {
            terminal_view,
            tab_control,
            selected_tab: ProjectPanelTab::General,
            snapshot: None,
            last_refresh: None,
            refresh_handle: None,
            action_handle: None,
            managed: Default::default(),
            managed_snapshots: Vec::new(),
            compact_hovered: false,
            panel_hovered: false,
            pinned: false,
            loading: false,
            action_busy: false,
            compact_mouse_state: Default::default(),
            panel_mouse_state: Default::default(),
            scroll_state: Default::default(),
        }
    }

    fn current_directory(&self, app: &AppContext) -> Option<PathBuf> {
        self.terminal_view
            .upgrade(app)
            .and_then(|terminal| terminal.as_ref(app).active_session_path_if_local(app))
    }

    fn is_visible(&self) -> bool {
        self.compact_hovered || self.panel_hovered || self.pinned
    }

    fn disable(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(handle) = self.refresh_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.action_handle.take() {
            handle.abort();
        }
        self.managed.stop_all();
        self.managed_snapshots.clear();
        self.snapshot = None;
        self.last_refresh = None;
        self.loading = false;
        self.action_busy = false;
        self.compact_hovered = false;
        self.panel_hovered = false;
        self.pinned = false;
        ctx.notify();
    }

    fn sync_tab_options(&mut self, ctx: &mut ViewContext<Self>) {
        let options = if server_tab_enabled(ctx) {
            vec![ProjectPanelTab::General, ProjectPanelTab::Server]
        } else {
            self.selected_tab = ProjectPanelTab::General;
            vec![ProjectPanelTab::General]
        };
        self.tab_control
            .update(ctx, |control, ctx| control.update_options(options, ctx));
    }

    fn refresh(&mut self, force: bool, ctx: &mut ViewContext<Self>) {
        let settings = ServiceDiscoverySettings::as_ref(ctx);
        if !is_service_discovery_available() || !*settings.enabled {
            self.disable(ctx);
            return;
        }
        let options = CollectionOptions {
            cache_usage: *settings.show_cache_usage,
            runtime_details: *settings.show_runtime_details,
            server_controls: *settings.enable_server_controls,
            docker_controls: *settings.enable_docker_controls,
        };
        let Some(directory) = self.current_directory(ctx) else {
            self.snapshot = None;
            self.loading = false;
            ctx.notify();
            return;
        };
        let is_fresh = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.directory == directory)
            && self
                .last_refresh
                .is_some_and(|last_refresh| last_refresh.elapsed() < SNAPSHOT_TTL);
        if !force && is_fresh {
            return;
        }
        if let Some(handle) = self.refresh_handle.take() {
            handle.abort();
        }
        self.loading = true;
        self.refresh_handle = Some(ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || collector::collect(directory, options)).await
            },
            |view, result, ctx| {
                view.loading = false;
                view.refresh_handle = None;
                match result {
                    Ok(snapshot) => {
                        view.snapshot = Some(snapshot);
                        view.last_refresh = Some(Instant::now());
                        view.sync_managed_snapshots();
                    }
                    Err(error) => {
                        log::warn!("Failed to collect project service information: {error}");
                        show_toast("Could not refresh project information", false, ctx);
                    }
                }
                ctx.notify();
            },
        ));
        ctx.notify();
    }

    fn start_static(&mut self, ctx: &mut ViewContext<Self>) {
        if !*ServiceDiscoverySettings::as_ref(ctx).enable_server_controls {
            return;
        }
        let Some(directory) = self.current_directory(ctx) else {
            return;
        };
        match self.managed.start_static(&directory) {
            Ok(service) => {
                self.sync_managed_snapshots();
                show_toast(
                    &format!(
                        "Static server started on port {}",
                        service.port.unwrap_or_default()
                    ),
                    true,
                    ctx,
                );
                self.refresh(true, ctx);
            }
            Err(error) => show_toast(&error.to_string(), false, ctx),
        }
    }

    fn start_script(&mut self, script: &str, ctx: &mut ViewContext<Self>) {
        if !*ServiceDiscoverySettings::as_ref(ctx).enable_server_controls {
            return;
        }
        let Some(directory) = self.current_directory(ctx) else {
            return;
        };
        let Some(package) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.package.clone())
        else {
            show_toast("package.json scripts are not available", false, ctx);
            return;
        };
        match self.managed.start_script(&directory, &package, script) {
            Ok(_) => {
                self.sync_managed_snapshots();
                show_toast(&format!("Started {} script", script), true, ctx);
                self.refresh(true, ctx);
            }
            Err(error) => show_toast(&error.to_string(), false, ctx),
        }
    }

    fn stop_managed(&mut self, id: ManagedServiceId, ctx: &mut ViewContext<Self>) {
        match self.managed.stop(id) {
            Ok(service) => {
                self.sync_managed_snapshots();
                show_toast(&format!("Stopped {}", service.label), true, ctx);
                self.refresh(true, ctx);
            }
            Err(error) => show_toast(&error.to_string(), false, ctx),
        }
    }

    fn run_docker(&mut self, action: DockerAction, ctx: &mut ViewContext<Self>) {
        if self.action_busy || !*ServiceDiscoverySettings::as_ref(ctx).enable_docker_controls {
            return;
        }
        let Some(directory) = self.current_directory(ctx) else {
            return;
        };
        self.action_busy = true;
        self.action_handle = Some(ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || managed::run_docker_action(directory, action))
                    .await
            },
            |view, result, ctx| {
                view.action_busy = false;
                view.action_handle = None;
                match result {
                    Ok(Ok(message)) => show_toast(&message, true, ctx),
                    Ok(Err(error)) => show_toast(&error.to_string(), false, ctx),
                    Err(error) => {
                        log::warn!("Docker project action failed: {error}");
                        show_toast("Docker action could not be completed", false, ctx);
                    }
                }
                view.refresh(true, ctx);
            },
        ));
        ctx.notify();
    }

    fn sync_managed_snapshots(&mut self) {
        let Some(directory) = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.directory.clone())
        else {
            self.managed_snapshots.clear();
            return;
        };
        self.managed_snapshots = self.managed.snapshots_for(&directory);
    }
}

impl Entity for ServiceDiscoveryView {
    type Event = ();
}

impl View for ServiceDiscoveryView {
    fn ui_name() -> &'static str {
        "ServiceDiscoveryView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        ui::render(self, app)
    }
}

impl TypedActionView for ServiceDiscoveryView {
    type Action = ServiceDiscoveryAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        if !is_service_discovery_available() {
            self.disable(ctx);
            return;
        }
        match action {
            ServiceDiscoveryAction::SetCompactHovered(hovered) => {
                let settings = ServiceDiscoverySettings::as_ref(ctx);
                self.compact_hovered = *hovered && *settings.enabled && *settings.open_on_hover;
                if self.compact_hovered {
                    self.refresh(false, ctx);
                }
            }
            ServiceDiscoveryAction::SetPanelHovered(hovered) => self.panel_hovered = *hovered,
            ServiceDiscoveryAction::TogglePinned => {
                if !*ServiceDiscoverySettings::as_ref(ctx).enabled {
                    return;
                }
                self.pinned = !self.pinned;
                if self.pinned {
                    self.refresh(false, ctx);
                }
            }
            ServiceDiscoveryAction::Dismiss => {
                self.pinned = false;
                self.compact_hovered = false;
                self.panel_hovered = false;
            }
            ServiceDiscoveryAction::Refresh => self.refresh(true, ctx),
            ServiceDiscoveryAction::OpenFinder => {
                if let Some(directory) = self.current_directory(ctx) {
                    ctx.open_file_path_in_explorer(&directory);
                }
            }
            ServiceDiscoveryAction::OpenEditor => {
                if let Some(directory) = self.current_directory(ctx) {
                    let editor = preferred_project_editor(ctx);
                    open_file_path_with_editor(None, directory, editor, ctx);
                }
            }
            ServiceDiscoveryAction::CopyPath => {
                if let Some(directory) = self.current_directory(ctx) {
                    ctx.clipboard().write(ClipboardContent::plain_text(
                        directory.to_string_lossy().into_owned(),
                    ));
                    show_toast("Project path copied", true, ctx);
                }
            }
            ServiceDiscoveryAction::OpenPort(port) => {
                ctx.open_url(&format!("http://localhost:{port}"));
            }
            ServiceDiscoveryAction::StartStatic => self.start_static(ctx),
            ServiceDiscoveryAction::StartScript(script) => self.start_script(script, ctx),
            ServiceDiscoveryAction::StopManaged(id) => self.stop_managed(*id, ctx),
            ServiceDiscoveryAction::RunDocker(action) => self.run_docker(action.clone(), ctx),
        }
        ctx.notify();
    }
}

fn tab_styles(app: &AppContext) -> UiComponentStyles {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    UiComponentStyles {
        font_family_id: Some(appearance.ui_font_family()),
        font_size: Some(11.),
        background: Some(UiFill::Solid(theme.surface_1().into())),
        border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
        border_width: Some(1.),
        border_color: Some(UiFill::Solid(theme.surface_3().into())),
        ..Default::default()
    }
}

fn preferred_project_editor(ctx: &mut AppContext) -> Option<Editor> {
    [Editor::Zed, Editor::VSCode]
        .into_iter()
        .find(|editor| editor.is_installed(ctx))
}

fn server_tab_enabled(app: &AppContext) -> bool {
    if !is_service_discovery_available() {
        return false;
    }
    let settings = ServiceDiscoverySettings::as_ref(app);
    *settings.enable_server_controls || *settings.enable_docker_controls
}

fn show_toast(message: &str, success: bool, ctx: &mut ViewContext<ServiceDiscoveryView>) {
    let window_id = ctx.window_id();
    ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
        let toast = if success {
            DismissibleToast::success(message.to_string())
        } else {
            DismissibleToast::error(message.to_string())
        };
        toast_stack.add_ephemeral_toast(toast, window_id, ctx);
    });
}
