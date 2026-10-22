use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use warp_graphql::managed_secrets::ManagedSecret;
use warpui_core::{Entity, SingletonEntity};

use crate::ManagedSecretValue;
use crate::client::{IdentityTokenOptions, ManagedSecretsClient, SecretOwner, TaskIdentityToken};
use crate::gcp::{GcpWorkloadIdentityFederationError, GcpWorkloadIdentityFederationToken};

pub struct ManagedSecretManager;

pub trait ActorProvider: Send + Sync + 'static {
    fn actor_uid(&self) -> Option<String>;
}

impl ManagedSecretManager {
    pub fn new(
        _client: Arc<dyn ManagedSecretsClient>,
        _actor_provider: Arc<dyn ActorProvider>,
    ) -> Self {
        Self
    }

    pub fn create_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _value: ManagedSecretValue,
        _description: Option<String>,
    ) -> impl Future<Output = anyhow::Result<ManagedSecret>> + use<> {
        async { Err(unavailable()) }
    }

    pub fn delete_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
    ) -> impl Future<Output = anyhow::Result<()>> + use<> {
        async { Err(unavailable()) }
    }

    pub fn update_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _value: Option<ManagedSecretValue>,
        _description: Option<String>,
    ) -> impl Future<Output = anyhow::Result<ManagedSecret>> + use<> {
        async { Err(unavailable()) }
    }

    pub fn list_secrets(&self) -> impl Future<Output = anyhow::Result<Vec<ManagedSecret>>> + use<> {
        async { Ok(Vec::new()) }
    }

    pub fn get_task_secrets(
        &self,
        _task_id: String,
    ) -> impl Future<Output = anyhow::Result<HashMap<String, ManagedSecretValue>>> + use<> {
        async { Err(unavailable()) }
    }

    pub fn issue_task_identity_token(
        &self,
        _options: IdentityTokenOptions,
    ) -> impl Future<Output = anyhow::Result<TaskIdentityToken>> + use<> {
        async { Err(unavailable()) }
    }

    pub fn issue_gcp_workload_identity_federation_token(
        &self,
        _audience: String,
        _token_type: String,
        _requested_duration: Duration,
    ) -> impl Future<
        Output = Result<GcpWorkloadIdentityFederationToken, GcpWorkloadIdentityFederationError>,
    > + use<> {
        async {
            Err(GcpWorkloadIdentityFederationError::new(
                "managed secret identity tokens are unavailable in this product profile",
            ))
        }
    }
}

fn unavailable() -> anyhow::Error {
    anyhow::anyhow!("managed secrets are unavailable in this product profile")
}

impl Entity for ManagedSecretManager {
    type Event = ();
}

impl SingletonEntity for ManagedSecretManager {}
