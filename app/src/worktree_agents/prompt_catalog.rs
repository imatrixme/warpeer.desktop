use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity as _};

use super::catalog::decode_menu_entries;
use super::coordinator::WorktreeAgentCoordinator;
use super::identity::ExecutionHostKey;
use crate::terminal::model::session::{SessionId, Sessions};

pub(crate) fn publish_prompt_catalog<T: Entity>(
    terminal_view_id: Option<EntityId>,
    session_id: Option<SessionId>,
    sessions: &ModelHandle<Sessions>,
    values: Option<&[String]>,
    ctx: &mut ModelContext<T>,
) {
    let Some(terminal_view_id) = terminal_view_id else {
        return;
    };
    let entries = values.map(decode_menu_entries).unwrap_or_default();
    let coordinator = WorktreeAgentCoordinator::handle(ctx);
    if entries.is_empty() {
        coordinator.update(ctx, |coordinator, ctx| {
            coordinator.clear_catalog(terminal_view_id, ctx);
        });
        return;
    }
    let Some(host) = host_for_session(session_id, sessions, ctx) else {
        return;
    };
    coordinator.update(ctx, |coordinator, ctx| {
        coordinator.publish_catalog(terminal_view_id, host, &entries, ctx);
    });
}

fn host_for_session<T: Entity>(
    session_id: Option<SessionId>,
    sessions: &ModelHandle<Sessions>,
    ctx: &ModelContext<T>,
) -> Option<ExecutionHostKey> {
    let session = sessions.as_ref(ctx).get(session_id?)?;
    Some(ExecutionHostKey::from_session(&session))
}
