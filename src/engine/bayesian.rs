/// A discrete Dirichlet-categorical posterior over plausible cycle lengths.
///
/// The model starts with a weak uniform prior and updates with every
/// completed period-to-period interval.
#[derive(Debug, Clone)]
pub struct CycleLengthPosterior {
    min_days: u32,
    max_days: u32,
    counts: Vec<u32>,
    prior_per_day: f64,
}

impl Default for CycleLengthPosterior {
    fn default() -> Self {
        Self::new(15, 60)
    }
}

impl CycleLengthPosterior {
    pub fn new(min_days: u32, max_days: u32) -> Self {
        Self::with_prior(min_days, max_days, 0.02)
    }

    pub fn with_prior(min_days: u32, max_days: u32, prior_per_day: f64) -> Self {
        assert!(min_days <= max_days, "minimum must not exceed maximum");
        assert!(prior_per_day > 0.0, "prior weight must be positive");
        let count = (max_days - min_days + 1) as usize;

        Self {
            min_days,
            max_days,
            counts: vec![0; count],
            prior_per_day,
        }
    }

    pub fn observe(&mut self, cycle_length_days: u32) -> bool {
        if !(self.min_days..=self.max_days).contains(&cycle_length_days) {
            return false;
        }

        self.counts[(cycle_length_days - self.min_days) as usize] += 1;
        true
    }

    pub fn probability(&self, cycle_length_days: u32) -> Option<f64> {
        if !(self.min_days..=self.max_days).contains(&cycle_length_days) {
            return None;
        }

        let total_observations: u32 = self.counts.iter().sum();
        let denominator = total_observations as f64 + self.prior_per_day * self.counts.len() as f64;
        let observed = self.counts[(cycle_length_days - self.min_days) as usize] as f64;

        Some((observed + self.prior_per_day) / denominator)
    }

    pub fn distribution(&self) -> Vec<(u32, f64)> {
        (self.min_days..=self.max_days)
            .filter_map(|day| self.probability(day).map(|probability| (day, probability)))
            .collect()
    }

    pub fn expected_length(&self) -> f64 {
        self.distribution()
            .iter()
            .map(|(day, probability)| *day as f64 * probability)
            .sum()
    }

    pub fn most_likely_length(&self) -> u32 {
        self.distribution()
            .into_iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(day, _)| day)
            .unwrap_or(self.min_days)
    }

    pub fn observations(&self) -> u32 {
        self.counts.iter().sum()
    }
}
