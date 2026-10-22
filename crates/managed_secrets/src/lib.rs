pub mod client;
#[cfg(feature = "runtime")]
mod envelope;
#[cfg(not(feature = "runtime"))]
#[path = "envelope_disabled.rs"]
mod envelope;
#[cfg(feature = "runtime")]
mod gcp;
#[cfg(not(feature = "runtime"))]
#[path = "gcp_disabled.rs"]
mod gcp;
#[cfg(feature = "runtime")]
mod manager;
#[cfg(not(feature = "runtime"))]
#[path = "manager_disabled.rs"]
mod manager;
mod secret_value;

pub use client::TaskIdentityToken;
pub use envelope::{UploadKey, init as init_envelope};
pub use gcp::{
    GcpCredentials, GcpFederationConfig, GcpWorkloadIdentityFederationError,
    GcpWorkloadIdentityFederationToken, PrepareGcpCredentialsError,
};
pub use manager::{ActorProvider, ManagedSecretManager};
pub use secret_value::ManagedSecretValue;
