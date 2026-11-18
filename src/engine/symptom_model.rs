use crate::engine::hierarchical::hierarchical_binary_probability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymptomObservation {
    pub cycle_day: u32,
    pub occurred: bool,
}

/// A cycle-day estimate partially pooled toward the user's overall symptom rate.
pub fn probability_on_cycle_day(
    observations: &[SymptomObservation],
    target_cycle_day: u32,
) -> Option<f64> {
    let matching: Vec<bool> = observations
        .iter()
        .filter(|entry| entry.cycle_day == target_cycle_day)
        .map(|entry| entry.occurred)
        .collect();
    if matching.is_empty() {
        return None;
    }

    let other: Vec<bool> = observations
        .iter()
        .filter(|entry| entry.cycle_day != target_cycle_day)
        .map(|entry| entry.occurred)
        .collect();

    let matching_successes = matching.iter().filter(|occurred| **occurred).count();
    let other_successes = other.iter().filter(|occurred| **occurred).count();

    hierarchical_binary_probability(
        matching_successes,
        matching.len(),
        other_successes,
        other.len(),
        4.0,
    )
}
