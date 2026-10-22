use super::*;

const SPEC: ObservationSourceSpec = ObservationSourceSpec {
    id: "test",
    capabilities: SourceCapabilities {
        progressive_snapshots: true,
        structured_events: true,
        diagnostics: true,
        cancellation: true,
        capability_detection: true,
    },
    foreground_interval_ms: 5_000,
    background_interval_ms: 30_000,
    expiry_ms: 20_000,
};

#[test]
fn scheduler_requires_a_consumer_and_one_in_flight_query() {
    let mut scheduler = SourceScheduler::new(SPEC);
    assert_eq!(scheduler.begin(0, None, false), None);
    assert!(
        scheduler
            .begin(0, Some(SourceVisibility::Foreground), false)
            .is_some()
    );
    assert_eq!(
        scheduler.begin(0, Some(SourceVisibility::Foreground), true),
        None
    );
}

#[test]
fn stale_completion_is_discarded_after_cancellation() {
    let mut scheduler = SourceScheduler::new(SPEC);
    let lease = scheduler
        .begin(0, Some(SourceVisibility::Foreground), false)
        .unwrap();
    scheduler.cancel(SourceDiagnostic::Cancelled);

    assert!(!scheduler.complete(
        lease,
        SourceHealth::Healthy,
        None,
        1,
        SourceVisibility::Foreground,
    ));
    assert_eq!(scheduler.diagnostic(), Some(SourceDiagnostic::Cancelled));
}

#[test]
fn background_sources_use_the_slower_cadence() {
    let mut scheduler = SourceScheduler::new(SPEC);
    let lease = scheduler
        .begin(0, Some(SourceVisibility::Background), false)
        .unwrap();
    assert!(scheduler.complete(
        lease,
        SourceHealth::Healthy,
        None,
        0,
        SourceVisibility::Background,
    ));
    assert_eq!(
        scheduler.begin(29_999, Some(SourceVisibility::Background), false),
        None
    );
    assert!(
        scheduler
            .begin(30_000, Some(SourceVisibility::Background), false)
            .is_some()
    );
}

#[test]
fn source_spec_carries_capabilities_and_expiry() {
    let scheduler = SourceScheduler::new(SPEC);
    assert_eq!(scheduler.spec().id, "test");
    assert!(scheduler.spec().capabilities.progressive_snapshots);
    assert_eq!(scheduler.spec().expiry_ms, 20_000);
}
