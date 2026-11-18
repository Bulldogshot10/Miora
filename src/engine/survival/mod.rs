pub mod gamma;
pub mod weibull;

use self::gamma::Gamma;
use self::weibull::Weibull;

use crate::engine::hierarchical::HierarchicalPosterior;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurvivalForecast {
    pub p10_days: u32,
    pub median_days: u32,
    pub p90_days: u32,
    pub weibull_weight: f64,
    pub gamma_weight: f64,
    pub hierarchical_weight: f64,
}

/// Fit Weibull and Gamma event-time models with a right-censored current cycle.
/// Candidate-family weights are normalized relative maximum log-likelihood
/// scores (or 50/50 for sparse histories); they are not integrated model evidence.
/// The final predictive CDF is blended with the hierarchical predictive distribution.
pub fn predict_next_cycle(
    completed_lengths: &[f64],
    regime_weights: &[f64],
    censored_age: Option<f64>,
    hierarchical: HierarchicalPosterior,
) -> Option<SurvivalForecast> {
    if completed_lengths.len() < 2 {
        return None;
    }

    let events: Vec<(f64, f64)> = completed_lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, length)| {
            let weight = regime_weights
                .get(index)
                .copied()
                .unwrap_or(1.0)
                .clamp(0.02, 1.0);
            (length.is_finite() && length > 0.0).then_some((length, weight))
        })
        .collect();
    let effective_events: f64 = events.iter().map(|(_, weight)| *weight).sum();
    if events.len() < 2 || effective_events <= 0.0 {
        return None;
    }

    let censor = censored_age.filter(|age| age.is_finite() && *age > 0.0);
    let weibull_fit = fit_weibull(&events, censor)?;
    let gamma_fit = fit_gamma(&events, censor)?;

    let total_information = effective_events + if censor.is_some() { 1.0 } else { 0.0 };
    let (weibull_weight, gamma_weight) = if total_information < 5.0 {
        (0.5, 0.5)
    } else {
        let maximum = weibull_fit.1.max(gamma_fit.1);
        let weibull_mass = (weibull_fit.1 - maximum).exp();
        let gamma_mass = (gamma_fit.1 - maximum).exp();
        let denominator = (weibull_mass + gamma_mass).max(f64::MIN_POSITIVE);
        (weibull_mass / denominator, gamma_mass / denominator)
    };
    let hierarchical_weight = (effective_events
        / (effective_events + hierarchical.prior_strength.max(0.0)))
    .clamp(0.0, 1.0);
    let censor_day = censor.unwrap_or(0.0);

    let mixture_cdf = |day: f64| {
        let personal_cdf =
            weibull_weight * weibull_fit.0.cdf(day) + gamma_weight * gamma_fit.0.cdf(day);
        let prior_cdf = hierarchical.predictive_cdf(day);
        hierarchical_weight * personal_cdf + (1.0 - hierarchical_weight) * prior_cdf
    };
    let cdf_at_censor = mixture_cdf(censor_day);
    let survival_at_censor = (1.0 - cdf_at_censor).max(1e-12);
    let conditional_cdf = |day: f64| {
        if day <= censor_day {
            return 0.0;
        }
        ((mixture_cdf(day) - cdf_at_censor) / survival_at_censor).clamp(0.0, 1.0)
    };

    Some(SurvivalForecast {
        p10_days: quantile_day(&conditional_cdf, censor_day, 0.10)?,
        median_days: quantile_day(&conditional_cdf, censor_day, 0.50)?,
        p90_days: quantile_day(&conditional_cdf, censor_day, 0.90)?,
        weibull_weight,
        gamma_weight,
        hierarchical_weight,
    })
}

fn quantile_day(cdf: &impl Fn(f64) -> f64, censor_day: f64, probability: f64) -> Option<u32> {
    let first_day = censor_day.floor().max(0.0) as u32 + 1;
    (first_day..=240).find(|day| cdf(f64::from(*day)) >= probability)
}

fn fit_weibull(events: &[(f64, f64)], censor: Option<f64>) -> Option<(Weibull, f64)> {
    const GRID_SIZE: usize = 48;
    let mut best: Option<(Weibull, f64)> = None;

    for shape_index in 0..GRID_SIZE {
        let fraction = shape_index as f64 / (GRID_SIZE - 1) as f64;
        let shape = 0.3 * (20.0_f64 / 0.3).powf(fraction);

        for scale_index in 0..GRID_SIZE {
            let scale = 5.0 + 95.0 * scale_index as f64 / (GRID_SIZE - 1) as f64;
            let model = Weibull { shape, scale };
            let mut likelihood = 0.0;

            for (length, weight) in events {
                likelihood += weight * model.log_pdf(*length);
            }
            if let Some(age) = censor {
                likelihood += model.log_survival(age);
            }

            if likelihood.is_finite() && best.as_ref().is_none_or(|(_, score)| likelihood > *score)
            {
                best = Some((model, likelihood));
            }
        }
    }

    best
}

fn fit_gamma(events: &[(f64, f64)], censor: Option<f64>) -> Option<(Gamma, f64)> {
    const GRID_SIZE: usize = 48;
    let mut best: Option<(Gamma, f64)> = None;

    for shape_index in 0..GRID_SIZE {
        let fraction = shape_index as f64 / (GRID_SIZE - 1) as f64;
        let shape = 0.3 * (20.0_f64 / 0.3).powf(fraction);

        for scale_index in 0..GRID_SIZE {
            let fraction = scale_index as f64 / (GRID_SIZE - 1) as f64;
            let scale = 0.2 * (120.0_f64 / 0.2).powf(fraction);
            let model = Gamma { shape, scale };
            let mut likelihood = 0.0;

            for (length, weight) in events {
                likelihood += weight * model.log_pdf(*length);
            }
            if let Some(age) = censor {
                likelihood += model.log_survival(age);
            }

            if likelihood.is_finite() && best.as_ref().is_none_or(|(_, score)| likelihood > *score)
            {
                best = Some((model, likelihood));
            }
        }
    }

    best
}
