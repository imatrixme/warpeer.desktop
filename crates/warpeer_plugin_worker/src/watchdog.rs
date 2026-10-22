use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(2);

pub(super) struct ExecutionWatchdog {
    deadline_micros: Arc<AtomicU64>,
    armed: Arc<AtomicBool>,
    started: Instant,
    limit: Duration,
}

impl ExecutionWatchdog {
    pub fn new(limit: Duration) -> Self {
        let deadline_micros = Arc::new(AtomicU64::new(0));
        let armed = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let monitor_deadline = deadline_micros.clone();
        let monitor_armed = armed.clone();
        thread::Builder::new()
            .name("warpeer-plugin-execution-watchdog".into())
            .spawn(move || {
                loop {
                    thread::sleep(POLL_INTERVAL);
                    if !monitor_armed.load(Ordering::Acquire) {
                        continue;
                    }
                    let elapsed = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
                    if elapsed > monitor_deadline.load(Ordering::Acquire) {
                        eprintln!("plugin worker execution slice exceeded");
                        std::process::exit(70);
                    }
                }
            })
            .expect("spawn plugin execution watchdog");
        Self {
            deadline_micros,
            armed,
            started,
            limit,
        }
    }

    pub fn begin(&self) -> WatchdogOperation {
        let deadline = self
            .started
            .elapsed()
            .saturating_add(self.limit)
            .as_micros()
            .min(u64::MAX as u128) as u64;
        self.deadline_micros.store(deadline, Ordering::Release);
        self.armed.store(true, Ordering::Release);
        WatchdogOperation {
            armed: self.armed.clone(),
        }
    }
}

pub(super) struct WatchdogOperation {
    armed: Arc<AtomicBool>,
}

impl WatchdogOperation {
    pub fn complete(self) {
        self.armed.store(false, Ordering::Release);
    }
}

impl Drop for WatchdogOperation {
    fn drop(&mut self) {
        self.armed.store(false, Ordering::Release);
    }
}
