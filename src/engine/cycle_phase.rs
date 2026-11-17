use chrono::NaiveDate;

use crate::domain::cycle::cycle_length_days;
use crate::engine::bayesian::CycleLengthPosterior;

const MIN_CYCLE_LENGTH: u32 = 21;
const MAX_CYCLE_LENGTH: u32 = 45;
const MIN_LUTEAL_LENGTH: u32 = 12;
const MAX_LUTEAL_LENGTH: u32 = 16;
const MIN_BLEEDING_DAYS: u32 = 2;
const MAX_BLEEDING_DAYS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclePhase {
    Menstrual,
    Follicular,
    OvulationWindow,
    Luteal,
    Learning,
}

impl CyclePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Menstrual => "menstrual",
            Self::Follicular => "follicular",
            Self::OvulationWindow => "ovulation_window",
            Self::Luteal => "luteal",
            Self::Learning => "learning",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Menstrual => "Menstrual phase",
            Self::Follicular => "Follicular phase",
            Self::OvulationWindow => "Estimated ovulation window",
            Self::Luteal => "Luteal phase",
            Self::Learning => "Still learning her cycle",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Menstrual => {
                "Bleeding observations and recorded period dates place this day in the menstrual phase."
            }
            Self::Follicular => {
                "The model estimates that this day is most likely in the follicular phase, before ovulation."
            }
            Self::OvulationWindow => {
                "The model estimates that this day is most likely near ovulation. Calendar history cannot confirm ovulation itself."
            }
            Self::Luteal => {
                "The model estimates that this day is most likely in the luteal phase, after ovulation."
            }
            Self::Learning => {
                "The phase probabilities are still spread across multiple possibilities. More confirmed period dates and bleeding observations will improve the estimate."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseProbabilities {
    pub menstrual: f64,
    pub follicular: f64,
    pub ovulation_window: f64,
    pub luteal: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CyclePhaseEstimate {
    pub phase: CyclePhase,
    pub cycle_day: u32,
    pub estimated_cycle_length: u32,
    pub completed_intervals_used: usize,
    pub completed_bleeding_durations_used: usize,
    pub confidence: f64,
    pub probabilities: PhaseProbabilities,
    pub is_personalized: bool,
}

#[derive(Debug, Clone)]
pub struct BleedingDurationPosterior {
    counts: Vec<u32>,
    prior_weights: Vec<f64>,
}

impl Default for BleedingDurationPosterior {
    fn default() -> Self {
        // A weak population prior centered around five days. Each confirmed
        // personal duration adds an observation and progressively dominates it.
        Self {
            counts: vec![0; (MAX_BLEEDING_DAYS - MIN_BLEEDING_DAYS + 1) as usize],
            prior_weights: vec![0.25, 0.75, 1.5, 2.0, 1.8, 1.2, 0.6, 0.3, 0.1],
        }
    }
}

impl BleedingDurationPosterior {
    pub fn observe(&mut self, duration_days: u32) -> bool {
        if !(MIN_BLEEDING_DAYS..=MAX_BLEEDING_DAYS).contains(&duration_days) {
            return false;
        }

        self.counts[(duration_days - MIN_BLEEDING_DAYS) as usize] += 1;
        true
    }

    pub fn probability(&self, duration_days: u32) -> Option<f64> {
        if !(MIN_BLEEDING_DAYS..=MAX_BLEEDING_DAYS).contains(&duration_days) {
            return None;
        }

        let total_observed: u32 = self.counts.iter().sum();
        let total_prior: f64 = self.prior_weights.iter().sum();
        let denominator = total_observed as f64 + total_prior;
        let index = (duration_days - MIN_BLEEDING_DAYS) as usize;

        Some((self.counts[index] as f64 + self.prior_weights[index]) / denominator)
    }

    pub fn distribution(&self) -> Vec<(u32, f64)> {
        (MIN_BLEEDING_DAYS..=MAX_BLEEDING_DAYS)
            .filter_map(|day| self.probability(day).map(|probability| (day, probability)))
            .collect()
    }

    pub fn expected_duration(&self) -> f64 {
        self.distribution()
            .iter()
            .map(|(days, probability)| *days as f64 * probability)
            .sum()
    }

    pub fn median_duration(&self) -> u32 {
        let mut cumulative = 0.0;
        for (days, probability) in self.distribution() {
            cumulative += probability;
            if cumulative >= 0.5 {
                return days;
            }
        }

        MAX_BLEEDING_DAYS
    }

    pub fn observations(&self) -> usize {
        self.counts.iter().map(|count| *count as usize).sum()
    }
}

pub fn estimate_bleeding_duration(completed_durations: &[u32]) -> BleedingDurationPosterior {
    let mut posterior = BleedingDurationPosterior::default();
    for duration in completed_durations {
        posterior.observe(*duration);
    }
    posterior
}

/// Bayesian phase classifier.
///
/// Cycle length is inferred from the user's completed period-start intervals.
/// Menstrual duration is inferred from confirmed bleeding end dates. Ovulation
/// is represented as a distribution over luteal lengths (12–16 days before
/// the next period), rather than one hard-coded cycle day.
pub fn estimate_cycle_phase(
    period_starts: &[NaiveDate],
    completed_bleeding_durations: &[u32],
    date: NaiveDate,
    confirmed_current_bleeding_days: Option<u32>,
    observed_bleeding_through_day: Option<u32>,
) -> Option<CyclePhaseEstimate> {
    let mut starts = period_starts.to_vec();
    starts.sort_unstable();
    starts.dedup();

    let current_start_index = starts.iter().rposition(|start| *start <= date)?;
    let current_start = starts[current_start_index];
    let cycle_day = (date - current_start).num_days() as u32 + 1;

    if let Some(observed_day) = observed_bleeding_through_day
        && cycle_day <= observed_day
    {
        return Some(fixed_menstrual_estimate(
            cycle_day,
            confirmed_current_bleeding_days.is_none(),
            starts.len().saturating_sub(1),
            completed_bleeding_durations.len(),
            true,
        ));
    }

    let observed_cycle_lengths: Vec<u32> = starts
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]))
        .filter(|days| (MIN_CYCLE_LENGTH..=MAX_CYCLE_LENGTH).contains(days))
        .collect();

    let mut cycle_posterior =
        CycleLengthPosterior::with_prior(MIN_CYCLE_LENGTH, MAX_CYCLE_LENGTH, 0.02);
    for length in &observed_cycle_lengths {
        cycle_posterior.observe(*length);
    }

    let bleeding_posterior = estimate_bleeding_duration(completed_bleeding_durations);
    let bleeding_distribution = if let Some(days) = confirmed_current_bleeding_days {
        vec![(days.clamp(1, MAX_BLEEDING_DAYS), 1.0)]
    } else {
        bleeding_posterior.distribution()
    };

    let cycle_distribution = cycle_posterior.distribution();
    let luteal_candidate_count = (MAX_LUTEAL_LENGTH - MIN_LUTEAL_LENGTH + 1) as f64;
    let luteal_weight = 1.0 / luteal_candidate_count;

    let mut probabilities = PhaseProbabilities {
        menstrual: 0.0,
        follicular: 0.0,
        ovulation_window: 0.0,
        luteal: 0.0,
    };

    for (cycle_length, cycle_weight) in &cycle_distribution {
        for (bleeding_days, bleeding_weight) in &bleeding_distribution {
            for luteal_days in MIN_LUTEAL_LENGTH..=MAX_LUTEAL_LENGTH {
                let weight = cycle_weight * bleeding_weight * luteal_weight;
                let ovulation_day = cycle_length.saturating_sub(luteal_days).max(1);
                let phase = if cycle_day <= *bleeding_days {
                    CyclePhase::Menstrual
                } else if cycle_day + 1 < ovulation_day {
                    CyclePhase::Follicular
                } else if cycle_day <= ovulation_day + 1 {
                    CyclePhase::OvulationWindow
                } else {
                    CyclePhase::Luteal
                };

                match phase {
                    CyclePhase::Menstrual => probabilities.menstrual += weight,
                    CyclePhase::Follicular => probabilities.follicular += weight,
                    CyclePhase::OvulationWindow => probabilities.ovulation_window += weight,
                    CyclePhase::Luteal => probabilities.luteal += weight,
                    CyclePhase::Learning => {}
                }
            }
        }
    }

    normalize_probabilities(&mut probabilities);

    let candidates = [
        (CyclePhase::Menstrual, probabilities.menstrual),
        (CyclePhase::Follicular, probabilities.follicular),
        (CyclePhase::OvulationWindow, probabilities.ovulation_window),
        (CyclePhase::Luteal, probabilities.luteal),
    ];
    let (most_likely_phase, confidence) = candidates
        .into_iter()
        .max_by(|left, right| left.1.total_cmp(&right.1))?;

    let phase = if confidence < 0.55 {
        CyclePhase::Learning
    } else {
        most_likely_phase
    };

    let expected_cycle_length = if observed_cycle_lengths.is_empty() {
        0
    } else {
        cycle_posterior.expected_length().round() as u32
    };
    let is_personalized =
        !observed_cycle_lengths.is_empty() && !completed_bleeding_durations.is_empty();

    Some(CyclePhaseEstimate {
        phase,
        cycle_day,
        estimated_cycle_length: expected_cycle_length,
        completed_intervals_used: observed_cycle_lengths.len(),
        completed_bleeding_durations_used: completed_bleeding_durations.len(),
        confidence,
        probabilities,
        is_personalized,
    })
}

fn fixed_menstrual_estimate(
    cycle_day: u32,
    bleeding_duration_is_estimated: bool,
    completed_intervals_used: usize,
    completed_bleeding_durations_used: usize,
    is_personalized: bool,
) -> CyclePhaseEstimate {
    CyclePhaseEstimate {
        phase: CyclePhase::Menstrual,
        cycle_day,
        estimated_cycle_length: 0,
        completed_intervals_used,
        completed_bleeding_durations_used,
        confidence: 1.0,
        probabilities: PhaseProbabilities {
            menstrual: 1.0,
            follicular: 0.0,
            ovulation_window: 0.0,
            luteal: 0.0,
        },
        is_personalized: is_personalized && !bleeding_duration_is_estimated,
    }
}

fn normalize_probabilities(probabilities: &mut PhaseProbabilities) {
    let total = probabilities.menstrual
        + probabilities.follicular
        + probabilities.ovulation_window
        + probabilities.luteal;

    if total <= f64::EPSILON {
        return;
    }

    probabilities.menstrual /= total;
    probabilities.follicular /= total;
    probabilities.ovulation_window /= total;
    probabilities.luteal /= total;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn bleeding_duration_posterior_learns_from_confirmed_end_dates() {
        let mut posterior = BleedingDurationPosterior::default();
        let initial_prediction = posterior.median_duration();

        for _ in 0..5 {
            assert!(posterior.observe(4));
        }

        assert_eq!(initial_prediction, 5);
        assert_eq!(posterior.median_duration(), 4);
        assert_eq!(posterior.observations(), 5);
    }

    #[test]
    fn active_bleeding_observation_overrides_phase_distribution() {
        let start = date(2026, 11, 8);
        let estimate =
            estimate_cycle_phase(&[start], &[], date(2026, 11, 13), None, Some(6)).unwrap();

        assert_eq!(estimate.phase, CyclePhase::Menstrual);
        assert_eq!(estimate.confidence, 1.0);
    }

    #[test]
    fn phase_model_keeps_ovulation_uncertain_with_only_two_intervals() {
        let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];
        let follicular =
            estimate_cycle_phase(&starts, &[5, 5], date(2026, 3, 5), Some(5), None).unwrap();
        let ovulation =
            estimate_cycle_phase(&starts, &[5, 5], date(2026, 3, 10), Some(5), None).unwrap();
        let luteal =
            estimate_cycle_phase(&starts, &[5, 5], date(2026, 3, 18), Some(5), None).unwrap();

        assert_eq!(follicular.phase, CyclePhase::Follicular);
        assert_eq!(ovulation.phase, CyclePhase::Learning);
        assert!(ovulation.probabilities.ovulation_window > 0.45);
        assert_eq!(luteal.phase, CyclePhase::Luteal);
        assert!(ovulation.is_personalized);
    }

    #[test]
    fn phase_probability_is_normalized() {
        let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];
        let estimate =
            estimate_cycle_phase(&starts, &[4, 6], date(2026, 3, 10), None, None).unwrap();
        let probabilities = estimate.probabilities;
        let sum = probabilities.menstrual
            + probabilities.follicular
            + probabilities.ovulation_window
            + probabilities.luteal;

        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn date_before_first_period_has_no_phase() {
        assert!(
            estimate_cycle_phase(&[date(2026, 11, 8)], &[], date(2026, 11, 7), None, None)
                .is_none()
        );
    }
}
