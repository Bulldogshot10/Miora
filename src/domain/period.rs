use jiff::civil::Date as NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowIntensity {
    Light,
    Medium,
    Heavy,
}

impl FlowIntensity {
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Light => 1,
            Self::Medium => 2,
            Self::Heavy => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub id: i64,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub flow_intensity: Option<FlowIntensity>,
}
