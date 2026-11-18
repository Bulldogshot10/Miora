use crate::engine::math::normal_cdf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopulationPrior {
    pub mean_cycle_days: f64,
    pub between_person_sd: f64,
    pub within_person_sd: f64,
    pub prior_strength: f64,
}

impl Default for PopulationPrior {
    fn default() -> Self {
        Self {
            mean_cycle_days: 28.0,
            between_person_sd: 5.0,
            within_person_sd: 4.0,
            prior_strength: 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HierarchicalPosterior {
    pub mean_cycle_days: f64,
    pub posterior_sd: f64,
    pub predictive_sd: f64,
    pub effective_observations: f64,
    pub population_mean_days: f64,
    pub prior_strength: f64,
}

impl HierarchicalPosterior {
    pub fn predictive_cdf(&self, value: f64) -> f64 {
        normal_cdf(value, self.mean_cycle_days, self.predictive_sd)
    }
}

/// Normal-normal partial pooling: a weak population-level prior is updated
/// with regime-weighted personal observations.
pub fn infer_cycle_length(
    lengths: &[f64],
    regime_weights: &[f64],
    prior: PopulationPrior,
) -> HierarchicalPosterior {
    let between_variance = prior.between_person_sd.powi(2).max(1e-6);
    let within_variance = prior.within_person_sd.powi(2).max(1e-6);
    let prior_precision = 1.0 / between_variance;
    let mut precision = prior_precision;
    let mut weighted_sum = prior.mean_cycle_days * prior_precision;
    let mut effective_observations = 0.0;

    for (index, length) in lengths.iter().copied().enumerate() {
        let weight = regime_weights
            .get(index)
            .copied()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        if !length.is_finite() || weight <= 0.0 {
            continue;
        }

        let observation_precision = weight / within_variance;
        precision += observation_precision;
        weighted_sum += length * observation_precision;
        effective_observations += weight;
    }

    let posterior_variance = 1.0 / precision;
    let posterior_mean = weighted_sum / precision;

    HierarchicalPosterior {
        mean_cycle_days: posterior_mean,
        posterior_sd: posterior_variance.sqrt(),
        predictive_sd: (within_variance + posterior_variance).sqrt(),
        effective_observations,
        population_mean_days: prior.mean_cycle_days,
        prior_strength: prior.prior_strength,
    }
}

/// Beta-binomial partial pooling for sparse symptom observations. The prior
/// rate is estimated from observations on other cycle days.
pub fn hierarchical_binary_probability(
    matching_successes: usize,
    matching_trials: usize,
    other_successes: usize,
    other_trials: usize,
    prior_strength: f64,
) -> Option<f64> {
    if matching_trials == 0 || matching_successes > matching_trials {
        return None;
    }

    let other_rate = (other_successes as f64 + 1.0) / (other_trials as f64 + 2.0);
    let strength = prior_strength.max(0.0);
    let alpha = 1.0 + other_rate * strength;
    let beta = 1.0 + (1.0 - other_rate) * strength;

    Some((matching_successes as f64 + alpha) / (matching_trials as f64 + alpha + beta))
}
