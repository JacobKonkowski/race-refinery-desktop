use serde::{Deserialize, Serialize};

/// Spotter pack state derived from the iRacing `CarLeftRight` enum.
///
/// `CarLeftRight` is stored as a bitfield variable but holds a single enum
/// value (irsdk_CarLeftRight), not OR-able flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PackState {
    /// Not in the world (garage / tow / not on track).
    #[default]
    Off,
    /// On track with no cars alongside.
    Clear,
    CarLeft,
    CarRight,
    /// Cars on both sides (three wide, you in the middle).
    ThreeWide,
    TwoCarsLeft,
    TwoCarsRight,
}

impl PackState {
    /// Map the raw `CarLeftRight` enum value to a `PackState`.
    pub fn from_car_left_right(value: u32) -> Self {
        match value {
            0 => PackState::Off,
            1 => PackState::Clear,
            2 => PackState::CarLeft,
            3 => PackState::CarRight,
            4 => PackState::ThreeWide,
            5 => PackState::TwoCarsLeft,
            6 => PackState::TwoCarsRight,
            _ => PackState::Off,
        }
    }

    /// True when at least one car is racing alongside the player.
    pub fn is_traffic(self) -> bool {
        matches!(
            self,
            PackState::CarLeft
                | PackState::CarRight
                | PackState::ThreeWide
                | PackState::TwoCarsLeft
                | PackState::TwoCarsRight
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_values() {
        assert_eq!(PackState::from_car_left_right(0), PackState::Off);
        assert_eq!(PackState::from_car_left_right(1), PackState::Clear);
        assert_eq!(PackState::from_car_left_right(4), PackState::ThreeWide);
        assert_eq!(PackState::from_car_left_right(6), PackState::TwoCarsRight);
        assert_eq!(PackState::from_car_left_right(99), PackState::Off);
    }

    #[test]
    fn traffic_detection() {
        assert!(!PackState::Clear.is_traffic());
        assert!(!PackState::Off.is_traffic());
        assert!(PackState::CarLeft.is_traffic());
        assert!(PackState::ThreeWide.is_traffic());
    }
}
