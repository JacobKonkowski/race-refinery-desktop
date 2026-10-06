use super::phrasing::{clip_label, number_clip_value};

/// A single playback unit — a voice pack clip or silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechUnit {
    Clip(String),
    /// Silence in milliseconds.
    Pause(u32),
}

/// Structured speech output from the coach engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechPlan {
    Clip(String),
    Sequence(Vec<SpeechUnit>),
}

impl SpeechPlan {
    pub fn clip(key: impl Into<String>) -> Self {
        Self::Clip(key.into())
    }

    pub fn sequence(units: Vec<SpeechUnit>) -> Self {
        Self::Sequence(units)
    }

    /// The plan as a unit list (a lone clip becomes a one-unit list).
    pub fn units(&self) -> Vec<SpeechUnit> {
        match self {
            Self::Clip(key) => vec![SpeechUnit::Clip(key.clone())],
            Self::Sequence(units) => units.clone(),
        }
    }

    /// Clip keys in playback order.
    pub fn clip_keys(&self) -> Vec<String> {
        self.units()
            .into_iter()
            .filter_map(|u| match u {
                SpeechUnit::Clip(key) => Some(key),
                SpeechUnit::Pause(_) => None,
            })
            .collect()
    }

    /// Human-readable line for the UI / logs. Number clips read back as digits:
    /// `[lap] 12, 1 minute 29.5`.
    pub fn display_text(&self) -> String {
        match self {
            Self::Clip(s) => s.clone(),
            Self::Sequence(units) => display_units(units),
        }
    }
}

fn display_units(units: &[SpeechUnit]) -> String {
    let mut words: Vec<String> = Vec::new();
    // Digits after `point` and the number after `position` attach to the previous word.
    let mut decimal = false;
    let mut glue = false;
    // The last word ends in a round tens clip that a following ones clip completes.
    let mut open_tens = false;

    for unit in units {
        if let SpeechUnit::Clip(key) = unit {
            if let Some(n) = number_clip_value(key) {
                let digits = n.to_string();
                match words.last_mut() {
                    Some(last) if open_tens && n < 10 => {
                        last.pop();
                        last.push_str(&digits);
                    }
                    Some(last) if decimal || glue => last.push_str(&digits),
                    _ => words.push(digits),
                }
                open_tens = !decimal && n >= 20 && n.is_multiple_of(10);
                glue = false;
                continue;
            }
            if key == "point" {
                match words.last_mut() {
                    Some(last) => last.push('.'),
                    None => words.push(".".into()),
                }
                decimal = true;
                open_tens = false;
                glue = false;
                continue;
            }
        }

        let word = match unit {
            SpeechUnit::Clip(key) => clip_label(key)
                .map(str::to_string)
                .unwrap_or_else(|| format!("[{key}]")),
            SpeechUnit::Pause(_) => {
                if let Some(last) = words.last_mut() {
                    last.push(',');
                }
                decimal = false;
                open_tens = false;
                glue = false;
                continue;
            }
        };
        match words.last_mut() {
            Some(last) if glue => last.push_str(&word),
            _ => words.push(word),
        }
        glue = matches!(unit, SpeechUnit::Clip(k) if k == "position");
        decimal = false;
        open_tens = false;
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::super::phrasing::{
        push_delta, push_gap_seconds, push_lap_time_callout, push_position, push_uint,
    };
    use super::*;

    fn text(build: impl FnOnce(&mut Vec<SpeechUnit>)) -> String {
        let mut units = Vec::new();
        build(&mut units);
        SpeechPlan::sequence(units).display_text()
    }

    #[test]
    fn numbers_read_back_as_digits() {
        assert_eq!(
            text(|u| {
                u.push(SpeechUnit::Clip("lap".into()));
                push_lap_time_callout(u, 12, 89_843.0);
            }),
            "[lap] 12, 1 minute 29.8"
        );
        assert_eq!(
            text(|u| push_lap_time_callout(u, 20, 65_300.0)),
            "20, 1 oh 5.3"
        );
        assert_eq!(text(|u| push_uint(u, 29)), "29");
        assert_eq!(text(|u| push_uint(u, 45)), "45");
        assert_eq!(text(|u| push_uint(u, 75)), "75");
        assert_eq!(text(|u| push_uint(u, 105)), "1 0 5");
    }

    #[test]
    fn deltas_gaps_and_positions() {
        assert_eq!(text(|u| push_delta(u, -1_440.0)), "1.4 seconds faster.");
        assert_eq!(text(|u| push_delta(u, 350.0)), "4 tenths slower.");
        assert_eq!(text(|u| push_gap_seconds(u, 3.0)), "3 seconds");
        assert_eq!(text(|u| push_position(u, 24)), "P24");
        assert_eq!(text(|u| push_position(u, 31)), "P31");
    }
}
