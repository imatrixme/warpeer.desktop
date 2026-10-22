use warpui::{Entity, ModelContext, SingletonEntity};

pub(crate) struct RemoteAgentContext;

impl RemoteAgentContext {
    pub(crate) fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }
}

impl Entity for RemoteAgentContext {
    type Event = ();
}

impl SingletonEntity for RemoteAgentContext {}
