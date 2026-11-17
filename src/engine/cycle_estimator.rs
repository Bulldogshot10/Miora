use chrono::{Days, NaiveDate};

use crate::domain::cycle::cycle_length_days;
use crate::domain::prediction::{CyclePrediction, PredictionConfidence};
use crate::engine::statistics::median;

pub fn estimate_next_period(starts: &[NaiveDate]) -> Option<CyclePrediction> {
    let mut ordered = starts.to_vec();
    ordered.sort_unstable();
    ordered.dedup();

    let lengths: Vec<f64> = ordered
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]))
        .filter(|days| (15..=60).contains(days))
        .map(f64::from)
        .collect();

    // Require at least two completed intervals before producing a personal estimate.
    if lengths.len() < 2 {
        return None;
    }

    let center = median(&lengths)?;
    let mad = {
        let deviations: Vec<f64> = lengths
            .iter()
            .map(|length| (length - center).abs())
            .collect();
        median(&deviations).unwrap_or(0.0)
    };

    let radius = (mad * 1.4826).ceil().max(1.0) as u64;
    let expected_days = center.round().max(1.0) as u64;
    let latest_start = *ordered.last()?;

    let expected_start = latest_start.checked_add_days(Days::new(expected_days))?;
    let earliest_start =
        latest_start.checked_add_days(Days::new(expected_days.saturating_sub(radius)))?;
    let latest_window_start =
        latest_start.checked_add_days(Days::new(expected_days.saturating_add(radius)))?;

    let confidence = match lengths.len() {
        0..=2 => PredictionConfidence::Early,
        3..=5 => PredictionConfidence::Developing,
        _ => PredictionConfidence::Established,
    };

    Some(CyclePrediction {
        expected_start,
        earliest_start,
        latest_start: latest_window_start,
        cycle_intervals_used: lengths.len(),
        confidence,
    })
}
