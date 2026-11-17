/// A discrete Dirichlet-categorical posterior over plausible cycle lengths.
///
/// Each integer cycle length starts with one pseudo-observation. Logged cycle
/// intervals update the posterior without requiring a neural model.
#[derive(Debug, Clone)]
pub struct CycleLengthPosterior {
    min_days: u32,
    max_days: u32,
    counts: Vec<u32>,
    prior_per_day: f64,
}

impl Default for CycleLengthPosterior {
    fn default() -> Self {
        Self::new(21, 45)
    }
}

impl CycleLengthPosterior {
    pub fn new(min_days: u32, max_days: u32) -> Self {
        assert!(min_days <= max_days, "minimum must not exceed maximum");
        let count = (max_days - min_days + 1) as usize;

        Self {
            min_days,
            max_days,
            counts: vec![0; count],
            prior_per_day: 1.0,
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

    pub fn expected_length(&self) -> f64 {
        let total_observations: u32 = self.counts.iter().sum();
        let denominator = total_observations as f64 + self.prior_per_day * self.counts.len() as f64;

        (self.min_days..=self.max_days)
            .zip(self.counts.iter())
            .map(|(day, count)| day as f64 * (*count as f64 + self.prior_per_day) / denominator)
            .sum()
    }

    pub fn observations(&self) -> u32 {
        self.counts.iter().sum()
    }
}
