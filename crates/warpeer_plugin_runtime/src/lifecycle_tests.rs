use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use warpeer_plugin_protocol::{PluginId, ResourceBudgets, WorkerId, WorkerState};

use crate::{PluginSupervisor, WorkerHandle, WorkerLauncher, WorkerRestartMode};

#[derive(Default)]
struct LifecycleCounts {
    launches: AtomicUsize,
    graceful_stops: AtomicUsize,
    forced_stops: AtomicUsize,
}

#[derive(Clone)]
struct Launcher {
    counts: Arc<LifecycleCounts>,
}

struct Handle {
    counts: Arc<LifecycleCounts>,
}

impl WorkerHandle for Handle {
    fn stop(&mut self) -> Result<(), String> {
        self.counts.graceful_stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn force_stop(&mut self) -> Result<(), String> {
        self.counts.forced_stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn is_alive(&mut self) -> bool {
        true
    }
}

impl WorkerLauncher for Launcher {
    fn launch(
        &self,
        _plugin_id: &PluginId,
        _worker_id: WorkerId,
        _budgets: ResourceBudgets,
    ) -> Result<Box<dyn WorkerHandle>, String> {
        self.counts.launches.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Handle {
            counts: self.counts.clone(),
        }))
    }
}

#[test]
fn reload_and_force_restart_replace_the_worker_identity() {
    let counts = Arc::new(LifecycleCounts::default());
    let supervisor = PluginSupervisor::new(
        Launcher {
            counts: counts.clone(),
        },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    );
    let plugin_id = PluginId::parse("dev.warpeer.lifecycle-test").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());

    let first = supervisor.trigger(&plugin_id).unwrap();
    supervisor.note_lifecycle(&plugin_id, WorkerState::Ready);
    supervisor
        .restart(&plugin_id, WorkerRestartMode::Graceful)
        .unwrap();
    let second = supervisor.trigger(&plugin_id).unwrap();
    assert_ne!(first.worker_id, second.worker_id);

    supervisor.note_lifecycle(&plugin_id, WorkerState::Ready);
    supervisor
        .restart(&plugin_id, WorkerRestartMode::Force)
        .unwrap();
    let third = supervisor.trigger(&plugin_id).unwrap();
    assert_ne!(second.worker_id, third.worker_id);
    assert_eq!(counts.launches.load(Ordering::SeqCst), 3);
    assert_eq!(counts.graceful_stops.load(Ordering::SeqCst), 1);
    assert_eq!(counts.forced_stops.load(Ordering::SeqCst), 1);
}

#[test]
fn disable_stop_invalidates_late_worker_output_identity() {
    let counts = Arc::new(LifecycleCounts::default());
    let supervisor = PluginSupervisor::new(
        Launcher { counts },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    );
    let plugin_id = PluginId::parse("dev.warpeer.stop-test").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());
    let running = supervisor.trigger(&plugin_id).unwrap().worker_id;
    supervisor.stop(&plugin_id).unwrap();

    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Stopped));
    assert_ne!(supervisor.worker_id(&plugin_id), Some(running));
}
