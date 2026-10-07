use race_refinery_live::LiveSnapshot;

use super::super::session_mode::SessionMode;

pub use race_refinery_live::CoachSessionMeta as SessionMeta;

/// Read-only view passed to race-engine rules each tick.
pub struct RaceContext<'a> {
    pub snap: &'a LiveSnapshot,
    pub session_mode: SessionMode,
    pub meta: &'a SessionMeta,
    pub fuel_per_lap: &'a [f32],
}

impl<'a> RaceContext<'a> {
    pub fn new(snap: &'a LiveSnapshot, meta: &'a SessionMeta, fuel_per_lap: &'a [f32]) -> Self {
        Self {
            session_mode: SessionMode::from_session_type(&snap.session_type),
            snap,
            meta,
            fuel_per_lap,
        }
    }
}
