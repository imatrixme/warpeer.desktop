use super::ServiceComposition;

#[test]
fn local_only_composition_disables_hosted_services() {
    assert!(!ServiceComposition::LocalOnly.allows_hosted_services());
    assert!(!ServiceComposition::LocalOnly.allows_product_education());
}

#[test]
fn full_warp_composition_keeps_hosted_services() {
    assert!(ServiceComposition::FullWarp.allows_hosted_services());
    assert!(ServiceComposition::FullWarp.allows_product_education());
}
