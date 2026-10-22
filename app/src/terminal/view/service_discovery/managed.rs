#![allow(clippy::disallowed_types)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};

use super::collector::{PackageManager, PackageProject};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ManagedServiceId(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ManagedServiceKind {
    Static,
    Script(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManagedServiceSnapshot {
    pub(super) id: ManagedServiceId,
    pub(super) kind: ManagedServiceKind,
    pub(super) label: String,
    pub(super) pid: u32,
    pub(super) port: Option<u16>,
    pub(super) directory: PathBuf,
}

struct ManagedService {
    snapshot: ManagedServiceSnapshot,
    child: Child,
}

#[derive(Default)]
pub(super) struct ManagedServiceController {
    next_id: u64,
    services: Vec<ManagedService>,
}

impl ManagedServiceController {
    pub(super) fn snapshots_for(&mut self, directory: &Path) -> Vec<ManagedServiceSnapshot> {
        self.remove_finished();
        self.services
            .iter()
            .filter(|service| service.snapshot.directory == directory)
            .map(|service| service.snapshot.clone())
            .collect()
    }

    pub(super) fn start_static(&mut self, directory: &Path) -> Result<ManagedServiceSnapshot> {
        if self
            .services
            .iter()
            .any(|service| service.snapshot.kind == ManagedServiceKind::Static)
        {
            return Err(anyhow!("A static server is already running for this pane"));
        }
        let port = available_local_port()?;
        let mut command = process_group_command("python3");
        command
            .args([
                "-m",
                "http.server",
                &port.to_string(),
                "--bind",
                "127.0.0.1",
                "--directory",
            ])
            .arg(directory)
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.spawn(
            command,
            directory.to_path_buf(),
            ManagedServiceKind::Static,
            "Static server".to_string(),
            Some(port),
        )
    }

    pub(super) fn start_script(
        &mut self,
        directory: &Path,
        package: &PackageProject,
        script: &str,
    ) -> Result<ManagedServiceSnapshot> {
        if self
            .services
            .iter()
            .any(|service| service.snapshot.kind == ManagedServiceKind::Script(script.to_string()))
        {
            return Err(anyhow!("The {script} script is already running"));
        }
        let plan = script_launch_plan(directory, package.manager, script);
        let mut command = process_group_command(&plan.program);
        command
            .args(&plan.args)
            .current_dir(&plan.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.spawn(
            command,
            directory.to_path_buf(),
            ManagedServiceKind::Script(script.to_string()),
            format!("{} {script}", package.manager.label()),
            None,
        )
    }

    pub(super) fn stop(&mut self, id: ManagedServiceId) -> Result<ManagedServiceSnapshot> {
        let index = self
            .services
            .iter()
            .position(|service| service.snapshot.id == id)
            .ok_or_else(|| anyhow!("Managed service is no longer running"))?;
        let mut service = self.services.remove(index);
        terminate_process_group(&mut service.child);
        Ok(service.snapshot)
    }

    pub(super) fn stop_all(&mut self) {
        for service in &mut self.services {
            terminate_process_group(&mut service.child);
        }
        self.services.clear();
    }

    fn spawn(
        &mut self,
        mut command: Command,
        directory: PathBuf,
        kind: ManagedServiceKind,
        label: String,
        port: Option<u16>,
    ) -> Result<ManagedServiceSnapshot> {
        let child = command
            .spawn()
            .with_context(|| format!("Could not start {label}"))?;
        self.next_id = self.next_id.saturating_add(1);
        let snapshot = ManagedServiceSnapshot {
            id: ManagedServiceId(self.next_id),
            kind,
            label,
            pid: child.id(),
            port,
            directory,
        };
        self.services.push(ManagedService {
            snapshot: snapshot.clone(),
            child,
        });
        Ok(snapshot)
    }

    fn remove_finished(&mut self) {
        self.services.retain_mut(|service| {
            service
                .child
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        });
    }
}

impl Drop for ManagedServiceController {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScriptLaunchPlan {
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) directory: PathBuf,
}

pub(super) fn script_launch_plan(
    directory: &Path,
    manager: PackageManager,
    script: &str,
) -> ScriptLaunchPlan {
    ScriptLaunchPlan {
        program: manager.executable().to_string(),
        args: manager.script_args(script),
        directory: directory.to_path_buf(),
    }
}

#[derive(Clone, Debug)]
pub(crate) enum DockerAction {
    StartDesktop,
    ComposeUp,
    ComposeDown,
    StartService(String),
    StopService(String),
}

pub(super) fn run_docker_action(directory: PathBuf, action: DockerAction) -> Result<String> {
    let (program, args, success_message) = match action {
        DockerAction::StartDesktop => docker_desktop_command()?,
        DockerAction::ComposeUp => (
            "docker".to_string(),
            vec!["compose".to_string(), "up".to_string(), "-d".to_string()],
            "Docker services started".to_string(),
        ),
        DockerAction::ComposeDown => (
            "docker".to_string(),
            vec!["compose".to_string(), "down".to_string()],
            "Docker services stopped".to_string(),
        ),
        DockerAction::StartService(service) => (
            "docker".to_string(),
            vec![
                "compose".to_string(),
                "up".to_string(),
                "-d".to_string(),
                service.clone(),
            ],
            format!("Docker service {service} started"),
        ),
        DockerAction::StopService(service) => (
            "docker".to_string(),
            vec!["compose".to_string(), "stop".to_string(), service.clone()],
            format!("Docker service {service} stopped"),
        ),
    };
    let output = run_command_with_timeout(&program, &args, &directory, Duration::from_secs(30))?;
    if output.status.success() {
        Ok(success_message)
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!(if message.is_empty() {
            format!("{program} exited with {}", output.status)
        } else {
            message
        }))
    }
}

fn run_command_with_timeout(
    program: &str,
    args: &[String],
    directory: &Path,
    timeout: Duration,
) -> Result<Output> {
    let mut child = process_group_command(program)
        .args(args)
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Could not run {program}"))?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .with_context(|| format!("Could not read {program} output"));
        }
        if started.elapsed() >= timeout {
            terminate_process_group(&mut child);
            return Err(anyhow!("{program} did not finish within 30 seconds"));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(target_os = "macos")]
fn docker_desktop_command() -> Result<(String, Vec<String>, String)> {
    Ok((
        "open".to_string(),
        vec!["-a".to_string(), "Docker".to_string()],
        "Docker Desktop is starting".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
fn docker_desktop_command() -> Result<(String, Vec<String>, String)> {
    Err(anyhow!(
        "Start the Docker daemon with your system service manager"
    ))
}

fn available_local_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("Could not reserve a local port")?;
    Ok(listener.local_addr()?.port())
}

#[allow(unused_mut)]
fn process_group_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
}

#[cfg(unix)]
fn terminate_process_group(child: &mut Child) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let pid = child.id();
    if pid >= 2 {
        let _ = kill(Pid::from_raw(-(pid as i32)), Signal::SIGKILL);
    }
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_process_group(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
