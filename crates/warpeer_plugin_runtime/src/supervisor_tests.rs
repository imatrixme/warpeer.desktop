use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use warpeer_plugin_protocol::*;

use crate::*;

#[derive(Clone)]
struct MockLauncher {
    launches: Arc<AtomicUsize>,
}

struct MockWorker;

impl WorkerHandle for MockWorker {
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn is_alive(&mut self) -> bool {
        true
    }
}

impl WorkerLauncher for MockLauncher {
    fn launch(
        &self,
        _plugin_id: &PluginId,
        _worker_id: WorkerId,
        _budgets: ResourceBudgets,
    ) -> Result<Box<dyn WorkerHandle>, String> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(MockWorker))
    }
}

#[test]
fn supervisor_is_lazy_and_reaps_idle_workers() {
    let launches = Arc::new(AtomicUsize::new(0));
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: launches.clone(),
        },
        ResourceBudgets::default(),
        Duration::from_millis(1),
    );
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());
    assert_eq!(launches.load(Ordering::SeqCst), 0);
    let first = supervisor.trigger(&plugin_id).unwrap();
    assert!(first.launched);
    let second = supervisor.trigger(&plugin_id).unwrap();
    assert!(!second.launched);
    assert_eq!(first.worker_id, second.worker_id);
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Starting));
    supervisor.note_lifecycle(&plugin_id, WorkerState::Ready);
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Running));
    supervisor.reap_idle(Instant::now() + Duration::from_secs(1));
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Idle));
}

#[test]
fn supervisor_reaps_after_last_surface_with_a_cancellable_short_grace() {
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: Arc::new(AtomicUsize::new(0)),
        },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    )
    .with_resource_release_grace(Duration::from_millis(500));
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());
    supervisor.trigger(&plugin_id).unwrap();
    supervisor
        .reserve_surface(
            &plugin_id,
            SurfaceBounds {
                x: 0.,
                y: 0.,
                width: 320.,
                height: 240.,
                scale: 1.,
            },
        )
        .unwrap();

    let first_release = Instant::now();
    supervisor.release_surface(&plugin_id);
    supervisor.reap_idle(first_release + Duration::from_millis(499));
    assert_ne!(supervisor.state(&plugin_id), Some(WorkerState::Idle));

    // A new trigger during the grace period keeps the existing worker alive.
    assert!(!supervisor.trigger(&plugin_id).unwrap().launched);
    supervisor.reap_idle(first_release + Duration::from_secs(2));
    assert_ne!(supervisor.state(&plugin_id), Some(WorkerState::Idle));

    supervisor
        .reserve_surface(
            &plugin_id,
            SurfaceBounds {
                x: 0.,
                y: 0.,
                width: 320.,
                height: 240.,
                scale: 1.,
            },
        )
        .unwrap();
    let second_release = Instant::now();
    supervisor.release_surface(&plugin_id);
    supervisor
        .accept_frame(
            &plugin_id,
            320 * 240,
            second_release + Duration::from_millis(100),
        )
        .unwrap();
    supervisor.reap_idle(second_release + Duration::from_millis(501));
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Idle));
    supervisor.note_lifecycle(&plugin_id, WorkerState::Stopped);
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Idle));
}

#[test]
fn supervisor_recovery_clears_failed_state_without_launching() {
    let launches = Arc::new(AtomicUsize::new(0));
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: launches.clone(),
        },
        ResourceBudgets::default(),
        Duration::from_secs(1),
    );
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());
    supervisor.note_crash(&plugin_id, "worker failed");
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Failed));
    supervisor.recover(&plugin_id).unwrap();
    assert_eq!(supervisor.state(&plugin_id), Some(WorkerState::Idle));
    assert_eq!(launches.load(Ordering::SeqCst), 0);
}

#[test]
fn supervisor_terminates_workers_that_never_become_ready() {
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: Arc::new(AtomicUsize::new(0)),
        },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    );
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());
    supervisor.trigger(&plugin_id).unwrap();
    supervisor.reap_unresponsive(
        Instant::now() + Duration::from_secs(2),
        Duration::from_secs(1),
    );
    assert_eq!(
        supervisor.state(&plugin_id),
        Some(WorkerState::WatchdogTerminated)
    );
}

#[test]
fn supervisor_enforces_surface_and_frame_budgets() {
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: Arc::new(AtomicUsize::new(0)),
        },
        ResourceBudgets::default(),
        Duration::from_secs(1),
    );
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let budgets = ResourceBudgets {
        max_surface_pixels: 64,
        max_frame_rate: 1,
        ..ResourceBudgets::default()
    };
    supervisor.register(plugin_id.clone(), budgets);
    let now = Instant::now();
    assert!(supervisor.accept_frame(&plugin_id, 64, now).is_ok());
    assert!(matches!(
        supervisor.accept_frame(&plugin_id, 64, now),
        Err(SupervisorError::FrameRateLimit)
    ));
    assert!(matches!(
        supervisor.accept_frame(&plugin_id, 65, now + Duration::from_secs(2)),
        Err(SupervisorError::SurfacePixels { .. })
    ));
}

#[test]
fn supervisor_failures_are_isolated_between_plugins() {
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: Arc::new(AtomicUsize::new(0)),
        },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    );
    let failed = PluginId::parse("dev.warpeer.failed").unwrap();
    let healthy = PluginId::parse("dev.warpeer.healthy").unwrap();
    for plugin_id in [&failed, &healthy] {
        supervisor.register(plugin_id.clone(), ResourceBudgets::default());
        supervisor.trigger(plugin_id).unwrap();
    }
    supervisor.note_lifecycle(&healthy, WorkerState::Ready);
    supervisor.note_crash(&failed, "worker failed");

    assert_eq!(supervisor.state(&failed), Some(WorkerState::Failed));
    assert_eq!(supervisor.state(&healthy), Some(WorkerState::Running));
    assert!(supervisor.accept_frame(&healthy, 1, Instant::now()).is_ok());
}

#[test]
fn supervisor_diagnostics_are_redacted_coalesced_and_clearable() {
    let supervisor = PluginSupervisor::new(
        MockLauncher {
            launches: Arc::new(AtomicUsize::new(0)),
        },
        ResourceBudgets::default(),
        Duration::from_secs(30),
    );
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    supervisor.register(plugin_id.clone(), ResourceBudgets::default());

    for _ in 0..3 {
        supervisor.record_diagnostic(
            &plugin_id,
            DiagnosticLevel::Info,
            "preview request https://example.com/image.png?token=secret password=secret",
        );
    }

    let diagnostics = supervisor.diagnostics(&plugin_id);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "preview request [redacted] [redacted]"
    );

    supervisor.clear_diagnostics(&plugin_id);
    assert!(supervisor.diagnostics(&plugin_id).is_empty());
}
