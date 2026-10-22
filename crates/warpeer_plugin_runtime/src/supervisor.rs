use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use warpeer_plugin_protocol::{
    DiagnosticLevel, PluginId, ResourceBudgets, SurfaceBounds, WorkerId, WorkerState,
};

use crate::SupervisorError;

const MAX_DIAGNOSTICS: usize = 200;
const DIAGNOSTIC_COALESCE_WINDOW: Duration = Duration::from_millis(750);
const CRASH_WINDOW: Duration = Duration::from_secs(60);
const MAX_RESTARTS_IN_WINDOW: usize = 3;

pub trait WorkerHandle: Send {
    fn stop(&mut self) -> Result<(), String>;
    fn force_stop(&mut self) -> Result<(), String> {
        self.stop()
    }
    fn is_alive(&mut self) -> bool;
}

pub trait WorkerLauncher: Send + Sync {
    fn launch(
        &self,
        plugin_id: &PluginId,
        worker_id: WorkerId,
        budgets: ResourceBudgets,
    ) -> Result<Box<dyn WorkerHandle>, String>;
}

#[derive(Clone, Debug)]
pub struct DiagnosticEntry {
    pub level: DiagnosticLevel,
    pub message: String,
    pub timestamp: Instant,
}

struct WorkerRecord {
    worker_id: WorkerId,
    state: WorkerState,
    budgets: ResourceBudgets,
    handle: Option<Box<dyn WorkerHandle>>,
    crashes: VecDeque<Instant>,
    diagnostics: VecDeque<DiagnosticEntry>,
    active_surfaces: u16,
    frame_times: VecDeque<Instant>,
    last_active: Instant,
    resources_released_at: Option<Instant>,
    starting_since: Option<Instant>,
}

impl WorkerRecord {
    fn new(budgets: ResourceBudgets) -> Self {
        Self {
            worker_id: WorkerId::new(),
            state: WorkerState::Idle,
            budgets,
            handle: None,
            crashes: VecDeque::new(),
            diagnostics: VecDeque::new(),
            active_surfaces: 0,
            frame_times: VecDeque::new(),
            last_active: Instant::now(),
            resources_released_at: None,
            starting_since: None,
        }
    }

    fn diagnostic(&mut self, level: DiagnosticLevel, message: impl Into<String>) -> bool {
        let timestamp = Instant::now();
        let message = redact(message.into());
        if self.diagnostics.back().is_some_and(|entry| {
            entry.level == level
                && entry.message == message
                && timestamp.saturating_duration_since(entry.timestamp) < DIAGNOSTIC_COALESCE_WINDOW
        }) {
            return false;
        }
        if self.diagnostics.len() == MAX_DIAGNOSTICS {
            self.diagnostics.pop_front();
        }
        self.diagnostics.push_back(DiagnosticEntry {
            level,
            message,
            timestamp,
        });
        true
    }
}

pub struct PluginSupervisor<L> {
    launcher: L,
    workers: Mutex<HashMap<PluginId, WorkerRecord>>,
    ceiling: ResourceBudgets,
    idle_grace: Duration,
    resource_release_grace: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggeredWorker {
    pub worker_id: WorkerId,
    pub launched: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerRestartMode {
    Graceful,
    Force,
}

impl<L: WorkerLauncher> PluginSupervisor<L> {
    pub fn new(launcher: L, ceiling: ResourceBudgets, idle_grace: Duration) -> Self {
        Self {
            launcher,
            workers: Mutex::new(HashMap::new()),
            ceiling,
            idle_grace,
            resource_release_grace: idle_grace,
        }
    }

    /// Uses a shorter, cancellable grace period after the last surface closes.
    /// Startup and capability work without a surface still use `idle_grace`.
    pub fn with_resource_release_grace(mut self, grace: Duration) -> Self {
        self.resource_release_grace = grace;
        self
    }

    pub fn register(&self, plugin_id: PluginId, budgets: ResourceBudgets) {
        let budgets = budgets.clamped_to(self.ceiling);
        let mut workers = self.workers.lock();
        match workers.entry(plugin_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().budgets = budgets;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(WorkerRecord::new(budgets));
            }
        }
    }

    pub fn recover(&self, plugin_id: &PluginId) -> Result<(), SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        record.crashes.clear();
        record.starting_since = None;
        record.state = if record.handle.is_some() {
            WorkerState::Running
        } else {
            WorkerState::Idle
        };
        record.diagnostic(DiagnosticLevel::Info, "plugin worker recovered");
        Ok(())
    }

    pub fn trigger(&self, plugin_id: &PluginId) -> Result<TriggeredWorker, SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        record.last_active = Instant::now();
        record.resources_released_at = None;
        if record
            .handle
            .as_mut()
            .is_some_and(|handle| handle.is_alive())
        {
            return Ok(TriggeredWorker {
                worker_id: record.worker_id,
                launched: false,
            });
        }
        prune_crashes(record);
        if record.crashes.len() >= MAX_RESTARTS_IN_WINDOW {
            record.state = WorkerState::Failed;
            return Err(SupervisorError::CrashLoop(plugin_id.clone()));
        }
        record.worker_id = WorkerId::new();
        record.state = WorkerState::Starting;
        match self
            .launcher
            .launch(plugin_id, record.worker_id, record.budgets)
        {
            Ok(handle) => {
                record.handle = Some(handle);
                record.state = WorkerState::Starting;
                record.starting_since = Some(Instant::now());
                record.diagnostic(DiagnosticLevel::Info, "plugin worker started");
                Ok(TriggeredWorker {
                    worker_id: record.worker_id,
                    launched: true,
                })
            }
            Err(error) => {
                record.state = WorkerState::Failed;
                record.crashes.push_back(Instant::now());
                record.diagnostic(DiagnosticLevel::Error, &error);
                Err(SupervisorError::Launch(error))
            }
        }
    }

    pub fn note_crash(&self, plugin_id: &PluginId, detail: impl Into<String>) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            record.handle = None;
            record.state = WorkerState::Failed;
            record.starting_since = None;
            record.crashes.push_back(Instant::now());
            record.diagnostic(DiagnosticLevel::Error, detail);
        }
    }

    pub fn note_violation(&self, plugin_id: &PluginId, detail: impl Into<String>) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            record.diagnostic(DiagnosticLevel::Warning, detail);
        }
    }

    pub fn record_diagnostic(
        &self,
        plugin_id: &PluginId,
        level: DiagnosticLevel,
        detail: impl Into<String>,
    ) -> bool {
        self.workers
            .lock()
            .get_mut(plugin_id)
            .is_some_and(|record| record.diagnostic(level, detail))
    }

    pub fn reserve_surface(
        &self,
        plugin_id: &PluginId,
        bounds: SurfaceBounds,
    ) -> Result<(), SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        if !bounds.is_valid() {
            return Err(SupervisorError::InvalidSurfaceBounds);
        }
        if record.active_surfaces >= record.budgets.active_surfaces {
            return Err(SupervisorError::SurfaceLimit);
        }
        if bounds.pixel_area() > record.budgets.max_surface_pixels {
            return Err(SupervisorError::SurfacePixels {
                actual: bounds.pixel_area(),
                maximum: record.budgets.max_surface_pixels,
            });
        }
        record.active_surfaces += 1;
        record.last_active = Instant::now();
        record.resources_released_at = None;
        Ok(())
    }

    pub fn validate_surface_bounds(
        &self,
        plugin_id: &PluginId,
        bounds: SurfaceBounds,
    ) -> Result<(), SupervisorError> {
        let workers = self.workers.lock();
        let record = workers
            .get(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        if !bounds.is_valid() {
            return Err(SupervisorError::InvalidSurfaceBounds);
        }
        if bounds.pixel_area() > record.budgets.max_surface_pixels {
            return Err(SupervisorError::SurfacePixels {
                actual: bounds.pixel_area(),
                maximum: record.budgets.max_surface_pixels,
            });
        }
        Ok(())
    }

    pub fn release_surface(&self, plugin_id: &PluginId) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            record.active_surfaces = record.active_surfaces.saturating_sub(1);
            record.last_active = Instant::now();
            if record.active_surfaces == 0 {
                record.resources_released_at = Some(record.last_active);
            }
        }
    }

    pub fn release_all_surfaces(&self, plugin_id: &PluginId) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            record.active_surfaces = 0;
            record.frame_times.clear();
            record.last_active = Instant::now();
            record.resources_released_at = Some(record.last_active);
        }
    }

    pub fn accept_frame(
        &self,
        plugin_id: &PluginId,
        pixel_area: u64,
        now: Instant,
    ) -> Result<(), SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        if pixel_area > record.budgets.max_surface_pixels {
            record.diagnostic(
                DiagnosticLevel::Warning,
                format!("plugin frame exceeded pixel budget: {pixel_area}"),
            );
            return Err(SupervisorError::SurfacePixels {
                actual: pixel_area,
                maximum: record.budgets.max_surface_pixels,
            });
        }
        let threshold = now - Duration::from_secs(1);
        while record
            .frame_times
            .front()
            .is_some_and(|timestamp| *timestamp < threshold)
        {
            record.frame_times.pop_front();
        }
        if record.frame_times.len() >= record.budgets.max_frame_rate as usize {
            record.diagnostic(
                DiagnosticLevel::Warning,
                "plugin frame rate exceeded budget",
            );
            return Err(SupervisorError::FrameRateLimit);
        }
        record.frame_times.push_back(now);
        // A frame already queued when the last surface closes must not revive the
        // worker or cancel its short teardown grace period.
        if record.active_surfaces > 0 {
            record.last_active = now;
            record.resources_released_at = None;
        }
        Ok(())
    }

    pub fn reap_idle(&self, now: Instant) {
        let mut workers = self.workers.lock();
        for record in workers.values_mut() {
            let released_resources_are_due = record.resources_released_at.is_some_and(|released| {
                now.saturating_duration_since(released) >= self.resource_release_grace
            });
            if record.active_surfaces == 0
                && (released_resources_are_due
                    || now.saturating_duration_since(record.last_active) >= self.idle_grace)
                && record.handle.is_some()
            {
                if let Err(error) = stop_record(record, WorkerRestartMode::Graceful) {
                    record.diagnostic(DiagnosticLevel::Warning, error.to_string());
                }
                record.state = WorkerState::Idle;
                record.starting_since = None;
                record.resources_released_at = None;
            }
        }
    }

    pub fn reap_unresponsive(&self, now: Instant, startup_timeout: Duration) {
        let mut workers = self.workers.lock();
        for record in workers.values_mut() {
            if record
                .starting_since
                .is_some_and(|started| now.duration_since(started) >= startup_timeout)
            {
                if let Err(error) = stop_record(record, WorkerRestartMode::Graceful) {
                    record.diagnostic(DiagnosticLevel::Warning, error.to_string());
                }
                record.state = WorkerState::WatchdogTerminated;
                record.starting_since = None;
                record.crashes.push_back(now);
                record.diagnostic(DiagnosticLevel::Error, "plugin worker startup timed out");
            }
        }
    }

    pub fn note_lifecycle(&self, plugin_id: &PluginId, state: WorkerState) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            // Idle reaping waits for the process to exit while holding the
            // supervisor lock. Its queued Stopped event can therefore arrive
            // after the authoritative Idle state has already been committed.
            if state == WorkerState::Stopped
                && record.state == WorkerState::Idle
                && record.handle.is_none()
            {
                return;
            }
            record.state = match state {
                WorkerState::Ready => WorkerState::Running,
                state => state,
            };
            if record.state != WorkerState::Starting {
                record.starting_since = None;
            }
            record.last_active = Instant::now();
        }
    }

    pub fn stop(&self, plugin_id: &PluginId) -> Result<(), SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        stop_record(record, WorkerRestartMode::Graceful)?;
        record.worker_id = WorkerId::new();
        record.state = WorkerState::Stopped;
        record.starting_since = None;
        record.active_surfaces = 0;
        record.frame_times.clear();
        record.resources_released_at = None;
        Ok(())
    }

    pub fn restart(
        &self,
        plugin_id: &PluginId,
        mode: WorkerRestartMode,
    ) -> Result<(), SupervisorError> {
        let mut workers = self.workers.lock();
        let record = workers
            .get_mut(plugin_id)
            .ok_or_else(|| SupervisorError::UnknownPlugin(plugin_id.clone()))?;
        stop_record(record, mode)?;
        record.worker_id = WorkerId::new();
        record.state = WorkerState::Idle;
        record.starting_since = None;
        record.active_surfaces = 0;
        record.frame_times.clear();
        record.resources_released_at = None;
        record.crashes.clear();
        record.diagnostic(
            DiagnosticLevel::Info,
            match mode {
                WorkerRestartMode::Graceful => "plugin worker reloaded",
                WorkerRestartMode::Force => "plugin worker force restarted",
            },
        );
        Ok(())
    }

    pub fn unregister(&self, plugin_id: &PluginId) -> Result<(), SupervisorError> {
        self.stop(plugin_id)?;
        self.workers.lock().remove(plugin_id);
        Ok(())
    }

    pub fn state(&self, plugin_id: &PluginId) -> Option<WorkerState> {
        self.workers
            .lock()
            .get(plugin_id)
            .map(|record| record.state.clone())
    }

    pub fn worker_id(&self, plugin_id: &PluginId) -> Option<WorkerId> {
        self.workers
            .lock()
            .get(plugin_id)
            .map(|record| record.worker_id)
    }

    pub fn diagnostics(&self, plugin_id: &PluginId) -> Vec<DiagnosticEntry> {
        self.workers
            .lock()
            .get(plugin_id)
            .map_or_else(Vec::new, |record| {
                record.diagnostics.iter().cloned().collect()
            })
    }

    pub fn clear_diagnostics(&self, plugin_id: &PluginId) {
        if let Some(record) = self.workers.lock().get_mut(plugin_id) {
            record.diagnostics.clear();
        }
    }
}

fn stop_record(record: &mut WorkerRecord, mode: WorkerRestartMode) -> Result<(), SupervisorError> {
    let Some(mut handle) = record.handle.take() else {
        return Ok(());
    };
    let result = match mode {
        WorkerRestartMode::Force => handle.force_stop(),
        WorkerRestartMode::Graceful => handle.stop().or_else(|graceful| {
            record.diagnostic(
                DiagnosticLevel::Warning,
                format!("graceful worker stop failed: {graceful}"),
            );
            handle.force_stop()
        }),
    };
    result.map_err(SupervisorError::Stop)
}

fn prune_crashes(record: &mut WorkerRecord) {
    let threshold = Instant::now() - CRASH_WINDOW;
    while record
        .crashes
        .front()
        .is_some_and(|timestamp| *timestamp < threshold)
    {
        record.crashes.pop_front();
    }
}

fn redact(message: String) -> String {
    message
        .split_whitespace()
        .map(|token| {
            if token.contains("token=") || token.contains("password=") || token.contains("key=") {
                "[redacted]"
            } else if token.starts_with("https://") {
                token.split('?').next().unwrap_or("[url]")
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
