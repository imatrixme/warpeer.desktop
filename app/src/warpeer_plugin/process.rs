use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use sysinfo::{Pid, ProcessesToUpdate, System};
use warpeer_plugin_protocol::{
    CURRENT_PROTOCOL_VERSION, CURRENT_RUNTIME_ABI, Envelope, HostToPlugin, PluginId, PluginToHost,
    ProtocolLimits, ResourceBudgets, WorkerId, read_framed, write_framed,
};
use warpeer_plugin_runtime::{PluginManager, WorkerHandle, WorkerLauncher};

pub(crate) enum WorkerOutput {
    Message {
        plugin_id: PluginId,
        worker_id: WorkerId,
        envelope: Envelope<PluginToHost>,
    },
    Failed {
        plugin_id: PluginId,
        worker_id: WorkerId,
        detail: String,
    },
    Diagnostic {
        plugin_id: PluginId,
        worker_id: WorkerId,
        detail: String,
    },
}

struct Connection {
    input: mpsc::SyncSender<Envelope<HostToPlugin>>,
    child: Arc<Mutex<Child>>,
    stopping: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct ProcessLauncher {
    manager: Arc<PluginManager>,
    connections: Arc<Mutex<HashMap<PluginId, Arc<Connection>>>>,
    outputs: async_channel::Sender<WorkerOutput>,
}

impl ProcessLauncher {
    pub fn new(manager: Arc<PluginManager>, outputs: async_channel::Sender<WorkerOutput>) -> Self {
        Self {
            manager,
            connections: Arc::default(),
            outputs,
        }
    }

    pub fn send(&self, plugin_id: &PluginId, message: HostToPlugin) -> Result<(), String> {
        let connection = self
            .connections
            .lock()
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("plugin worker is not connected: {plugin_id}"))?;
        connection
            .input
            .send(Envelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                abi: CURRENT_RUNTIME_ABI,
                message,
            })
            .map_err(|_| format!("plugin worker input closed: {plugin_id}"))
    }

    pub fn remove(&self, plugin_id: &PluginId) {
        self.connections.lock().remove(plugin_id);
    }

    pub fn try_send(&self, plugin_id: &PluginId, message: HostToPlugin) -> Result<(), String> {
        let connection = self
            .connections
            .lock()
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("plugin worker is not connected: {plugin_id}"))?;
        connection
            .input
            .try_send(Envelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                abi: CURRENT_RUNTIME_ABI,
                message,
            })
            .map_err(|error| format!("plugin worker input unavailable: {error}"))
    }

    #[cfg(feature = "integration_tests")]
    pub fn connection_count(&self) -> usize {
        self.connections.lock().len()
    }

    #[cfg(feature = "integration_tests")]
    pub fn worker_memory_bytes(&self, plugin_id: &PluginId) -> Option<u64> {
        let pid = self
            .connections
            .lock()
            .get(plugin_id)
            .map(|connection| Pid::from_u32(connection.child.lock().id()))?;
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
        system.process(pid).map(|process| process.memory())
    }

    #[cfg(feature = "integration_tests")]
    pub fn terminate_worker(&self, plugin_id: &PluginId) -> Result<(), String> {
        let connection = self
            .connections
            .lock()
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("plugin worker is not connected: {plugin_id}"))?;
        connection
            .child
            .lock()
            .kill()
            .map_err(|error| error.to_string())
    }
}

impl WorkerLauncher for ProcessLauncher {
    fn launch(
        &self,
        plugin_id: &PluginId,
        worker_id: WorkerId,
        budgets: ResourceBudgets,
    ) -> Result<Box<dyn WorkerHandle>, String> {
        let installed = self
            .manager
            .installed(plugin_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("plugin is not installed: {plugin_id}"))?;
        let package_root = installed.package_root(self.manager.paths());
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut child = Command::new(executable)
            .arg("--warpeer-plugin-worker")
            .arg("--plugin-id")
            .arg(plugin_id.as_str())
            .arg("--package-root")
            .arg(package_root)
            .arg("--execution-slice-ms")
            .arg(budgets.execution_slice_ms.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        let stdin = child.stdin.take().ok_or("plugin worker has no stdin")?;
        let stdout = child.stdout.take().ok_or("plugin worker has no stdout")?;
        let stderr = child.stderr.take().ok_or("plugin worker has no stderr")?;
        let child = Arc::new(Mutex::new(child));
        let limits = ProtocolLimits {
            max_payload_bytes: budgets
                .max_surface_pixels
                .saturating_mul(4)
                .max(8 * 1024 * 1024),
            max_queued_messages: budgets.queued_messages as usize,
        };
        let (input_tx, input_rx) = mpsc::sync_channel(limits.max_queued_messages.max(1));
        spawn_writer(
            plugin_id.clone(),
            worker_id,
            stdin,
            input_rx,
            limits,
            self.outputs.clone(),
        );
        spawn_reader(
            plugin_id.clone(),
            worker_id,
            stdout,
            limits,
            self.outputs.clone(),
        );
        spawn_stderr(plugin_id.clone(), worker_id, stderr, self.outputs.clone());
        let connection = Arc::new(Connection {
            input: input_tx,
            child: child.clone(),
            stopping: AtomicBool::new(false),
        });
        spawn_process_watchdog(
            plugin_id.clone(),
            worker_id,
            connection.clone(),
            budgets.memory_bytes,
            self.outputs.clone(),
        );
        self.connections
            .lock()
            .insert(plugin_id.clone(), connection.clone());
        Ok(Box::new(ProcessHandle {
            plugin_id: plugin_id.clone(),
            connection,
            connections: self.connections.clone(),
        }))
    }
}

fn spawn_process_watchdog(
    plugin_id: PluginId,
    worker_id: WorkerId,
    connection: Arc<Connection>,
    memory_limit: u64,
    outputs: async_channel::Sender<WorkerOutput>,
) {
    thread::Builder::new()
        .name(format!("warpeer-plugin-watchdog-{plugin_id}"))
        .spawn(move || {
            let pid = Pid::from_u32(connection.child.lock().id());
            let mut system = System::new();
            loop {
                thread::sleep(Duration::from_millis(250));
                if let Ok(Some(status)) = connection.child.lock().try_wait() {
                    if !connection.stopping.load(Ordering::Acquire) {
                        let _ = outputs.try_send(WorkerOutput::Failed {
                            plugin_id,
                            worker_id,
                            detail: format!("plugin worker exited unexpectedly: {status}"),
                        });
                    }
                    break;
                }
                system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
                let Some(footprint) = system.process(pid).map(|process| process.memory()) else {
                    continue;
                };
                if footprint <= memory_limit {
                    continue;
                }
                connection.stopping.store(true, Ordering::Release);
                let mut child = connection.child.lock();
                let _ = child.kill();
                let _ = child.wait();
                let _ = outputs.try_send(WorkerOutput::Failed {
                    plugin_id,
                    worker_id,
                    detail: format!(
                        "plugin worker memory budget exceeded: {footprint} > {memory_limit} bytes"
                    ),
                });
                break;
            }
        })
        .expect("spawn Warpeer plugin memory watchdog");
}

struct ProcessHandle {
    plugin_id: PluginId,
    connection: Arc<Connection>,
    connections: Arc<Mutex<HashMap<PluginId, Arc<Connection>>>>,
}

impl WorkerHandle for ProcessHandle {
    fn stop(&mut self) -> Result<(), String> {
        self.connection.stopping.store(true, Ordering::Release);
        let _ = self.connection.input.try_send(Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            abi: CURRENT_RUNTIME_ABI,
            message: HostToPlugin::Stop,
        });
        self.reap_after(Duration::from_millis(250))
    }

    fn force_stop(&mut self) -> Result<(), String> {
        self.connection.stopping.store(true, Ordering::Release);
        self.reap_after(Duration::ZERO)
    }

    fn is_alive(&mut self) -> bool {
        self.connection
            .child
            .lock()
            .try_wait()
            .is_ok_and(|status| status.is_none())
    }
}

impl ProcessHandle {
    fn reap_after(&self, grace: Duration) -> Result<(), String> {
        let deadline = Instant::now() + grace;
        loop {
            if self
                .connection
                .child
                .lock()
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                let mut child = self.connection.child.lock();
                if child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_none()
                {
                    child.kill().map_err(|error| error.to_string())?;
                    child.wait().map_err(|error| error.to_string())?;
                }
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.connections.lock().remove(&self.plugin_id);
        Ok(())
    }
}

fn spawn_writer(
    plugin_id: PluginId,
    worker_id: WorkerId,
    stdin: ChildStdin,
    input: Receiver<Envelope<HostToPlugin>>,
    limits: ProtocolLimits,
    outputs: async_channel::Sender<WorkerOutput>,
) {
    thread::spawn(move || {
        let mut writer = BufWriter::new(stdin);
        for message in input {
            if let Err(error) = write_framed(&mut writer, &message, limits) {
                let _ = outputs.send_blocking(WorkerOutput::Failed {
                    plugin_id,
                    worker_id,
                    detail: error.to_string(),
                });
                break;
            }
        }
    });
}

fn spawn_reader(
    plugin_id: PluginId,
    worker_id: WorkerId,
    stdout: impl std::io::Read + Send + 'static,
    limits: ProtocolLimits,
    outputs: async_channel::Sender<WorkerOutput>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_framed::<Envelope<PluginToHost>>(&mut reader, limits) {
                Ok(Some(envelope)) => {
                    if outputs
                        .send_blocking(WorkerOutput::Message {
                            plugin_id: plugin_id.clone(),
                            worker_id,
                            envelope,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = outputs.send_blocking(WorkerOutput::Failed {
                        plugin_id,
                        worker_id,
                        detail: error.to_string(),
                    });
                    break;
                }
            }
        }
    });
}

fn spawn_stderr(
    plugin_id: PluginId,
    worker_id: WorkerId,
    stderr: impl std::io::Read + Send + 'static,
    outputs: async_channel::Sender<WorkerOutput>,
) {
    thread::spawn(move || {
        use std::io::BufRead as _;
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let _ = outputs.send_blocking(WorkerOutput::Diagnostic {
                plugin_id: plugin_id.clone(),
                worker_id,
                detail: line,
            });
        }
    });
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
