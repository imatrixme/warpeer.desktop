use std::path::PathBuf;
use std::sync::Arc;

use super::{MAX_CONCURRENT_COMMANDS_PER_CONTROL_MASTER, RemoteCommandExecutor};

#[test]
fn executors_on_the_same_control_master_share_permits() {
    let path = PathBuf::from("/tmp/warp-test-shared-control-master");
    let first = RemoteCommandExecutor::new(path.clone(), None);
    let second = RemoteCommandExecutor::new(path, None);
    let other = RemoteCommandExecutor::new(PathBuf::from("/tmp/warp-test-other-master"), None);

    assert!(Arc::ptr_eq(&first.permits, &second.permits));
    assert!(!Arc::ptr_eq(&first.permits, &other.permits));
    assert_eq!(
        first.permits.available_permits(),
        MAX_CONCURRENT_COMMANDS_PER_CONTROL_MASTER
    );
}
