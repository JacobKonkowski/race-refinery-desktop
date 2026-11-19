/// iRacing `SessionFlags` bit masks (`irsdk_Flags`) used for edge detection.
///
/// Values match the SDK: driver black is `0x00010000`, not `0x00000040` (that bit
/// is debris and caused false black-flag callouts).
pub const CHECKERED: u32 = 0x0000_0001;
pub const WHITE: u32 = 0x0000_0002;
pub const GREEN: u32 = 0x0000_0004;
pub const YELLOW: u32 = 0x0000_0008;
pub const RED: u32 = 0x0000_0010;
/// Player is being shown the blue (faster car / being lapped).
pub const BLUE: u32 = 0x0000_0020;
/// Track debris — not a black flag.
pub const DEBRIS: u32 = 0x0000_0040;
pub const YELLOW_WAVING: u32 = 0x0000_0100;
pub const GREEN_HELD: u32 = 0x0000_0400;
/// Player has a black (penalty) flag.
pub const BLACK: u32 = 0x0001_0000;
