use std::time::Duration;

use super::default_grace_period;

#[test]
fn default_grace_period_matches_product_profile() {
    let expected = if cfg!(feature = "warpeer") {
        Duration::from_secs(10)
    } else {
        Duration::from_secs(60)
    };

    assert_eq!(default_grace_period(), expected);
}
