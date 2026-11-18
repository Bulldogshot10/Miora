use crate::engine::{MAX_CYCLE_LENGTH, MIN_CYCLE_LENGTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleObservationKind {
    Completed,
    RightCensored,
    PotentiallyMissedPeriodLog,
    OutsideSupportedRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleObservation {
    pub elapsed_days: u32,
    pub kind: CycleObservationKind,
}

impl CycleObservation {
    pub fn completed_interval(days: u32) -> Self {
        let kind = if (MIN_CYCLE_LENGTH..=MAX_CYCLE_LENGTH).contains(&days) {
            CycleObservationKind::Completed
        } else if days > MAX_CYCLE_LENGTH {
            CycleObservationKind::PotentiallyMissedPeriodLog
        } else {
            CycleObservationKind::OutsideSupportedRange
        };

        Self {
            elapsed_days: days,
            kind,
        }
    }

    pub fn right_censored(elapsed_days: u32) -> Self {
        Self {
            elapsed_days,
            kind: CycleObservationKind::RightCensored,
        }
    }
}
