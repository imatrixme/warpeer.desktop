use super::*;

#[test]
fn token_and_age_labels_stay_compact() {
    assert_eq!(format_tokens(950), "950");
    assert_eq!(format_tokens(12_400), "12.4K");
    assert_eq!(format_age(75_000), "1m ago");
}

#[test]
fn context_icons_clamp_to_the_supported_range() {
    assert_eq!(context_icon(100), Icon::ContextRemaining100);
    assert_eq!(context_icon(0), Icon::ContextRemaining0);
}
