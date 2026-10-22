const SNAP_DISTANCE: f32 = 8.;
const SNAP_FRACTIONS: [f32; 5] = [0.25, 1. / 3., 0.5, 2. / 3., 0.75];

pub(super) fn snap_boundary(raw_boundary: f32, total_size: f32, min: f32, max: f32) -> f32 {
    if total_size <= 0. || min > max {
        return raw_boundary;
    }

    SNAP_FRACTIONS
        .into_iter()
        .map(|fraction| total_size * fraction)
        .filter(|target| *target >= min && *target <= max)
        .min_by(|left, right| {
            (raw_boundary - *left)
                .abs()
                .total_cmp(&(raw_boundary - *right).abs())
        })
        .filter(|target| (raw_boundary - *target).abs() <= SNAP_DISTANCE)
        .unwrap_or(raw_boundary)
}

#[cfg(test)]
#[path = "resize_snap_tests.rs"]
mod tests;
