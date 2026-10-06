mod candidate;
mod context;
pub mod flags;
mod helpers;
mod rule;
pub mod rules;

pub use context::{RaceContext, SessionMeta};
pub use rule::Rule;
pub use rules::RuleSet;

use race_refinery_live::LiveSnapshot;
use race_refinery_settings::AppSettings;

use super::queue::SpeechPriority;
use super::speech::SpeechPlan;
use candidate::pick_highest;

/// Modular iRacing race engineer — replaces the monolithic `CoachEngine`.
pub struct RaceEngine {
    session_key: String,
    session_meta: SessionMeta,
    rules: RuleSet,
}

impl RaceEngine {
    pub fn new() -> Self {
        Self {
            session_key: String::new(),
            session_meta: SessionMeta::default(),
            rules: RuleSet::new(),
        }
    }

    pub fn reset_session(&mut self) {
        self.rules.on_session_reset();
    }

    pub fn set_session_meta(&mut self, meta: SessionMeta) {
        self.session_meta = meta;
    }

    /// Poll for at most one new speech plan; caller enqueues into `SpeechQueue`.
    pub fn poll(
        &mut self,
        snap: &LiveSnapshot,
        settings: &AppSettings,
    ) -> Option<(SpeechPriority, SpeechPlan)> {
        if snap.lap <= 0 && snap.track.is_empty() {
            return None;
        }

        self.maybe_reset_session(snap);
        let fuel_per_lap = self.rules.fuel.fuel_per_lap().to_vec();
        let ctx = RaceContext::new(snap, &self.session_meta, &fuel_per_lap);

        self.rules.before_tick(&ctx, settings);

        let mut candidates = self.rules.gather(&ctx, settings);

        let suppressed = snap.on_pit_road || !snap.on_track;
        if suppressed {
            candidates.retain(|c| c.priority >= SpeechPriority::SAFETY);
        }

        let idx = pick_highest(&candidates)?;
        let chosen = candidates.swap_remove(idx);
        self.rules.apply_mark(&ctx, &chosen.mark, settings);
        self.rules.after_tick(&ctx);

        Some((chosen.priority, chosen.plan))
    }

    fn maybe_reset_session(&mut self, snap: &LiveSnapshot) {
        let key = format!("{}|{}", snap.track, snap.session_type);
        if self.session_key.is_empty() {
            self.session_key = key;
            return;
        }
        if key != self.session_key {
            self.reset_session();
            self.session_key = key;
        }
    }
}

impl Default for RaceEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub type CoachEngine = RaceEngine;

#[cfg(test)]
mod tests {
    use super::*;
    use race_refinery_live::PackState;
    use race_refinery_settings::AppSettings;

    fn settings() -> AppSettings {
        AppSettings::default()
    }

    fn base_snapshot() -> LiveSnapshot {
        LiveSnapshot {
            track: "Test Track".into(),
            session_type: "Race".into(),
            lap: 1,
            on_track: true,
            ..Default::default()
        }
    }

    fn clip_key(plan: &SpeechPlan) -> Option<&str> {
        match plan {
            SpeechPlan::Clip(k) => Some(k.as_str()),
            _ => None,
        }
    }

    #[test]
    fn intro_spoken_first() {
        let mut engine = RaceEngine::new();
        let plan = engine.poll(&base_snapshot(), &settings());
        assert!(plan.is_some());
        assert!(plan.unwrap().1.display_text().contains("[intro_online]"));
    }

    #[test]
    fn red_flag_beats_pack_alert() {
        let mut engine = RaceEngine::new();
        engine.poll(&base_snapshot(), &settings());

        let mut snap = base_snapshot();
        snap.session_flags = flags::RED;
        snap.pack_state = PackState::ThreeWide;
        let plan = engine.poll(&snap, &settings()).unwrap();
        assert!(matches!(plan.1, SpeechPlan::Clip(k) if k == "flag_red"));
    }

    #[test]
    fn pack_alert_suppressed_in_pits() {
        let mut engine = RaceEngine::new();
        engine.poll(&base_snapshot(), &settings());

        let mut snap = base_snapshot();
        snap.pack_state = PackState::CarLeft;
        snap.on_pit_road = true;
        assert!(engine.poll(&snap, &settings()).is_none());
    }

    #[test]
    fn incident_increase_announced() {
        let mut engine = RaceEngine::new();
        engine.poll(&base_snapshot(), &settings());

        let mut snap = base_snapshot();
        snap.incident_count = 4;
        let plan = engine.poll(&snap, &settings()).unwrap();
        assert!(plan.1.display_text().contains("4"));
    }

    #[test]
    fn session_reset_on_track_change() {
        let mut engine = RaceEngine::new();
        let intro = engine.poll(&base_snapshot(), &settings());
        assert!(intro.is_some());

        let mut snap = base_snapshot();
        snap.track = "Other Track".into();
        let intro2 = engine.poll(&snap, &settings());
        assert!(intro2.is_some());
        assert!(intro2.unwrap().1.display_text().contains("[intro_online]"));
    }

    #[test]
    fn race_fuel_muted_in_practice() {
        let mut engine = RaceEngine::new();
        engine.rules.fuel.fuel_per_lap = vec![2.0];
        let mut snap = base_snapshot();
        snap.session_type = "Practice".into();
        snap.session_laps_remain = Some(10);
        snap.fuel_level = 5.0;
        engine.poll(&snap, &settings());
        let plan = engine.poll(&snap, &settings());
        assert!(plan.is_none() || !plan.unwrap().1.display_text().contains("fuel_short"));
    }

    #[test]
    fn qual_lap_plan_has_session_delta() {
        let mut engine = RaceEngine::new();
        engine.poll(&base_snapshot(), &settings());

        let mut snap = base_snapshot();
        snap.session_type = "Qualifying".into();
        snap.lap = 2;
        snap.last_lap_valid = true;
        snap.last_lap_ms = Some(92_000.0);
        snap.delta_to_session_best_ms = Some(400.0);
        engine.poll(&snap, &settings());
        let plan = engine.poll(&snap, &settings()).unwrap();
        assert!(plan.1.display_text().contains("session best"));
    }

    #[test]
    fn pack_clear_after_car_left() {
        let mut engine = RaceEngine::new();
        let s = settings();
        let mut snap = base_snapshot();
        engine.poll(&snap, &s);
        snap.pack_state = PackState::CarLeft;
        assert_eq!(
            clip_key(&engine.poll(&snap, &s).unwrap().1),
            Some("pack_car_left")
        );
        snap.pack_state = PackState::Clear;
        assert_eq!(
            clip_key(&engine.poll(&snap, &s).unwrap().1),
            Some("pack_clear_left")
        );
    }

    #[test]
    fn pack_clear_after_three_wide() {
        let mut engine = RaceEngine::new();
        let s = settings();
        let mut snap = base_snapshot();
        engine.poll(&snap, &s);
        snap.pack_state = PackState::ThreeWide;
        engine.poll(&snap, &s);
        snap.pack_state = PackState::Clear;
        assert_eq!(
            clip_key(&engine.poll(&snap, &s).unwrap().1),
            Some("pack_clear")
        );
    }

    #[test]
    fn pack_traffic_immediate_after_clear() {
        let mut engine = RaceEngine::new();
        let s = settings();
        let mut snap = base_snapshot();
        engine.poll(&snap, &s);
        snap.pack_state = PackState::CarLeft;
        engine.poll(&snap, &s);
        snap.pack_state = PackState::Clear;
        engine.poll(&snap, &s);
        snap.pack_state = PackState::CarLeft;
        assert_eq!(
            clip_key(&engine.poll(&snap, &s).unwrap().1),
            Some("pack_car_left")
        );
    }

    #[test]
    fn pack_clear_requires_pack_alerts() {
        let mut engine = RaceEngine::new();
        let mut s = settings();
        let mut snap = base_snapshot();
        engine.poll(&snap, &s);
        snap.pack_state = PackState::CarLeft;
        engine.poll(&snap, &s);
        snap.pack_state = PackState::Clear;
        s.audio_pack_alerts_enabled = false;
        assert!(engine.poll(&snap, &s).is_none());
    }

    #[test]
    fn pack_clear_suppressed_in_pits() {
        let mut engine = RaceEngine::new();
        let s = settings();
        let mut snap = base_snapshot();
        engine.poll(&snap, &s);
        snap.pack_state = PackState::CarLeft;
        engine.poll(&snap, &s);
        snap.pack_state = PackState::Clear;
        snap.on_pit_road = true;
        assert!(engine.poll(&snap, &s).is_none());
    }

    #[test]
    fn pace_matching_best_on_small_delta() {
        use super::super::speech::SpeechUnit;
        let mut units = Vec::new();
        super::helpers::push_pace_delta_units(&mut units, 30.0);
        assert!(matches!(&units[0], SpeechUnit::Clip(k) if k == "pace_matching_best"));
    }
}
