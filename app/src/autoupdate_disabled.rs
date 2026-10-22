use std::sync::Arc;

use anyhow::{Result, bail};
use channel_versions::{Changelog, VersionInfo};
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, ViewContext};

use crate::server::server_api::ServerApi;
use crate::workspace::Workspace;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AutoupdateStage {
    #[default]
    NoUpdateAvailable,
    CheckingForUpdate,
    DownloadingUpdate,
    UnableToUpdateToNewVersion {
        new_version: VersionInfo,
    },
    UpdateReady {
        new_version: VersionInfo,
        update_id: String,
    },
    Updating {
        new_version: VersionInfo,
        update_id: String,
    },
    UnableToLaunchNewVersion {
        new_version: VersionInfo,
    },
    UpdatedPendingRestart {
        new_version: VersionInfo,
    },
}

impl AutoupdateStage {
    pub fn ready_for_update(&self) -> bool {
        false
    }

    pub fn available_new_version(&self) -> Option<&VersionInfo> {
        None
    }
}

pub struct AutoupdateState {
    stage: AutoupdateStage,
}

impl AutoupdateState {
    pub fn new(_server_api: Arc<ServerApi>) -> Self {
        Self {
            stage: AutoupdateStage::NoUpdateAvailable,
        }
    }

    pub fn register(ctx: &mut AppContext, server_api: Arc<ServerApi>) {
        ctx.add_singleton_model(|_| Self::new(server_api));
    }

    pub fn start_polling(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn manually_check_for_update(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn maybe_daily_check_for_update(&mut self, _ctx: &mut ModelContext<Self>) {}
}

impl Entity for AutoupdateState {
    type Event = AutoupdateStateEvent;
}

impl SingletonEntity for AutoupdateState {}

pub enum AutoupdateStateEvent {
    CheckComplete {
        result: Box<Result<UpdateReady>>,
        request_type: RequestType,
    },
    UpdateAvailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateReady {
    Yes {
        new_version: VersionInfo,
        update_id: String,
    },
    CanDownload {
        new_version: VersionInfo,
        update_id: String,
    },
    No,
}

pub enum ReadyForRelaunch {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    ManualCheck,
    Poll,
    DailyCheck,
}

pub fn get_update_state(app: &AppContext) -> AutoupdateStage {
    AutoupdateState::as_ref(app).stage.clone()
}

pub fn apply_update(
    _initiating_workspace: &mut Workspace,
    _ctx: &mut ViewContext<Workspace>,
) -> Result<ReadyForRelaunch> {
    bail!("Autoupdate support is not compiled into this product")
}

pub fn initiate_relaunch_for_update(_app: &mut AppContext) {}

pub fn apply_pending_update<F>(_app: &mut AppContext, _on_update_complete: F) -> bool
where
    F: FnOnce(&mut AppContext) + Send + 'static,
{
    false
}

pub fn cancel_relaunch(_app: &mut AppContext) {}

pub fn spawn_child_if_necessary(_app: &mut AppContext) {}

pub fn manually_download_new_version(_ctx: &mut AppContext) {}

pub(crate) fn check_and_report_update_errors(_ctx: &mut AppContext) {}

pub fn remove_old_executable() -> Result<()> {
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub struct RelaunchModel;

impl RelaunchModel {
    pub fn new() -> Self {
        Self
    }
}

impl Entity for RelaunchModel {
    type Event = ();
}

impl SingletonEntity for RelaunchModel {}

pub fn is_incoming_version_past_current(_version: Option<&str>) -> bool {
    false
}

pub async fn get_current_changelog(_server_api: Arc<ServerApi>) -> Result<Option<Changelog>> {
    Ok(None)
}
