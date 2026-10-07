use std::time::{Duration, Instant};

use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::{chatter_cooldown_multiplier, chatter_is_verbose, wrap_with_radio};
use super::super::rule::Rule;

const GAP_CHANGE_THRESHOLD_S: f32 = 0.3;
const GAP_COOLDOWN: Duration = Duration::from_secs(9);

pub struct GapsRule {
    last_announced_gap_ahead: Option<f32>,
    last_announced_gap_behind: Option<f32>,
    last_gap_spoken: Option<Instant>,
}

impl GapsRule {
    pub fn new() -> Self {
        Self {
            last_announced_gap_ahead: None,
            last_announced_gap_behind: None,
            last_gap_spoken: None,
        }
    }

    pub fn sync_from_lap_complete(&mut self, gap_ahead: Option<f32>, gap_behind: Option<f32>) {
        self.last_announced_gap_ahead = gap_ahead;
        self.last_announced_gap_behind = gap_behind;
    }
}

impl Default for GapsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for GapsRule {
    fn id(&self) -> &'static str {
        "gaps"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_gap_alerts_enabled || !chatter_is_verbose(settings) {
            return;
        }
        if ctx.snap.on_pit_road || !ctx.snap.on_track {
            return;
        }
        let cooldown = Duration::from_secs_f64(
            GAP_COOLDOWN.as_secs_f64() * chatter_cooldown_multiplier(settings),
        );
        let cooled = self
            .last_gap_spoken
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true);
        if !cooled {
            return;
        }

        if let (Some(cur), Some(prev)) =
            (ctx.snap.gap_to_car_ahead_s, self.last_announced_gap_ahead)
        {
            let delta = cur - prev;
            if delta.abs() >= GAP_CHANGE_THRESHOLD_S {
                let clip = if delta < 0.0 {
                    "gaining_ahead"
                } else {
                    "losing_ahead"
                };
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
                    mark: Mark::GapChange,
                });
                return;
            }
        }
        if let (Some(cur), Some(prev)) =
            (ctx.snap.gap_to_car_behind_s, self.last_announced_gap_behind)
        {
            let delta = cur - prev;
            if delta.abs() >= GAP_CHANGE_THRESHOLD_S {
                let clip = if delta > 0.0 {
                    "gaining_behind"
                } else {
                    "losing_behind"
                };
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
                    mark: Mark::GapChange,
                });
            }
        }
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::GapChange) {
            self.last_announced_gap_ahead = ctx.snap.gap_to_car_ahead_s;
            self.last_announced_gap_behind = ctx.snap.gap_to_car_behind_s;
            self.last_gap_spoken = Some(Instant::now());
        }
    }

    fn on_session_reset(&mut self) {
        self.last_announced_gap_ahead = None;
        self.last_announced_gap_behind = None;
        self.last_gap_spoken = None;
    }
}
