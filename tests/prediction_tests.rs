use chrono::NaiveDate;
use miora::engine::bayesian::CycleLengthPosterior;
use miora::engine::cycle_estimator::estimate_next_period;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
fn estimator_waits_for_two_completed_intervals() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29)];

    assert!(estimate_next_period(&starts).is_none());
}

#[test]
fn estimator_returns_a_window_from_logged_history() {
    let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];

    let prediction = estimate_next_period(&starts).unwrap();

    assert_eq!(prediction.expected_start, date(2026, 3, 26));
    assert_eq!(prediction.earliest_start, date(2026, 3, 25));
    assert_eq!(prediction.latest_start, date(2026, 3, 27));
    assert_eq!(prediction.cycle_intervals_used, 2);
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
