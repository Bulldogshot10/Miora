#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Calendar,
    CheckIn,
    Insights,
}

impl Screen {
    pub const fn as_index(self) -> i32 {
        match self {
            Self::Home => 0,
            Self::Calendar => 1,
            Self::CheckIn => 2,
            Self::Insights => 3,
        }
    }

    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Calendar,
            2 => Self::CheckIn,
            3 => Self::Insights,
            _ => Self::Home,
        }
    }
}
