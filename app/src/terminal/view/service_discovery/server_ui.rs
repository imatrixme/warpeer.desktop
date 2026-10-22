use warpui::elements::{Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement};
use warpui::{AppContext, Element, SingletonEntity as _};

use super::collector::{DockerService, ServiceSnapshot};
use super::managed::{DockerAction, ManagedServiceKind};
use super::widgets::{
    action_button, empty_row, empty_state, metadata_text, section_gap, section_header, service_row,
};
use super::{ServiceDiscoveryAction, ServiceDiscoveryView};
use crate::appearance::Appearance;
use crate::service_discovery_settings::ServiceDiscoverySettings;
use crate::ui_components::icons::Icon;

pub(super) fn render(
    view: &ServiceDiscoveryView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let Some(snapshot) = view.snapshot.as_ref() else {
        return empty_state("Collecting server controls...", appearance);
    };
    let settings = ServiceDiscoverySettings::as_ref(app);
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    if *settings.enable_server_controls {
        column.add_child(section_header("Static server", 1, appearance));
        column.add_child(render_static_server(view, appearance));
        column.add_child(section_gap());
        let script_count = snapshot
            .package
            .as_ref()
            .map_or(0, |package| package.scripts.len());
        column.add_child(section_header("Dynamic services", script_count, appearance));
        if let Some(package) = &snapshot.package {
            column.add_child(
                Container::new(metadata_text(
                    &format!(
                        "{} · {}",
                        package.name.as_deref().unwrap_or("package.json"),
                        package.manager.label()
                    ),
                    appearance,
                ))
                .with_margin_bottom(4.)
                .finish(),
            );
            for script in &package.scripts {
                column.add_child(render_script(
                    view,
                    &script.name,
                    &script.command,
                    appearance,
                ));
            }
        } else {
            column.add_child(empty_row("No package.json scripts", appearance));
        }
    }
    if *settings.enable_docker_controls {
        if *settings.enable_server_controls {
            column.add_child(section_gap());
        }
        column.add_child(section_header(
            "Docker",
            snapshot.docker.services.len(),
            appearance,
        ));
        column.add_child(render_docker(snapshot, view.action_busy, appearance));
    }
    column.finish()
}

fn render_static_server(view: &ServiceDiscoveryView, appearance: &Appearance) -> Box<dyn Element> {
    let running = view
        .managed_snapshots
        .iter()
        .find(|service| service.kind == ManagedServiceKind::Static);
    match running {
        Some(service) => service_row(
            "Static files",
            &format!(
                "PID {} · localhost:{}",
                service.pid,
                service.port.unwrap_or_default()
            ),
            "Stop",
            ServiceDiscoveryAction::StopManaged(service.id),
            appearance,
        ),
        None => service_row(
            "Serve current directory",
            "Bound to localhost on an available port",
            "Start",
            ServiceDiscoveryAction::StartStatic,
            appearance,
        ),
    }
}

fn render_script(
    view: &ServiceDiscoveryView,
    name: &str,
    command: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let running = view
        .managed_snapshots
        .iter()
        .find(|service| service.kind == ManagedServiceKind::Script(name.to_string()));
    match running {
        Some(service) => service_row(
            name,
            &format!("PID {} · {command}", service.pid),
            "Stop",
            ServiceDiscoveryAction::StopManaged(service.id),
            appearance,
        ),
        None => service_row(
            name,
            command,
            "Start",
            ServiceDiscoveryAction::StartScript(name.to_string()),
            appearance,
        ),
    }
}

fn render_docker(
    snapshot: &ServiceSnapshot,
    action_busy: bool,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let docker = &snapshot.docker;
    if !docker.running {
        return service_row(
            "Docker Desktop",
            if docker.installed {
                "Docker daemon is stopped"
            } else {
                "Docker CLI is unavailable"
            },
            if action_busy { "Starting" } else { "Start" },
            ServiceDiscoveryAction::RunDocker(DockerAction::StartDesktop),
            appearance,
        );
    }
    let Some(compose_file) = docker.compose_file.as_ref() else {
        return empty_row("Docker is running; no Compose file found", appearance);
    };
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(service_row(
            compose_file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Compose"),
            "Service group",
            "Start all",
            ServiceDiscoveryAction::RunDocker(DockerAction::ComposeUp),
            appearance,
        ))
        .with_child(
            Container::new(action_button(
                "Stop all",
                Some(Icon::Stop),
                ServiceDiscoveryAction::RunDocker(DockerAction::ComposeDown),
                appearance,
            ))
            .with_margin_top(4.)
            .finish(),
        );
    for service in &docker.services {
        column.add_child(render_docker_service(service, appearance));
    }
    column.finish()
}

fn render_docker_service(service: &DockerService, appearance: &Appearance) -> Box<dyn Element> {
    let (label, action) = if service.running {
        ("Stop", DockerAction::StopService(service.name.clone()))
    } else {
        ("Start", DockerAction::StartService(service.name.clone()))
    };
    service_row(
        &service.name,
        if service.running {
            "Running"
        } else {
            "Stopped"
        },
        label,
        ServiceDiscoveryAction::RunDocker(action),
        appearance,
    )
}
