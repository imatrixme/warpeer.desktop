use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use warpeer_plugin_protocol::PluginId;
use warpeer_plugin_runtime::WorkerHandle;

use super::{Connection, ProcessHandle};

#[test]
fn stopping_an_unresponsive_worker_reaps_the_process_and_connection() {
    let plugin_id = PluginId::parse("dev.warpeer.unresponsive-fixture").unwrap();
    let child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (input, _output) = std::sync::mpsc::sync_channel(1);
    let connection = Arc::new(Connection {
        input,
        child: Arc::new(Mutex::new(child)),
        stopping: AtomicBool::new(false),
    });
    let connections = Arc::new(Mutex::new(HashMap::from([(
        plugin_id.clone(),
        connection.clone(),
    )])));
    let mut handle = ProcessHandle {
        plugin_id: plugin_id.clone(),
        connection,
        connections: connections.clone(),
    };

    handle.stop().unwrap();

    assert!(!handle.is_alive());
    assert!(!connections.lock().contains_key(&plugin_id));
}
