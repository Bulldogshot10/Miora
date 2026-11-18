use crate::engine::math::normal_log_pdf;

const HAZARD: f64 = 0.08;
const PRIOR_MEAN_DAYS: f64 = 28.0;
const PRIOR_VARIANCE: f64 = 25.0;
const OBSERVATION_VARIANCE: f64 = 16.0;

#[derive(Debug, Clone, Copy)]
struct RunState {
    run_length: usize,
    probability: f64,
    count: f64,
    sum: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangepointResult {
    /// Probability that each observation belongs to the currently active regime.
    /// Entries are ordered from oldest to newest.
    pub regime_weights: Vec<f64>,
    /// Posterior probability for each possible current run length.
    pub run_length_probabilities: Vec<(usize, f64)>,
    pub current_changepoint_probability: f64,
}

/// Bayesian online changepoint detection with a Gaussian observation model,
/// known within-regime variance, and a constant per-cycle hazard.
pub fn detect_cycle_regimes(values: &[f64]) -> ChangepointResult {
    if values.is_empty() {
        return ChangepointResult {
            regime_weights: Vec::new(),
            run_length_probabilities: Vec::new(),
            current_changepoint_probability: 0.0,
        };
    }

    let mut states = vec![RunState {
        run_length: 1,
        probability: 1.0,
        count: 1.0,
        sum: values[0],
    }];

    for value in values.iter().copied().skip(1) {
        let prior_standard_deviation = (PRIOR_VARIANCE + OBSERVATION_VARIANCE).sqrt();
        let prior_log_probability =
            normal_log_pdf(value, PRIOR_MEAN_DAYS, prior_standard_deviation);

        let mut changepoint_mass = 0.0;
        let mut next_states = Vec::with_capacity(states.len() + 1);

        for state in &states {
            let posterior_precision = 1.0 / PRIOR_VARIANCE + state.count / OBSERVATION_VARIANCE;
            let posterior_variance = 1.0 / posterior_precision;
            let posterior_mean = (PRIOR_MEAN_DAYS / PRIOR_VARIANCE
                + state.sum / OBSERVATION_VARIANCE)
                / posterior_precision;
            let predictive_variance = OBSERVATION_VARIANCE + posterior_variance;
            let predictive_log_probability =
                normal_log_pdf(value, posterior_mean, predictive_variance.sqrt());
            let predictive_probability = predictive_log_probability.exp();

            changepoint_mass += state.probability * prior_log_probability.exp() * HAZARD;
            next_states.push(RunState {
                run_length: state.run_length + 1,
                probability: state.probability * predictive_probability * (1.0 - HAZARD),
                count: state.count + 1.0,
                sum: state.sum + value,
            });
        }

        next_states.push(RunState {
            run_length: 1,
            probability: changepoint_mass,
            count: 1.0,
            sum: value,
        });

        let total_probability: f64 = next_states.iter().map(|state| state.probability).sum();
        if total_probability <= f64::MIN_POSITIVE || !total_probability.is_finite() {
            states = vec![RunState {
                run_length: 1,
                probability: 1.0,
                count: 1.0,
                sum: value,
            }];
            continue;
        }

        for state in &mut next_states {
            state.probability /= total_probability;
        }
        next_states.retain(|state| state.probability > 1e-14);
        states = next_states;
    }

    let mut regime_weights = Vec::with_capacity(values.len());
    for lag in (1..=values.len()).rev() {
        let probability = states
            .iter()
            .filter(|state| state.run_length >= lag)
            .map(|state| state.probability)
            .sum::<f64>()
            .clamp(0.0, 1.0);
        regime_weights.push(probability);
    }

    let current_changepoint_probability = states
        .iter()
        .find(|state| state.run_length == 1)
        .map(|state| state.probability)
        .unwrap_or(0.0);

    let mut run_length_probabilities: Vec<(usize, f64)> = states
        .iter()
        .map(|state| (state.run_length, state.probability))
        .collect();
    run_length_probabilities.sort_by_key(|(run_length, _)| *run_length);

    ChangepointResult {
        regime_weights,
        run_length_probabilities,
        current_changepoint_probability,
    }
}
