use std::time::{Duration, Instant};

use warpui::EntityId;

use super::*;

fn tab_id(value: usize) -> EntityId {
    EntityId::from_usize(value)
}

#[test]
fn leaving_returning_and_leaving_again_resets_deadline() {
    let mut state = RuntimeState::default();
    let tab = tab_id(1);
    let start = Instant::now();
    state.protect(tab);
    state.unlock(tab, true, start, TabRelockDelay::Minute1);

    assert!(state.mark_away(tab, start, TabRelockDelay::Minute1));
    let first_deadline = state.next_deadline().unwrap();
    assert!(state.mark_viewed(tab));
    assert_eq!(state.next_deadline(), None);

    let later = start + Duration::from_secs(20);
    assert!(state.mark_away(tab, later, TabRelockDelay::Minute1));
    assert_eq!(state.next_deadline(), Some(later + Duration::from_secs(60)));
    assert!(state.next_deadline().unwrap() > first_deadline);
}

#[test]
fn stale_expiration_does_not_lock_a_returned_tab() {
    let mut state = RuntimeState::default();
    let tab = tab_id(2);
    let start = Instant::now();
    state.protect(tab);
    state.unlock(tab, false, start, TabRelockDelay::Seconds30);
    assert!(state.mark_viewed(tab));

    assert!(state.expire_due(start + Duration::from_secs(31)).is_empty());
    assert_eq!(
        state.entries.get(&tab).unwrap().state,
        TabPrivacyState::UnlockedViewed
    );
}

#[test]
fn immediate_and_never_delays_apply_without_polling() {
    let mut state = RuntimeState::default();
    let immediate = tab_id(3);
    state.protect(immediate);
    state.unlock(
        immediate,
        false,
        Instant::now(),
        TabRelockDelay::Immediately,
    );
    assert_eq!(
        state.entries.get(&immediate).unwrap().state,
        TabPrivacyState::Locked
    );

    let never = tab_id(4);
    state.protect(never);
    state.unlock(never, false, Instant::now(), TabRelockDelay::Never);
    assert!(matches!(
        state.entries.get(&never).unwrap().state,
        TabPrivacyState::UnlockedAway { deadline: None, .. }
    ));
    assert_eq!(state.next_deadline(), None);
}

#[test]
fn authentication_revision_rejects_stale_completion() {
    let mut state = RuntimeState::default();
    let tab = tab_id(5);
    state.protect(tab);
    let first = state
        .begin_authentication(AuthenticationScope::Tab(tab))
        .unwrap();
    let second = state
        .begin_authentication(AuthenticationScope::Tab(tab))
        .unwrap();

    assert!(
        state
            .finish_authentication(
                AuthenticationScope::Tab(tab),
                first,
                true,
                Some(tab),
                Instant::now(),
                TabRelockDelay::Minute1,
            )
            .is_empty()
    );
    assert_eq!(
        state.entries.get(&tab).unwrap().state,
        TabPrivacyState::Authenticating { revision: second }
    );
}

#[test]
fn authenticating_tabs_remain_locked_until_authentication_succeeds() {
    let mut runtime = TabPrivacyRuntime::new();
    let tab = tab_id(12);
    runtime.state.protect(tab);
    runtime
        .state
        .begin_authentication(AuthenticationScope::Tab(tab))
        .expect("protected tab should authenticate");

    assert!(runtime.is_locked(tab));
    assert!(runtime.all_protected_locked());
}

#[test]
fn global_lock_transitions_every_unlocked_entry() {
    let mut state = RuntimeState::default();
    let first = tab_id(6);
    let second = tab_id(7);
    for tab in [first, second] {
        state.protect(tab);
        state.unlock(tab, true, Instant::now(), TabRelockDelay::Minute1);
    }

    let mut changed = state.lock_all();
    changed.sort();
    assert_eq!(changed, vec![first, second]);
    assert!(
        state
            .entries
            .values()
            .all(|entry| entry.state == TabPrivacyState::Locked)
    );
}

#[test]
fn successful_all_scope_authentication_applies_viewed_and_away_states() {
    let mut state = RuntimeState::default();
    let viewed = tab_id(8);
    let away = tab_id(9);
    state.protect(viewed);
    state.protect(away);
    let revision = state
        .begin_authentication(AuthenticationScope::All)
        .expect("protected tabs should authenticate");
    let now = Instant::now();

    let mut changed = state.finish_authentication(
        AuthenticationScope::All,
        revision,
        true,
        Some(viewed),
        now,
        TabRelockDelay::Seconds30,
    );

    changed.sort();
    assert_eq!(changed, vec![viewed, away]);
    assert_eq!(
        state.entries.get(&viewed).unwrap().state,
        TabPrivacyState::UnlockedViewed
    );
    assert!(matches!(
        state.entries.get(&away).unwrap().state,
        TabPrivacyState::UnlockedAway {
            deadline: Some(deadline),
            ..
        } if deadline == now + Duration::from_secs(30)
    ));
}

#[test]
fn failed_authentication_returns_the_target_to_locked() {
    let mut state = RuntimeState::default();
    let tab = tab_id(10);
    state.protect(tab);
    let revision = state
        .begin_authentication(AuthenticationScope::Tab(tab))
        .expect("protected tab should authenticate");

    assert_eq!(
        state.finish_authentication(
            AuthenticationScope::Tab(tab),
            revision,
            false,
            Some(tab),
            Instant::now(),
            TabRelockDelay::Minute1,
        ),
        vec![tab]
    );
    assert_eq!(
        state.entries.get(&tab).unwrap().state,
        TabPrivacyState::Locked
    );
}

#[test]
fn locking_during_authentication_rejects_the_late_completion() {
    let mut state = RuntimeState::default();
    let tab = tab_id(11);
    state.protect(tab);
    let revision = state
        .begin_authentication(AuthenticationScope::Tab(tab))
        .expect("protected tab should authenticate");
    assert!(state.lock(tab));

    assert!(
        state
            .finish_authentication(
                AuthenticationScope::Tab(tab),
                revision,
                true,
                Some(tab),
                Instant::now(),
                TabRelockDelay::Minute1,
            )
            .is_empty()
    );
    assert_eq!(
        state.entries.get(&tab).unwrap().state,
        TabPrivacyState::Locked
    );
}
