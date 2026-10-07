use std::time::{Duration, Instant};

use race_refinery_live::PackState;
use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::{chatter_cooldown_multiplier, pack_clear_clip, wrap_with_radio};
use super::super::rule::Rule;

const PACK_TRAFFIC_REMINDER: Duration = Duration::from_secs(3);
const PRECURSOR_DIST_THRESHOLD: f32 = 0.08;

/// Shortest distance along the lap between two `lap_dist_pct` values (0..1), circular.
fn circular_dist_pct(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    d.min(1.0 - d)
}

/// True when `other` is ahead of `player` on track (smaller forward distance than behind).
fn is_ahead_of(player: f32, other: f32) -> bool {
    let forward = (other - player).rem_euclid(1.0);
    let behind = (player - other).rem_euclid(1.0);
    forward > 0.0 && forward <= behind
}

pub struct PackRule {
    last_pack_state: PackState,
    last_traffic_spoken: Option<Instant>,
    spoke_precursor: bool,
}

impl PackRule {
    pub fn new() -> Self {
        Self {
            last_pack_state: PackState::Off,
            last_traffic_spoken: None,
            spoke_precursor: false,
        }
    }

    pub fn pack_state_before_tick(&self) -> PackState {
        self.last_pack_state
    }

    pub fn update_after_maintenance(&mut self, snap: &race_refinery_live::LiveSnapshot) {
        if !snap.pack_state.is_traffic() && snap.pack_state != PackState::Clear {
            self.last_pack_state = snap.pack_state;
        }
    }
}

impl Default for PackRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PackRule {
    fn id(&self) -> &'static str {
        "pack"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_pack_alerts_enabled {
            return;
        }
        if ctx.snap.on_pit_road || !ctx.snap.on_track {
            return;
        }

        self.gather_precursor(ctx, settings, out);
        self.gather_traffic(ctx, settings, out);
        self.gather_clear(ctx, settings, out);
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        match mark {
            Mark::Pack => {
                self.last_pack_state = ctx.snap.pack_state;
                self.last_traffic_spoken = Some(Instant::now());
                self.spoke_precursor = false;
            }
            Mark::PackClear => {
                self.last_pack_state = ctx.snap.pack_state;
            }
            Mark::PackPrecursor => {
                self.spoke_precursor = true;
            }
            _ => {}
        }
    }

    fn on_session_reset(&mut self) {
        self.last_pack_state = PackState::Off;
        self.last_traffic_spoken = None;
        self.spoke_precursor = false;
    }
}

impl PackRule {
    fn gather_precursor(
        &mut self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        out: &mut Vec<Candidate>,
    ) {
        if !settings.audio_pack_precursors_enabled || self.spoke_precursor {
            return;
        }
        if ctx.snap.pack_state.is_traffic() {
            return;
        }
        let player_idx = ctx.meta.player_car_idx;
        if player_idx < 0 {
            return;
        }
        let player_pct = ctx.snap.lap_dist_pct;
        for comp in &ctx.snap.competitors {
            if comp.car_idx == player_idx || comp.on_pit_road {
                continue;
            }
            let dist = circular_dist_pct(comp.lap_dist_pct, player_pct);
            // Ignore cars that are essentially overlapping / already alongside.
            if dist > PRECURSOR_DIST_THRESHOLD && dist < 0.25 {
                // "Approaching ahead" = car ahead of us (we're catching them).
                // "Approaching behind" = car behind us (they're catching us).
                let clip = if is_ahead_of(player_pct, comp.lap_dist_pct) {
                    "pack_approaching_ahead"
                } else {
                    "pack_approaching_behind"
                };
                out.push(Candidate {
                    priority: SpeechPriority::SAFETY,
                    plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
                    mark: Mark::PackPrecursor,
                });
                return;
            }
        }
    }

    fn gather_traffic(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        out: &mut Vec<Candidate>,
    ) {
        if !ctx.snap.pack_state.is_traffic() {
            return;
        }
        let changed = ctx.snap.pack_state != self.last_pack_state;
        let reminder_secs =
            (PACK_TRAFFIC_REMINDER.as_secs_f64() * chatter_cooldown_multiplier(settings)) as u64;
        if !changed {
            let remind = self
                .last_traffic_spoken
                .map(|t| t.elapsed() >= Duration::from_secs(reminder_secs.max(1)))
                .unwrap_or(true);
            if !remind {
                return;
            }
        }
        let clip = match ctx.snap.pack_state {
            PackState::CarLeft => "pack_car_left",
            PackState::CarRight => "pack_car_right",
            PackState::ThreeWide => "pack_three_wide_middle",
            PackState::TwoCarsLeft => "pack_two_left",
            PackState::TwoCarsRight => "pack_two_right",
            PackState::Clear | PackState::Off => return,
        };
        out.push(Candidate {
            priority: SpeechPriority::SAFETY,
            plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
            mark: Mark::Pack,
        });
    }

    fn gather_clear(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        out: &mut Vec<Candidate>,
    ) {
        if ctx.snap.pack_state != PackState::Clear {
            return;
        }
        if !self.last_pack_state.is_traffic() {
            return;
        }
        let clip = pack_clear_clip(self.last_pack_state);
        out.push(Candidate {
            priority: SpeechPriority::SAFETY,
            plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
            mark: Mark::PackClear,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_dist_wraps_near_start_finish() {
        assert!((circular_dist_pct(0.99, 0.01) - 0.02).abs() < 1e-5);
        assert!((circular_dist_pct(0.01, 0.99) - 0.02).abs() < 1e-5);
        assert!((circular_dist_pct(0.10, 0.20) - 0.10).abs() < 1e-5);
    }

    #[test]
    fn ahead_behind_across_start_finish() {
        // Player at 0.98, other at 0.02 ΓåÆ other is ahead (just past S/F).
        assert!(is_ahead_of(0.98, 0.02));
        // Player at 0.02, other at 0.98 ΓåÆ other is behind.
        assert!(!is_ahead_of(0.02, 0.98));
    }

    #[test]
    fn precursor_fires_across_lap_wrap() {
        use crate::engine::context::{RaceContext, SessionMeta};
        use race_refinery_live::CompetitorEntry;
        use race_refinery_live::LiveSnapshot;

        let snap = LiveSnapshot {
            track: "T".into(),
            session_type: "Race".into(),
            lap: 2,
            on_track: true,
            lap_dist_pct: 0.96,
            competitors: vec![CompetitorEntry {
                car_idx: 1,
                driver_name: "Other".into(),
                car_number: "2".into(),
                class_id: 0,
                class_color: String::new(),
                position: 1,
                class_position: 1,
                best_lap_ms: None,
                last_lap_ms: None,
                on_pit_road: false,
                is_player: false,
                lap_dist_pct: 0.05, // ~0.09 circular distance ahead across S/F
                gap_to_player_s: None,
            }],
            ..Default::default()
        };

        let meta = SessionMeta {
            player_car_idx: 0,
            ..SessionMeta::default()
        };
        let fuel = Vec::<f32>::new();
        let ctx = RaceContext::new(&snap, &meta, &fuel);
        let settings = AppSettings {
            audio_pack_alerts_enabled: true,
            audio_pack_precursors_enabled: true,
            ..AppSettings::default()
        };
        let mut rule = PackRule::new();
        let mut out = Vec::new();
        rule.on_tick(&ctx, &settings, &mut out);
        assert!(
            out.iter().any(|c| matches!(c.mark, Mark::PackPrecursor)),
            "expected pack precursor across S/F wrap"
        );
    }
}
