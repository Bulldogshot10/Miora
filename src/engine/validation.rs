//! Rolling-origin forecast evaluation for historical cycle-start data.
//!
//! This module reports empirical interval coverage, interval width, interval
//! score, and survival-model pinball loss. It does not claim population
//! calibration: the quality of these metrics depends on the representativeness
//! and consent status of the supplied history.

use jiff::{ToSpan, civil::Date as NaiveDate};

use crate::domain::prediction::CyclePrediction;
use crate::engine::cycle_estimator::{estimate_next_period_at, estimate_next_period_baseline};

const SURVIVAL_INTERVAL_LEVEL: f64 = 0.80;
const BASELINE_INTERVAL_LEVEL: f64 = 0.682_689_492;
const PINBALL_QUANTILES: [f64; 3] = [0.10, 0.50, 0.90];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastMetrics {
    pub predictions: usize,
    /// Empirical fraction of actual starts inside the reported interval.
    pub interval_coverage: f64,
    /// Nominal level associated with the interval construction.
    pub nominal_interval_level: f64,
    pub mean_interval_width_days: f64,
    pub mean_absolute_median_error_days: f64,
    /// Winkler interval score at the interval's nominal level.
    pub mean_interval_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollingOriginEvaluation {
    pub censor_age_days: u32,
    pub survival: ForecastMetrics,
    pub median_mad_baseline: ForecastMetrics,
    /// Mean pinball loss across the P10, P50, and P90 survival quantiles.
    pub survival_mean_pinball_loss: f64,
}

#[derive(Debug, Clone, Copy)]
struct ForecastSample {
    actual_day: f64,
    lower_day: f64,
    median_day: f64,
    upper_day: f64,
}

impl ForecastSample {
    fn from_prediction(
        prediction: CyclePrediction,
        cycle_start: NaiveDate,
        actual_start: NaiveDate,
    ) -> Option<Self> {
        let lower_day = (prediction.earliest_start - cycle_start).get_days() as f64;
        let median_day = (prediction.expected_start - cycle_start).get_days() as f64;
        let upper_day = (prediction.latest_start - cycle_start).get_days() as f64;
        let actual_day = (actual_start - cycle_start).get_days() as f64;

        if lower_day > median_day || median_day > upper_day {
            return None;
        }

        Some(Self {
            actual_day,
            lower_day,
            median_day,
            upper_day,
        })
    }
}

/// Evaluate forecasts at a fixed 14-day censoring age with at least two
/// completed intervals before each origin.
pub fn evaluate_rolling_origin(starts: &[NaiveDate]) -> Option<RollingOriginEvaluation> {
    evaluate_rolling_origin_with(starts, 2, 14)
}

/// Evaluate historical cycle-start dates without allowing future observations
/// into each forecast. minimum_history_intervals counts completed intervals;
/// censor_age_days is the simulated age of the active cycle at each origin.
///
/// The first usable origin has minimum_history_intervals + 1 start dates.
/// Origins whose next period had already started by the simulated forecast date
/// are skipped rather than scored with information from the future.
pub fn evaluate_rolling_origin_with(
    starts: &[NaiveDate],
    minimum_history_intervals: usize,
    censor_age_days: u32,
) -> Option<RollingOriginEvaluation> {
    if minimum_history_intervals < 2 || censor_age_days == 0 {
        return None;
    }

    let mut ordered = starts.to_vec();
    ordered.sort_unstable();
    ordered.dedup();
    let minimum_start_count = minimum_history_intervals.checked_add(2)?;
    if ordered.len() < minimum_start_count {
        return None;
    }

    let mut survival_samples = Vec::new();
    let mut baseline_samples = Vec::new();

    for (current_index, window) in ordered
        .windows(2)
        .enumerate()
        .skip(minimum_history_intervals)
    {
        let cycle_start = window[0];
        let actual_start = window[1];
        let forecast_date = cycle_start
            .checked_add((censor_age_days as i64).days())
            .ok()?;

        if actual_start <= forecast_date {
            continue;
        }

        // Only dates available at this historical origin enter either model.
        let history = &ordered[..=current_index];

        if let Some(prediction) = estimate_next_period_at(history, forecast_date) {
            if let Some(sample) =
                ForecastSample::from_prediction(prediction, cycle_start, actual_start)
            {
                survival_samples.push(sample);
            }
        }

        if let Some(prediction) = estimate_next_period_baseline(history) {
            if let Some(sample) =
                ForecastSample::from_prediction(prediction, cycle_start, actual_start)
            {
                baseline_samples.push(sample);
            }
        }
    }

    let survival = summarize(&survival_samples, SURVIVAL_INTERVAL_LEVEL)?;
    let median_mad_baseline = summarize(&baseline_samples, BASELINE_INTERVAL_LEVEL)?;
    let survival_mean_pinball_loss = mean_pinball_loss(&survival_samples)?;

    Some(RollingOriginEvaluation {
        censor_age_days,
        survival,
        median_mad_baseline,
        survival_mean_pinball_loss,
    })
}

fn summarize(samples: &[ForecastSample], nominal_level: f64) -> Option<ForecastMetrics> {
    if samples.is_empty() || !(0.0..1.0).contains(&nominal_level) {
        return None;
    }

    let alpha = 1.0 - nominal_level;
    let count = samples.len() as f64;
    let mut covered = 0usize;
    let mut total_width = 0.0;
    let mut total_median_error = 0.0;
    let mut total_interval_score = 0.0;

    for sample in samples {
        let width = sample.upper_day - sample.lower_day;
        if sample.actual_day >= sample.lower_day && sample.actual_day <= sample.upper_day {
            covered += 1;
        }

        let mut interval_score = width;
        if sample.actual_day < sample.lower_day {
            interval_score += (2.0 / alpha) * (sample.lower_day - sample.actual_day);
        } else if sample.actual_day > sample.upper_day {
            interval_score += (2.0 / alpha) * (sample.actual_day - sample.upper_day);
        }

        total_width += width;
        total_median_error += (sample.actual_day - sample.median_day).abs();
        total_interval_score += interval_score;
    }

    Some(ForecastMetrics {
        predictions: samples.len(),
        interval_coverage: covered as f64 / count,
        nominal_interval_level: nominal_level,
        mean_interval_width_days: total_width / count,
        mean_absolute_median_error_days: total_median_error / count,
        mean_interval_score: total_interval_score / count,
    })
}

fn mean_pinball_loss(samples: &[ForecastSample]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }

    let total = samples
        .iter()
        .map(|sample| {
            PINBALL_QUANTILES
                .iter()
                .copied()
                .zip([sample.lower_day, sample.median_day, sample.upper_day])
                .map(|(quantile, forecast)| {
                    let error = sample.actual_day - forecast;
                    if error >= 0.0 {
                        quantile * error
                    } else {
                        (quantile - 1.0) * error
                    }
                })
                .sum::<f64>()
        })
        .sum::<f64>();

    Some(total / (samples.len() as f64 * PINBALL_QUANTILES.len() as f64))
}
