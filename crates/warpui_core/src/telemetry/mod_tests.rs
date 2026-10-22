use chrono::Utc;

use super::{flush_events, record_app_active_event, record_event, record_identify_user_event};

#[cfg(feature = "no_telemetry")]
#[test]
fn no_telemetry_build_never_queues_events() {
    let anonymous_id = "anonymous".to_owned();
    record_event(
        None,
        anonymous_id.clone(),
        "Event".into(),
        None,
        false,
        Utc::now(),
    );
    record_identify_user_event("user".to_owned(), anonymous_id.clone(), Utc::now());
    record_app_active_event(None, anonymous_id, Utc::now());

    assert!(flush_events().is_empty());
}

#[cfg(not(feature = "no_telemetry"))]
#[test]
fn telemetry_build_queues_events() {
    flush_events();
    record_event(
        None,
        "anonymous".to_owned(),
        "Event".into(),
        None,
        false,
        Utc::now(),
    );

    assert!(!flush_events().is_empty());
}
