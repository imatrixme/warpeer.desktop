use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use warpui::r#async::FutureExt as _;

const DIRTY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_STATUS_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirtyProbeResult {
    Clean,
    Dirty,
    Unavailable,
}

pub(crate) async fn query_dirty(path: &Path) -> DirtyProbeResult {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("--untracked-files=no")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match command.output().with_timeout(DIRTY_TIMEOUT).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Ok(_)) | Ok(Err(_)) | Err(_) => return DirtyProbeResult::Unavailable,
    };
    classify_status(&output.stdout)
}

fn classify_status(output: &[u8]) -> DirtyProbeResult {
    if output.len() > MAX_STATUS_BYTES || output.iter().any(|byte| !byte.is_ascii_whitespace()) {
        DirtyProbeResult::Dirty
    } else {
        DirtyProbeResult::Clean
    }
}

#[cfg(test)]
#[path = "catalog_dirty_tests.rs"]
mod tests;
