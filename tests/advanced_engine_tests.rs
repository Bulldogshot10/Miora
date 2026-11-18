use miora::engine::changepoint::detect_cycle_regimes;
use miora::engine::hierarchical::{PopulationPrior, infer_cycle_length};
use miora::engine::observation::{CycleObservation, CycleObservationKind};
use miora::engine::survival::gamma::Gamma;
use miora::engine::survival::predict_next_cycle;
use miora::engine::survival::weibull::Weibull;

#[test]
fn observation_quality_marks_long_gaps_as_potentially_missed_logs() {
    assert_eq!(
        CycleObservation::completed_interval(28).kind,
        CycleObservationKind::Completed
    );
    assert_eq!(
        CycleObservation::completed_interval(75).kind,
        CycleObservationKind::PotentiallyMissedPeriodLog
    );
    assert_eq!(
        CycleObservation::right_censored(34).kind,
        CycleObservationKind::RightCensored
    );
}

#[test]
fn changepoint_detector_downweights_observations_from_an_old_regime() {
    let observations = [28.0, 29.0, 28.0, 29.0, 45.0, 46.0, 45.0, 44.0];
    let result = detect_cycle_regimes(&observations);

    assert_eq!(result.regime_weights.len(), observations.len());
    assert!(result.regime_weights.last().copied().unwrap() > 0.99);
    assert!(result.regime_weights[0] < result.regime_weights[7]);
    assert!(result.current_changepoint_probability.is_finite());
}

#[test]
fn hierarchical_cycle_posterior_shrinks_sparse_personal_history() {
    let lengths = [36.0, 37.0];
    let weights = [1.0, 1.0];
    let posterior = infer_cycle_length(&lengths, &weights, PopulationPrior::default());

    assert!(posterior.mean_cycle_days > 28.0);
    assert!(posterior.mean_cycle_days < 36.5);
    assert!(posterior.predictive_sd > posterior.posterior_sd);
}

#[test]
fn weibull_and_gamma_distributions_have_valid_cdfs() {
    let weibull = Weibull {
        shape: 1.0,
        scale: 10.0,
    };
    let gamma = Gamma {
        shape: 1.0,
        scale: 10.0,
    };

    assert!((weibull.cdf(10.0) - (1.0 - (-1.0_f64).exp())).abs() < 1e-10);
    assert!((gamma.cdf(10.0) - (1.0 - (-1.0_f64).exp())).abs() < 1e-8);
}

#[test]
fn survival_forecast_conditions_on_the_current_censored_cycle() {
    let lengths = [27.0, 28.0, 29.0, 28.0, 30.0, 29.0];
    let weights = [1.0; 6];
    let posterior = infer_cycle_length(&lengths, &weights, PopulationPrior::default());

    let uncensored = predict_next_cycle(&lengths, &weights, None, posterior).unwrap();
    let censored = predict_next_cycle(&lengths, &weights, Some(35.0), posterior).unwrap();

    assert!(uncensored.p10_days <= uncensored.median_days);
    assert!(uncensored.median_days <= uncensored.p90_days);
    assert!(censored.p10_days > 35);
    assert!(censored.median_days > uncensored.median_days);
    assert!(censored.p90_days >= censored.median_days);
}

#[test]
fn changepoint_run_length_posterior_is_normalized_and_regime_weights_are_monotone() {
    let observations = [28.0, 29.0, 27.0, 28.0, 29.0, 28.0, 44.0, 46.0, 45.0];
    let result = detect_cycle_regimes(&observations);

    let posterior_sum: f64 = result
        .run_length_probabilities
        .iter()
        .map(|(_, probability)| probability)
        .sum();

    assert!((posterior_sum - 1.0).abs() < 1e-10);
    assert!(
        result
            .run_length_probabilities
            .iter()
            .all(|(_, probability)| probability.is_finite() && (0.0..=1.0).contains(probability))
    );
    assert!(
        result
            .regime_weights
            .windows(2)
            .all(|pair| pair[0] <= pair[1] + 1e-12)
    );
    assert!((result.regime_weights.last().copied().unwrap_or_default() - 1.0).abs() < 1e-12);
}

#[test]
fn gamma_distribution_matches_non_exponential_reference_values() {
    let gamma_shape_two = Gamma {
        shape: 2.0,
        scale: 3.0,
    };
    let gamma_half_shape = Gamma {
        shape: 0.5,
        scale: 2.0,
    };

    let expected_shape_two = 1.0 - 3.0 * (-2.0_f64).exp();
    let expected_half_shape = 0.682_689_492_137_085_9;

    assert!((gamma_shape_two.cdf(6.0) - expected_shape_two).abs() < 1e-9);
    assert!((gamma_half_shape.cdf(1.0) - expected_half_shape).abs() < 1e-8);
}

#[test]
fn weibull_distribution_matches_shape_two_reference_values() {
    let weibull = Weibull {
        shape: 2.0,
        scale: 10.0,
    };

    assert!((weibull.cdf(10.0) - (1.0 - (-1.0_f64).exp())).abs() < 1e-12);
    assert!((weibull.log_pdf(10.0) - (0.2_f64.ln() - 1.0)).abs() < 1e-12);
}

#[test]
fn survival_candidate_weights_are_finite_and_normalized() {
    let lengths = [27.0, 28.0, 29.0, 28.0, 30.0, 29.0, 28.0, 27.0];
    let weights = [1.0; 8];
    let posterior = infer_cycle_length(&lengths, &weights, PopulationPrior::default());
    let forecast = predict_next_cycle(&lengths, &weights, None, posterior).unwrap();

    assert!(forecast.weibull_weight.is_finite());
    assert!(forecast.gamma_weight.is_finite());
    assert!((forecast.weibull_weight + forecast.gamma_weight - 1.0).abs() < 1e-12);
    assert!((0.0..=1.0).contains(&forecast.hierarchical_weight));
    assert!(forecast.p10_days <= forecast.median_days);
    assert!(forecast.median_days <= forecast.p90_days);
}
