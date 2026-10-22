use anyhow::{Result, anyhow};
use async_trait::async_trait;
use warp_graphql::mutations::upsert_runner::UpsertRunnerInput;
use warp_graphql::queries::get_runners::{Runner, RunnerSortBy};

use super::ServerApi;

pub struct UpsertedRunner {
    pub runner: Runner,
    pub is_update: bool,
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait FactoryClient: 'static + Send + Sync {
    async fn get_runners(&self, sort_by: Option<RunnerSortBy>) -> Result<Vec<Runner>>;

    async fn upsert_runner(&self, input: UpsertRunnerInput) -> Result<UpsertedRunner>;

    async fn delete_runner(&self, uid: String) -> Result<String>;
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl FactoryClient for ServerApi {
    async fn get_runners(&self, sort_by: Option<RunnerSortBy>) -> Result<Vec<Runner>> {
        let _ = sort_by;
        Err(anyhow!("Factory is unavailable in this product profile"))
    }

    async fn upsert_runner(&self, input: UpsertRunnerInput) -> Result<UpsertedRunner> {
        drop(input);
        Err(anyhow!("Factory is unavailable in this product profile"))
    }

    async fn delete_runner(&self, uid: String) -> Result<String> {
        drop(uid);
        Err(anyhow!("Factory is unavailable in this product profile"))
    }
}
