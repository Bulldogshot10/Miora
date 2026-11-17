use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionConfidence {
    Early,
    Developing,
    Established,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclePrediction {
    pub expected_start: NaiveDate,
    pub earliest_start: NaiveDate,
    pub latest_start: NaiveDate,
    pub cycle_intervals_used: usize,
    pub confidence: PredictionConfidence,
}
