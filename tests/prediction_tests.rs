use jiff::civil::Date as NaiveDate;
use miora::engine::bayesian::CycleLengthPosterior;
use miora::engine::cycle_estimator::{
    estimate_next_period, estimate_next_period_at, estimate_next_period_baseline,
};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::new(year as i16, month as i8, day as i8).unwrap()
}

#[test]
fn estimator_waits_for_two_completed_intervals() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29)];

    assert!(estimate_next_period(&starts).is_none());
}

#[test]
fn survival_estimator_returns_ordered_predictive_quantiles() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];

    let prediction = estimate_next_period(&starts).unwrap();

    assert_eq!(prediction.expected_start, date(2026, 3, 26));
    assert!(prediction.earliest_start <= prediction.expected_start);
    assert!(prediction.expected_start <= prediction.latest_start);
    assert_eq!(prediction.cycle_intervals_used, 2);
}

#[test]
fn current_cycle_is_used_as_a_right_censored_observation() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];
    let today = date(2026, 4, 4);

    let prediction = estimate_next_period_at(&starts, today).unwrap();

    assert!(prediction.expected_start > today);
    assert!(prediction.earliest_start > today);
}

#[test]
fn median_mad_baseline_remains_available_for_comparison() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];

    let baseline = estimate_next_period_baseline(&starts).unwrap();

    assert_eq!(baseline.expected_start, date(2026, 3, 26));
}

#[test]
fn rolling_origin_evaluation_compares_survival_and_median_mad() {
    use jiff::ToSpan;
    use miora::engine::validation::evaluate_rolling_origin;

    let intervals = [28_i64, 29, 30, 27, 31, 28, 30, 29, 27, 31];
    let mut starts = vec![date(2024, 1, 1)];
    for interval in intervals {
        let next = starts.last().unwrap().checked_add(interval.days()).unwrap();
        starts.push(next);
    }

    let evaluation = evaluate_rolling_origin(&starts).unwrap();
    assert_eq!(evaluation.censor_age_days, 14);
    assert_eq!(evaluation.survival.predictions, intervals.len() - 2);
    assert_eq!(
        evaluation.median_mad_baseline.predictions,
        evaluation.survival.predictions
    );
    assert_eq!(evaluation.survival.nominal_interval_level, 0.80);
    assert!(evaluation.survival.interval_coverage.is_finite());
    assert!((0.0..=1.0).contains(&evaluation.survival.interval_coverage));
    assert!(evaluation.survival.mean_interval_width_days >= 0.0);
    assert!(evaluation.survival.mean_absolute_median_error_days >= 0.0);
    assert!(evaluation.survival.mean_interval_score >= 0.0);
    assert!(evaluation.survival_mean_pinball_loss.is_finite());
    assert!(evaluation.median_mad_baseline.interval_coverage.is_finite());
}

#[test]
fn rolling_origin_evaluation_rejects_insufficient_or_invalid_settings() {
    use miora::engine::validation::evaluate_rolling_origin_with;

    let starts = [date(2024, 1, 1), date(2024, 1, 29), date(2024, 2, 26)];
    assert!(evaluate_rolling_origin_with(&starts, 2, 14).is_none());
    assert!(evaluate_rolling_origin_with(&starts, 1, 14).is_none());
    assert!(evaluate_rolling_origin_with(&starts, 2, 0).is_none());
}

#[test]
fn bayesian_posterior_updates_after_observations() {
    let mut posterior = CycleLengthPosterior::default();
    let prior_probability = posterior.probability(28).unwrap();

    assert!(posterior.observe(28));
    assert!(posterior.observe(28));
    assert!(posterior.probability(28).unwrap() > prior_probability);
    assert_eq!(posterior.observations(), 2);
}
