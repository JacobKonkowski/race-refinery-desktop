use super::super::queue::SpeechPriority;
use super::super::speech::SpeechPlan;

#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Intro,
    Sector { num: i32, is_pb: bool, ms: f64 },
    LapComplete,
    InvalidLap,
    Flags,
    Incident,
    Pack,
    PackPrecursor,
    PackClear,
    FuelLow,
    FuelToEnd,
    PitToFinish,
    FuelOneMoreStop,
    GapChange,
    RaceClock,
    PitsOpen,
    TyreHot,
    VoiceResponse,
}

pub struct Candidate {
    pub priority: SpeechPriority,
    pub plan: SpeechPlan,
    pub mark: Mark,
}

pub fn pick_highest(candidates: &[Candidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| c.priority)
        .map(|(i, _)| i)
}
