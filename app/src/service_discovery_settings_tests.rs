use super::is_service_discovery_available;

#[test]
fn project_service_discovery_is_disabled() {
    assert!(!is_service_discovery_available());
}
