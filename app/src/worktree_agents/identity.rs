use std::path::{Path, PathBuf};

use crate::terminal::model::session::{Session, SessionType};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ExecutionHostKey {
    Local,
    Remote(String),
}

impl ExecutionHostKey {
    pub(crate) fn from_session(session: &Session) -> Self {
        match session.session_type() {
            SessionType::Local => Self::Local,
            SessionType::WarpifiedRemote {
                host_id: Some(host_id),
            } => Self::Remote(format!("{host_id:?}")),
            SessionType::WarpifiedRemote { host_id: None } => {
                Self::Remote(format!("{}@{}", session.user(), session.hostname()))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RepositoryKey {
    host: ExecutionHostKey,
    common_dir: PathBuf,
}

impl RepositoryKey {
    pub(crate) fn new(host: ExecutionHostKey, common_dir: impl Into<PathBuf>) -> Self {
        Self {
            host,
            common_dir: normalize_path(common_dir.into()),
        }
    }

    pub(crate) fn host(&self) -> &ExecutionHostKey {
        &self.host
    }

    pub(crate) fn common_dir(&self) -> &Path {
        &self.common_dir
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct WorktreeKey {
    repository: RepositoryKey,
    path: PathBuf,
}

impl WorktreeKey {
    pub(crate) fn new(repository: RepositoryKey, path: impl Into<PathBuf>) -> Self {
        Self {
            repository,
            path: normalize_path(path.into()),
        }
    }

    pub(crate) fn repository(&self) -> &RepositoryKey {
        &self.repository
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        normalized.push(component);
    }
    normalized
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
