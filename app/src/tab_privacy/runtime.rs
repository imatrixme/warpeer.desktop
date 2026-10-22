use std::collections::HashMap;
use std::time::Instant;

use warpui::r#async::{SpawnedFutureHandle, Timer};
use warpui::{Entity, EntityId, ModelContext, SingletonEntity};

use super::TabRelockDelay;
use crate::pane_group::PaneId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationScope {
    Tab(EntityId),
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPrivacyState {
    Locked,
    Authenticating {
        revision: u64,
    },
    UnlockedViewed,
    UnlockedAway {
        deadline: Option<Instant>,
        generation: u64,
    },
}

#[derive(Clone, Copy, Debug)]
struct TabPrivacyEntry {
    state: TabPrivacyState,
    remembered_pane: Option<PaneId>,
    generation: u64,
}

impl Default for TabPrivacyEntry {
    fn default() -> Self {
        Self {
            state: TabPrivacyState::Locked,
            remembered_pane: None,
            generation: 0,
        }
    }
}

#[derive(Default)]
struct RuntimeState {
    entries: HashMap<EntityId, TabPrivacyEntry>,
    next_auth_revision: u64,
}

impl RuntimeState {
    fn protect(&mut self, tab_id: EntityId) -> bool {
        if self.entries.contains_key(&tab_id) {
            return false;
        }
        self.entries.insert(tab_id, TabPrivacyEntry::default());
        true
    }

    fn remove(&mut self, tab_id: EntityId) -> bool {
        self.entries.remove(&tab_id).is_some()
    }

    fn lock(&mut self, tab_id: EntityId) -> bool {
        let Some(entry) = self.entries.get_mut(&tab_id) else {
            return false;
        };
        if entry.state == TabPrivacyState::Locked {
            return false;
        }
        entry.generation = entry.generation.wrapping_add(1);
        entry.state = TabPrivacyState::Locked;
        true
    }

    fn lock_all(&mut self) -> Vec<EntityId> {
        let mut changed = Vec::new();
        for (tab_id, entry) in &mut self.entries {
            if entry.state != TabPrivacyState::Locked {
                entry.generation = entry.generation.wrapping_add(1);
                entry.state = TabPrivacyState::Locked;
                changed.push(*tab_id);
            }
        }
        changed
    }

    fn mark_viewed(&mut self, tab_id: EntityId) -> bool {
        let Some(entry) = self.entries.get_mut(&tab_id) else {
            return false;
        };
        if matches!(
            entry.state,
            TabPrivacyState::Locked | TabPrivacyState::Authenticating { .. }
        ) {
            return false;
        }
        entry.generation = entry.generation.wrapping_add(1);
        let changed = entry.state != TabPrivacyState::UnlockedViewed;
        entry.state = TabPrivacyState::UnlockedViewed;
        changed
    }

    fn mark_away(&mut self, tab_id: EntityId, now: Instant, delay: TabRelockDelay) -> bool {
        let Some(entry) = self.entries.get_mut(&tab_id) else {
            return false;
        };
        if matches!(
            entry.state,
            TabPrivacyState::Locked | TabPrivacyState::Authenticating { .. }
        ) {
            return false;
        }
        entry.generation = entry.generation.wrapping_add(1);
        let generation = entry.generation;
        entry.state = match delay.duration() {
            Some(duration) if duration.is_zero() => TabPrivacyState::Locked,
            Some(duration) => TabPrivacyState::UnlockedAway {
                deadline: Some(now + duration),
                generation,
            },
            None => TabPrivacyState::UnlockedAway {
                deadline: None,
                generation,
            },
        };
        true
    }

    fn unlock(
        &mut self,
        tab_id: EntityId,
        viewed: bool,
        now: Instant,
        delay: TabRelockDelay,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&tab_id) else {
            return false;
        };
        entry.generation = entry.generation.wrapping_add(1);
        entry.state = TabPrivacyState::UnlockedViewed;
        if !viewed {
            return self.mark_away(tab_id, now, delay);
        }
        true
    }

    fn begin_authentication(&mut self, scope: AuthenticationScope) -> Option<u64> {
        self.next_auth_revision = self.next_auth_revision.wrapping_add(1);
        let revision = self.next_auth_revision;
        let mut changed = false;
        for (tab_id, entry) in &mut self.entries {
            if matches!(scope, AuthenticationScope::All)
                || matches!(scope, AuthenticationScope::Tab(target) if target == *tab_id)
            {
                entry.generation = entry.generation.wrapping_add(1);
                entry.state = TabPrivacyState::Authenticating { revision };
                changed = true;
            }
        }
        changed.then_some(revision)
    }

    fn finish_authentication(
        &mut self,
        scope: AuthenticationScope,
        revision: u64,
        success: bool,
        viewed_tab: Option<EntityId>,
        now: Instant,
        delay: TabRelockDelay,
    ) -> Vec<EntityId> {
        let targets: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(tab_id, entry)| {
                let in_scope = matches!(scope, AuthenticationScope::All)
                    || matches!(scope, AuthenticationScope::Tab(target) if target == *tab_id);
                (in_scope && entry.state == TabPrivacyState::Authenticating { revision })
                    .then_some(*tab_id)
            })
            .collect();
        for tab_id in &targets {
            if success {
                self.unlock(*tab_id, viewed_tab == Some(*tab_id), now, delay);
            } else {
                self.lock(*tab_id);
            }
        }
        targets
    }

    fn expire_due(&mut self, now: Instant) -> Vec<EntityId> {
        let due: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(tab_id, entry)| match entry.state {
                TabPrivacyState::UnlockedAway {
                    deadline: Some(deadline),
                    generation,
                } if deadline <= now && generation == entry.generation => Some(*tab_id),
                TabPrivacyState::Locked
                | TabPrivacyState::Authenticating { .. }
                | TabPrivacyState::UnlockedViewed
                | TabPrivacyState::UnlockedAway { .. } => None,
            })
            .collect();
        for tab_id in &due {
            self.lock(*tab_id);
        }
        due
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|entry| match entry.state {
                TabPrivacyState::UnlockedAway {
                    deadline: Some(deadline),
                    generation,
                } if generation == entry.generation => Some(deadline),
                TabPrivacyState::Locked
                | TabPrivacyState::Authenticating { .. }
                | TabPrivacyState::UnlockedViewed
                | TabPrivacyState::UnlockedAway { .. } => None,
            })
            .min()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TabPrivacyRuntimeEvent {
    Changed(Vec<EntityId>),
}

pub struct TabPrivacyRuntime {
    state: RuntimeState,
    timer: Option<SpawnedFutureHandle>,
}

impl TabPrivacyRuntime {
    pub fn new() -> Self {
        Self {
            state: RuntimeState::default(),
            timer: None,
        }
    }

    pub fn protected_count(&self) -> usize {
        self.state.entries.len()
    }

    pub fn is_protected(&self, tab_id: EntityId) -> bool {
        self.state.entries.contains_key(&tab_id)
    }

    pub fn state(&self, tab_id: EntityId) -> Option<TabPrivacyState> {
        self.state.entries.get(&tab_id).map(|entry| entry.state)
    }

    pub fn is_locked(&self, tab_id: EntityId) -> bool {
        matches!(
            self.state(tab_id),
            Some(TabPrivacyState::Locked | TabPrivacyState::Authenticating { .. })
        )
    }

    pub fn all_protected_locked(&self) -> bool {
        !self.state.entries.is_empty()
            && self.state.entries.values().all(|entry| {
                matches!(
                    entry.state,
                    TabPrivacyState::Locked | TabPrivacyState::Authenticating { .. }
                )
            })
    }

    pub fn remembered_pane(&self, tab_id: EntityId) -> Option<PaneId> {
        self.state.entries.get(&tab_id)?.remembered_pane
    }

    pub fn remember_pane(&mut self, tab_id: EntityId, pane_id: PaneId) {
        if let Some(entry) = self.state.entries.get_mut(&tab_id) {
            entry.remembered_pane = Some(pane_id);
        }
    }

    pub fn protect(&mut self, tab_id: EntityId, ctx: &mut ModelContext<Self>) {
        if self.state.protect(tab_id) {
            self.emit_changed(vec![tab_id], ctx);
        }
    }

    pub fn remove_protection(&mut self, tab_id: EntityId, ctx: &mut ModelContext<Self>) {
        if self.state.remove(tab_id) {
            self.reschedule(ctx);
            self.emit_changed(vec![tab_id], ctx);
        }
    }

    pub fn lock(&mut self, tab_id: EntityId, ctx: &mut ModelContext<Self>) {
        if self.state.lock(tab_id) {
            self.reschedule(ctx);
            self.emit_changed(vec![tab_id], ctx);
        }
    }

    pub fn lock_all(&mut self, ctx: &mut ModelContext<Self>) {
        let changed = self.state.lock_all();
        if !changed.is_empty() {
            self.reschedule(ctx);
            self.emit_changed(changed, ctx);
        }
    }

    pub fn clear(&mut self, ctx: &mut ModelContext<Self>) {
        let changed = self.state.entries.keys().copied().collect();
        self.state.entries.clear();
        self.reschedule(ctx);
        self.emit_changed(changed, ctx);
    }

    pub fn mark_viewed(&mut self, tab_id: EntityId, ctx: &mut ModelContext<Self>) {
        if self.state.mark_viewed(tab_id) {
            self.reschedule(ctx);
            self.emit_changed(vec![tab_id], ctx);
        }
    }

    pub fn mark_away(
        &mut self,
        tab_id: EntityId,
        delay: TabRelockDelay,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.state.mark_away(tab_id, Instant::now(), delay) {
            self.reschedule(ctx);
            self.emit_changed(vec![tab_id], ctx);
        }
    }

    pub fn begin_authentication(
        &mut self,
        scope: AuthenticationScope,
        ctx: &mut ModelContext<Self>,
    ) -> Option<u64> {
        let revision = self.state.begin_authentication(scope)?;
        self.reschedule(ctx);
        let changed = match scope {
            AuthenticationScope::Tab(tab_id) => vec![tab_id],
            AuthenticationScope::All => self.state.entries.keys().copied().collect(),
        };
        self.emit_changed(changed, ctx);
        Some(revision)
    }

    pub fn finish_authentication(
        &mut self,
        scope: AuthenticationScope,
        revision: u64,
        success: bool,
        viewed_tab: Option<EntityId>,
        delay: TabRelockDelay,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let changed = self.state.finish_authentication(
            scope,
            revision,
            success,
            viewed_tab,
            Instant::now(),
            delay,
        );
        if !changed.is_empty() {
            self.reschedule(ctx);
            self.emit_changed(changed, ctx);
            true
        } else {
            false
        }
    }

    pub fn apply_delay_change(&mut self, delay: TabRelockDelay, ctx: &mut ModelContext<Self>) {
        let away: Vec<_> = self
            .state
            .entries
            .iter()
            .filter_map(|(tab_id, entry)| {
                matches!(entry.state, TabPrivacyState::UnlockedAway { .. }).then_some(*tab_id)
            })
            .collect();
        let now = Instant::now();
        for tab_id in &away {
            self.state.mark_away(*tab_id, now, delay);
        }
        self.reschedule(ctx);
        self.emit_changed(away, ctx);
    }

    fn reschedule(&mut self, ctx: &mut ModelContext<Self>) {
        if let Some(timer) = self.timer.take() {
            timer.abort();
        }
        let Some(deadline) = self.state.next_deadline() else {
            return;
        };
        let wait = deadline.saturating_duration_since(Instant::now());
        self.timer = Some(ctx.spawn(Timer::after(wait), |model, _, ctx| {
            model.timer = None;
            let changed = model.state.expire_due(Instant::now());
            model.reschedule(ctx);
            model.emit_changed(changed, ctx);
        }));
    }

    fn emit_changed(&self, changed: Vec<EntityId>, ctx: &mut ModelContext<Self>) {
        if !changed.is_empty() {
            ctx.emit(TabPrivacyRuntimeEvent::Changed(changed));
        }
    }
}

impl Entity for TabPrivacyRuntime {
    type Event = TabPrivacyRuntimeEvent;
}

impl SingletonEntity for TabPrivacyRuntime {}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
