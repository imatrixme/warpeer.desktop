use super::*;

fn assert_flexes(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
    }
}

#[test]
fn target_fraction_preserves_sibling_proportions() {
    let resized = redistribute_flexes(&[1., 2., 1.], 0, PaneSizeFraction::Half).unwrap();

    assert_flexes(&resized, &[2., 4. / 3., 2. / 3.]);
}

#[test]
fn target_fraction_recovers_from_zero_sibling_flex() {
    let resized = redistribute_flexes(&[2., 0., 0.], 0, PaneSizeFraction::Quarter).unwrap();

    assert_flexes(&resized, &[0.5, 0.75, 0.75]);
}

#[test]
fn target_fraction_requires_a_sibling() {
    assert_eq!(redistribute_flexes(&[1.], 0, PaneSizeFraction::Half), None);
}

#[test]
fn pane_fraction_menu_groups_cover_small_and_large_sizes() {
    assert_eq!(
        PaneSizeFraction::MENU_ROWS[0].map(PaneSizeFraction::label),
        ["1/2", "1/3", "1/4"]
    );
    assert_eq!(
        PaneSizeFraction::MENU_ROWS[1].map(PaneSizeFraction::label),
        ["2/3", "3/4", "4/5"]
    );
}

#[test]
fn large_target_fraction_preserves_sibling_proportions() {
    let resized = redistribute_flexes(&[1., 2., 1.], 0, PaneSizeFraction::FourFifths).unwrap();

    assert_flexes(&resized, &[3.2, 0.533_333_36, 0.266_666_68]);
}
