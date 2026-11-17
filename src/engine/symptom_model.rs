#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymptomObservation {
    pub cycle_day: u32,
    pub occurred: bool,
}

/// Laplace-smoothed probability for one symptom on one relative cycle day.
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

    let positive = matching.iter().filter(|occurred| **occurred).count() as f64;
    Some((positive + 1.0) / (matching.len() as f64 + 2.0))
}
