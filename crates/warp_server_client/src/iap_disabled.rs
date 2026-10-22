use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use instant::Instant;
use warp_core::channel::IapConfig;
use warpui_core::r#async::BoxFuture;
use warpui_core::{AppContext, Entity, ModelContext, SingletonEntity};

pub type PathResolver = Box<dyn Fn(&mut AppContext) -> BoxFuture<'static, Option<String>>>;

pub trait IapIdentityTokenMinter: Send + Sync + 'static {
    fn mint_identity_token(
        &self,
        audience: String,
        requested_duration: Duration,
    ) -> BoxFuture<'static, Result<String>>;
}

#[derive(Clone)]
pub struct ManagedIapMint;

impl ManagedIapMint {
    pub fn new(_minter: Arc<dyn IapIdentityTokenMinter>) -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
pub struct CachedToken {
    pub token: String,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub enum IapCredentialsState {
    Missing,
    Refreshing {
        previous: Option<CachedToken>,
    },
    Loaded(CachedToken),
    Failed {
        message: String,
        previous: Option<CachedToken>,
    },
}

pub struct IapState {
    audiences: String,
    service_account_email: String,
}

impl IapState {
    pub fn new(config: &IapConfig) -> Self {
        Self {
            audiences: config.audiences.to_string(),
            service_account_email: config.service_account_email.to_string(),
        }
    }

    pub fn get_cached(&self) -> Option<String> {
        None
    }

    pub fn proxy_auth_header(&self) -> Option<(&'static str, String)> {
        None
    }

    pub fn state(&self) -> IapCredentialsState {
        IapCredentialsState::Missing
    }

    pub fn audiences(&self) -> &str {
        &self.audiences
    }

    pub fn service_account_email(&self) -> &str {
        &self.service_account_email
    }
}

impl http_client::iap::IapTokenProvider for IapState {
    fn cached_token(&self) -> Option<String> {
        None
    }
}

pub struct IapManager;

pub enum IapManagerEvent {
    StateChanged,
    AccessUnavailable,
    RefreshFailed {
        message: String,
        is_first_failure_of_streak: bool,
    },
}

impl IapManager {
    pub fn new(
        _state: Option<Arc<IapState>>,
        _path_resolver: PathResolver,
        _managed_mint: Option<ManagedIapMint>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    pub fn is_enabled(&self) -> bool {
        false
    }

    pub fn state(&self) -> Option<IapCredentialsState> {
        None
    }

    pub fn iap_state(&self) -> Option<Arc<IapState>> {
        None
    }

    pub fn has_valid_token(&self) -> bool {
        false
    }

    pub fn ensure_access(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn handle_challenge(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn start_refresh(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn check_ws_connect_error(&mut self, _err: &anyhow::Error, _ctx: &mut ModelContext<Self>) {}
}

pub fn ws_connect_is_iap_challenge(_err: &anyhow::Error) -> bool {
    false
}

impl Entity for IapManager {
    type Event = IapManagerEvent;
}

impl SingletonEntity for IapManager {}
