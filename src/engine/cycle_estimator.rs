use jiff::{ToSpan, civil::Date as NaiveDate};

use crate::domain::cycle::cycle_length_days;
use crate::domain::prediction::{CyclePrediction, PredictionConfidence};
use crate::engine::changepoint::detect_cycle_regimes;
use crate::engine::hierarchical::{PopulationPrior, infer_cycle_length};
use crate::engine::observation::{CycleObservation, CycleObservationKind};
use crate::engine::statistics::median;
use crate::engine::survival::predict_next_cycle;
use crate::engine::{MAX_CYCLE_LENGTH, MIN_CYCLE_LENGTH};

/// Predict the next period using changepoint-weighted hierarchical survival inference.
pub fn estimate_next_period(starts: &[NaiveDate]) -> Option<CyclePrediction> {
    estimate_with_censoring(starts, None)
}

/// Predict using the active cycle as a right-censored observation on the supplied date.
pub fn estimate_next_period_at(
    starts: &[NaiveDate],
    today: NaiveDate,
) -> Option<CyclePrediction> {
    let latest = starts.iter().copied().filter(|date| *date <= today).max()?;
    let elapsed = (today - latest).get_days().max(0) as f64;
    estimate_with_censoring(
        &starts
            .iter()
            .copied()
            .filter(|date| *date <= today)
            .collect::<Vec<_>>(),
        (elapsed > 0.0).then_some(elapsed),
    )
}

fn estimate_with_censoring(
    starts: &[NaiveDate],
    censored_age: Option<f64>,
) -> Option<CyclePrediction> {
    let mut ordered = starts.to_vec();
    ordered.sort_unstable();
    ordered.dedup();

    let latest_start = *ordered.last()?;
    let observations: Vec<CycleObservation> = ordered
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]))
        .map(CycleObservation::completed_interval)
        .collect();
    let lengths: Vec<f64> = observations
        .iter()
        .filter(|observation| observation.kind == CycleObservationKind::Completed)
        .map(|observation| f64::from(observation.elapsed_days))
        .collect();

    if lengths.len() < 2 {
        return None;
    }

    let current_observation = censored_age
        .map(|age| CycleObservation::right_censored(age.round().max(0.0) as u32));
    let censor_for_model = current_observation
        .filter(|observation| observation.elapsed_days > 0)
        .map(|observation| f64::from(observation.elapsed_days));

    let changepoints = detect_cycle_regimes(&lengths);
    let hierarchical = infer_cycle_length(
        &lengths,
        &changepoints.regime_weights,
        PopulationPrior::default(),
    );
    let forecast = predict_next_cycle(
        &lengths,
        &changepoints.regime_weights,
        censor_for_model,
        hierarchical,
    )?;

    let expected_start = latest_start
        .checked_add((forecast.median_days as i64).days())
        .ok()?;
    let earliest_start = latest_start
        .checked_add((forecast.p10_days as i64).days())
        .ok()?;
    let latest_window_start = latest_start
        .checked_add((forecast.p90_days as i64).days())
        .ok()?;

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

/// Retained median/MAD baseline for rolling-origin model comparison.
pub fn estimate_next_period_baseline(starts: &[NaiveDate]) -> Option<CyclePrediction> {
    let mut ordered = starts.to_vec();
    ordered.sort_unstable();
    ordered.dedup();

    let lengths: Vec<f64> = ordered
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]))
        .filter(|days| (MIN_CYCLE_LENGTH..=MAX_CYCLE_LENGTH).contains(days))
        .map(f64::from)
        .collect();
    if lengths.len() < 2 {
        return None;
    }

    let center = median(&lengths)?;
    let deviations: Vec<f64> = lengths
        .iter()
        .map(|length| (length - center).abs())
        .collect();
    let radius = (median(&deviations)? * 1.4826).ceil().max(1.0) as u64;
    let expected_days = center.round().max(1.0) as u64;
    let latest_start = *ordered.last()?;

    Some(CyclePrediction {
        expected_start: latest_start
            .checked_add((expected_days as i64).days())
            .ok()?,
        earliest_start: latest_start
            .checked_add((expected_days.saturating_sub(radius) as i64).days())
            .ok()?,
        latest_start: latest_start
            .checked_add((expected_days.saturating_add(radius) as i64).days())
            .ok()?,
        cycle_intervals_used: lengths.len(),
        confidence: match lengths.len() {
            0..=2 => PredictionConfidence::Early,
            3..=5 => PredictionConfidence::Developing,
            _ => PredictionConfidence::Established,
        },
    })
}
