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
fn bayesian_posterior_updates_after_observations() {
    let mut posterior = CycleLengthPosterior::default();
    let prior_probability = posterior.probability(28).unwrap();

    assert!(posterior.observe(28));
    assert!(posterior.observe(28));
    assert!(posterior.probability(28).unwrap() > prior_probability);
    assert_eq!(posterior.observations(), 2);
}
