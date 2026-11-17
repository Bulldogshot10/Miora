#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymptomCode {
    Cramps,
    Bloating,
    Headache,
    Fatigue,
    MoodChanges,
}

impl SymptomCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cramps => "cramps",
            Self::Bloating => "bloating",
            Self::Headache => "headache",
            Self::Fatigue => "fatigue",
            Self::MoodChanges => "mood_changes",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cramps => "Cramps",
            Self::Bloating => "Bloating",
            Self::Headache => "Headache",
            Self::Fatigue => "Fatigue",
            Self::MoodChanges => "Mood changes",
        }
    }

    pub fn parse_key(value: &str) -> Option<Self> {
        match value {
            "cramps" => Some(Self::Cramps),
            "bloating" => Some(Self::Bloating),
            "headache" => Some(Self::Headache),
            "fatigue" => Some(Self::Fatigue),
            "mood_changes" => Some(Self::MoodChanges),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Mild,
    Moderate,
    Severe,
}

impl Severity {
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Mild => 1,
            Self::Moderate => 2,
            Self::Severe => 3,
        }
    }

    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Mild),
            2 => Some(Self::Moderate),
            3 => Some(Self::Severe),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymptomEntry {
    pub code: SymptomCode,
    pub severity: Severity,
}
