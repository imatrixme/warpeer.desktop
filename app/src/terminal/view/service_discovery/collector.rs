use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use std::{fs, thread};

use serde_json::Value;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const CACHE_DIRECTORIES: &[(&str, &str)] = &[
    ("node_modules", "node_modules"),
    ("Rust target", "target"),
    ("Build", "build"),
    ("Distribution", "dist"),
    ("Next.js", ".next"),
    ("Turbo", ".turbo"),
    ("Cache", ".cache"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CollectionOptions {
    pub(super) cache_usage: bool,
    pub(super) runtime_details: bool,
    pub(super) server_controls: bool,
    pub(super) docker_controls: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ServiceSnapshot {
    pub(super) directory: PathBuf,
    pub(super) caches: Vec<CacheUsage>,
    pub(super) package: Option<PackageProject>,
    pub(super) docker: DockerSnapshot,
    pub(super) processes: Vec<ServiceProcess>,
    pub(super) ports: Vec<ServicePort>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CacheUsage {
    pub(super) label: String,
    pub(super) path: PathBuf,
    pub(super) size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PackageManager {
    Bun,
    Npm,
    Pnpm,
    Yarn,
}

impl PackageManager {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Bun => "Bun",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "Yarn",
        }
    }

    pub(super) fn executable(self) -> &'static str {
        match self {
            Self::Bun => "bun",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
        }
    }

    pub(super) fn script_args(self, script: &str) -> Vec<String> {
        match self {
            Self::Yarn => vec![script.to_string()],
            Self::Bun | Self::Npm | Self::Pnpm => vec!["run".to_string(), script.to_string()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PackageProject {
    pub(super) name: Option<String>,
    pub(super) manager: PackageManager,
    pub(super) scripts: Vec<PackageScript>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PackageScript {
    pub(super) name: String,
    pub(super) command: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DockerSnapshot {
    pub(super) installed: bool,
    pub(super) running: bool,
    pub(super) compose_file: Option<PathBuf>,
    pub(super) services: Vec<DockerService>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DockerService {
    pub(super) name: String,
    pub(super) running: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ServiceProcess {
    pub(super) pid: u32,
    pub(super) name: String,
    pub(super) environment: String,
    pub(super) cpu_percent: f32,
    pub(super) memory_bytes: u64,
    pub(super) ports: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ServicePort {
    pub(super) port: u16,
    pub(super) pid: u32,
    pub(super) environment: String,
}

pub(super) fn collect(directory: PathBuf, options: CollectionOptions) -> ServiceSnapshot {
    let caches = options
        .cache_usage
        .then(|| collect_cache_usage(&directory))
        .unwrap_or_default();
    let package = options
        .server_controls
        .then(|| read_package_project(&directory))
        .flatten();
    let docker = options
        .docker_controls
        .then(|| collect_docker_snapshot(&directory))
        .unwrap_or_default();
    let (processes, ports) = options
        .runtime_details
        .then(|| collect_processes(&directory))
        .unwrap_or_default();
    ServiceSnapshot {
        directory,
        caches,
        package,
        docker,
        processes,
        ports,
    }
}

fn collect_cache_usage(directory: &Path) -> Vec<CacheUsage> {
    CACHE_DIRECTORIES
        .iter()
        .filter_map(|(label, relative_path)| {
            let path = directory.join(relative_path);
            path.is_dir().then(|| CacheUsage {
                label: (*label).to_string(),
                size_bytes: directory_size(&path),
                path,
            })
        })
        .collect()
}

fn directory_size(path: &Path) -> u64 {
    let mut command = Command::new("du");
    command.args(["-sk"]).arg(path);
    run_with_timeout(&mut command, COMMAND_TIMEOUT)
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.split_whitespace().next()?.parse::<u64>().ok())
        .unwrap_or_default()
        .saturating_mul(1024)
}

fn read_package_project(directory: &Path) -> Option<PackageProject> {
    let contents = fs::read_to_string(directory.join("package.json")).ok()?;
    let value = serde_json::from_str::<Value>(&contents).ok()?;
    let mut scripts = value
        .get("scripts")?
        .as_object()?
        .iter()
        .filter_map(|(name, command)| {
            Some(PackageScript {
                name: name.clone(),
                command: command.as_str()?.to_string(),
            })
        })
        .collect::<Vec<_>>();
    scripts.sort_by_key(|script| (script_priority(&script.name), script.name.clone()));
    Some(PackageProject {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        manager: detect_package_manager(directory, &value),
        scripts,
    })
}

fn script_priority(name: &str) -> u8 {
    match name {
        "dev" => 0,
        "start" => 1,
        "serve" => 2,
        "preview" => 3,
        _ => 4,
    }
}

fn detect_package_manager(directory: &Path, package: &Value) -> PackageManager {
    if let Some(manager) = package.get("packageManager").and_then(Value::as_str) {
        if manager.starts_with("bun@") {
            return PackageManager::Bun;
        }
        if manager.starts_with("pnpm@") {
            return PackageManager::Pnpm;
        }
        if manager.starts_with("yarn@") {
            return PackageManager::Yarn;
        }
    }
    if directory.join("bun.lock").exists() || directory.join("bun.lockb").exists() {
        PackageManager::Bun
    } else if directory.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if directory.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

fn collect_docker_snapshot(directory: &Path) -> DockerSnapshot {
    let compose_file = [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ]
    .into_iter()
    .map(|name| directory.join(name))
    .find(|path| path.is_file());
    let installed = command_succeeds(Command::new("docker").arg("--version"));
    if !installed {
        return DockerSnapshot {
            compose_file,
            ..Default::default()
        };
    }
    let running =
        command_succeeds(Command::new("docker").args(["info", "--format", "{{.ServerVersion}}"]));
    let services = compose_file
        .is_some()
        .then(|| collect_compose_services(directory, running))
        .unwrap_or_default();
    DockerSnapshot {
        installed,
        running,
        compose_file,
        services,
    }
}

fn collect_compose_services(directory: &Path, docker_running: bool) -> Vec<DockerService> {
    let mut config_command = Command::new("docker");
    config_command
        .current_dir(directory)
        .args(["compose", "config", "--services"]);
    let configured = command_lines(config_command);
    let running = if docker_running {
        let mut running_command = Command::new("docker");
        running_command.current_dir(directory).args([
            "compose",
            "ps",
            "--services",
            "--status",
            "running",
        ]);
        command_lines(running_command)
            .into_iter()
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    configured
        .into_iter()
        .map(|name| DockerService {
            running: running.contains(&name),
            name,
        })
        .collect()
}

fn command_lines(mut command: Command) -> Vec<String> {
    run_with_timeout(&mut command, COMMAND_TIMEOUT)
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| {
            output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn command_succeeds(command: &mut Command) -> bool {
    run_with_timeout(command, COMMAND_TIMEOUT).is_some_and(|output| output.status.success())
}

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Option<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        if child.try_wait().ok()?.is_some() {
            return child.wait_with_output().ok();
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn collect_processes(directory: &Path) -> (Vec<ServiceProcess>, Vec<ServicePort>) {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cwd(UpdateKind::Always)
            .with_cmd(UpdateKind::Always)
            .with_memory()
            .with_cpu(),
    );
    let mut pids = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let cwd = process.cwd()?;
            let name = process.name().to_string_lossy();
            let command = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            (is_related_directory(cwd, directory) && classify_runtime(&name, &command).is_some())
                .then_some(*pid)
        })
        .collect::<Vec<_>>();
    pids.sort_unstable();
    if !pids.is_empty() {
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
    }
    let ports = query_listening_ports(&pids.iter().map(|pid| pid.as_u32()).collect::<Vec<_>>());
    let ports_by_pid = ports
        .iter()
        .fold(HashMap::<u32, Vec<u16>>::new(), |mut map, port| {
            map.entry(port.pid).or_default().push(port.port);
            map
        });
    let mut processes = pids
        .into_iter()
        .filter_map(|pid| {
            let process = system.process(pid)?;
            let name = process.name().to_string_lossy().into_owned();
            let command = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            Some(ServiceProcess {
                pid: pid.as_u32(),
                environment: classify_runtime(&name, &command)?.to_string(),
                name,
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                ports: ports_by_pid.get(&pid.as_u32()).cloned().unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| {
        right
            .cpu_percent
            .total_cmp(&left.cpu_percent)
            .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
            .then_with(|| left.pid.cmp(&right.pid))
    });
    (processes, ports)
}

fn is_related_directory(process_cwd: &Path, directory: &Path) -> bool {
    process_cwd == directory || process_cwd.starts_with(directory)
}

pub(super) fn classify_runtime(name: &str, command: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    let command = command.to_ascii_lowercase();
    if name.contains("workerd") || command.contains("workerd") || command.contains("wrangler") {
        Some("Workers")
    } else if name == "bun" || command.starts_with("bun ") || command.contains(" bun ") {
        Some("Bun")
    } else if name == "node" || command.starts_with("node ") || command.contains(" node ") {
        if command.contains("vite") {
            Some("Vite")
        } else if command.contains("next") {
            Some("Next.js")
        } else {
            Some("Node.js")
        }
    } else if name == "deno" || command.starts_with("deno ") || command.contains(" deno ") {
        Some("Deno")
    } else {
        None
    }
}

fn query_listening_ports(pids: &[u32]) -> Vec<ServicePort> {
    if pids.is_empty() {
        return Vec::new();
    }
    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut command = Command::new("lsof");
    command.args([
        "-nP",
        "-a",
        "-p",
        &pid_list,
        "-iTCP",
        "-sTCP:LISTEN",
        "-Fpcn",
    ]);
    run_with_timeout(&mut command, COMMAND_TIMEOUT)
        .filter(|output| output.status.success())
        .map(|output| parse_lsof_ports(&output.stdout))
        .unwrap_or_default()
}

pub(super) fn parse_lsof_ports(bytes: &[u8]) -> Vec<ServicePort> {
    let Ok(output) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let mut current_pid = None;
    let mut current_command = String::new();
    let mut seen = HashSet::new();
    let mut ports = Vec::new();
    for line in output.lines() {
        match line.as_bytes().first() {
            Some(b'p') => current_pid = line[1..].parse::<u32>().ok(),
            Some(b'c') => current_command = line[1..].to_string(),
            Some(b'n') => {
                let Some(pid) = current_pid else { continue };
                let Some(port) = line[1..]
                    .rsplit(':')
                    .next()
                    .and_then(|value| value.parse::<u16>().ok())
                else {
                    continue;
                };
                if seen.insert((pid, port)) {
                    ports.push(ServicePort {
                        port,
                        pid,
                        environment: classify_runtime(&current_command, &current_command)
                            .unwrap_or("Service")
                            .to_string(),
                    });
                }
            }
            Some(_) | None => {}
        }
    }
    ports.sort_by_key(|port| (port.port, port.pid));
    ports
}

#[cfg(test)]
#[path = "collector_tests.rs"]
mod tests;
