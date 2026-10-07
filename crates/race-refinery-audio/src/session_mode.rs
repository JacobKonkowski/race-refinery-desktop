/// Session type classification for coach message gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Practice,
    Qualifying,
    Race,
    Other,
}

impl SessionMode {
    pub fn from_session_type(session_type: &str) -> Self {
        let lower = session_type.to_ascii_lowercase();
        if lower.contains("qual") {
            Self::Qualifying
        } else if lower.contains("race") {
            Self::Race
        } else if lower.contains("practice") || lower.contains("warmup") || lower.contains("test") {
            Self::Practice
        } else {
            Self::Other
        }
    }

    pub fn is_race(self) -> bool {
        matches!(self, Self::Race)
    }

    pub fn is_qual(self) -> bool {
        matches!(self, Self::Qualifying)
    }

    pub fn is_practice(self) -> bool {
        matches!(self, Self::Practice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_types() {
        assert_eq!(
            SessionMode::from_session_type("Qualifying"),
            SessionMode::Qualifying
        );
        assert_eq!(SessionMode::from_session_type("Race"), SessionMode::Race);
        assert_eq!(
            SessionMode::from_session_type("Open Practice"),
            SessionMode::Practice
        );
    }
}
