use std::sync::Arc;

pub use cloud_object_client::ObjectUpdateMessage;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::server::server_api::object::ObjectClient;

pub enum ListenerEvent {}

pub struct Listener;

impl Listener {
    pub fn new(cloud_objects_client: Arc<dyn ObjectClient>, ctx: &mut ModelContext<Self>) -> Self {
        let _ = (cloud_objects_client, ctx);
        Self
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        let _ = ctx;
        Self
    }

    pub fn has_current_subscription_abort_handle(&self) -> bool {
        false
    }
}

impl Entity for Listener {
    type Event = ListenerEvent;
}

impl SingletonEntity for Listener {}
