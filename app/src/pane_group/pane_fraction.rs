#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneSizeFraction {
    Half,
    Third,
    Quarter,
    TwoThirds,
    ThreeQuarters,
    FourFifths,
}

impl PaneSizeFraction {
    pub const MENU_ROWS: [[Self; 3]; 2] = [
        [Self::Half, Self::Third, Self::Quarter],
        [Self::TwoThirds, Self::ThreeQuarters, Self::FourFifths],
    ];
    pub const ALL: [Self; 6] = [
        Self::Half,
        Self::Third,
        Self::Quarter,
        Self::TwoThirds,
        Self::ThreeQuarters,
        Self::FourFifths,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Half => "1/2",
            Self::Third => "1/3",
            Self::Quarter => "1/4",
            Self::TwoThirds => "2/3",
            Self::ThreeQuarters => "3/4",
            Self::FourFifths => "4/5",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Half => "Resize pane to one half",
            Self::Third => "Resize pane to one third",
            Self::Quarter => "Resize pane to one quarter",
            Self::TwoThirds => "Resize pane to two thirds",
            Self::ThreeQuarters => "Resize pane to three quarters",
            Self::FourFifths => "Resize pane to four fifths",
        }
    }

    pub(crate) fn ratio(self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::Third => 1. / 3.,
            Self::Quarter => 0.25,
            Self::TwoThirds => 2. / 3.,
            Self::ThreeQuarters => 0.75,
            Self::FourFifths => 0.8,
        }
    }
}

pub(super) fn redistribute_flexes(
    flexes: &[f32],
    target_index: usize,
    fraction: PaneSizeFraction,
) -> Option<Vec<f32>> {
    if flexes.len() < 2 || target_index >= flexes.len() {
        return None;
    }

    let total = flexes.iter().sum::<f32>();
    if !total.is_finite() || total <= f32::EPSILON {
        return None;
    }

    let target = total * fraction.ratio();
    let remaining = total - target;
    let old_remaining = total - flexes[target_index];
    let sibling_count = flexes.len() - 1;
    let mut result = flexes.to_vec();

    if old_remaining > f32::EPSILON {
        let scale = remaining / old_remaining;
        for (index, flex) in result.iter_mut().enumerate() {
            if index != target_index {
                *flex *= scale;
            }
        }
    } else {
        let sibling_flex = remaining / sibling_count as f32;
        for (index, flex) in result.iter_mut().enumerate() {
            if index != target_index {
                *flex = sibling_flex;
            }
        }
    }

    result[target_index] = target;
    Some(result)
}

#[cfg(test)]
#[path = "pane_fraction_tests.rs"]
mod tests;
