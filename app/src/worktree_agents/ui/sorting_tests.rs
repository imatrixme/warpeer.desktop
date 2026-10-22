use super::*;

#[test]
fn optional_metrics_keep_unavailable_values_last() {
    assert_eq!(
        compare_optional(Some(20_u64), None, true),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        compare_optional(None, Some(20_u64), false),
        std::cmp::Ordering::Greater
    );
}
