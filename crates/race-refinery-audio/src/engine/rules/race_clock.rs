use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::{chatter_allows_verbose, wrap_with_radio};
use super::super::rule::Rule;

pub struct RaceClockRule {
    spoke_five_laps: bool,
    spoke_final_lap: bool,
    spoke_five_minutes: bool,
    spoke_one_minute: bool,
    last_time_remain_s: Option<f64>,
}

impl RaceClockRule {
    pub fn new() -> Self {
        Self {
            spoke_five_laps: false,
            spoke_final_lap: false,
            spoke_five_minutes: false,
            spoke_one_minute: false,
            last_time_remain_s: None,
        }
    }

    pub fn after_tick(&mut self, ctx: &RaceContext<'_>) {
        self.last_time_remain_s = ctx.snap.session_time_remain_s;
    }
}

impl Default for RaceClockRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for RaceClockRule {
    fn id(&self) -> &'static str {
        "race_clock"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_race_clock_enabled || !settings.audio_strategy_enabled {
            return;
        }
        if !chatter_allows_verbose(settings) {
            return;
        }
        if !ctx.session_mode.is_race() {
            return;
        }
        if let Some(laps) = ctx.snap.session_laps_remain {
            if laps == 5 && !self.spoke_five_laps {
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip("race_five_laps")),
                    mark: Mark::RaceClock,
                });
                return;
            }
            if laps == 1 && !self.spoke_final_lap {
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip("race_final_lap")),
                    mark: Mark::RaceClock,
                });
                return;
            }
        }
        if let (Some(cur), Some(prev)) = (ctx.snap.session_time_remain_s, self.last_time_remain_s) {
            if prev > 300.0 && cur <= 300.0 && !self.spoke_five_minutes {
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip("race_five_minutes")),
                    mark: Mark::RaceClock,
                });
                return;
            }
            if prev > 60.0 && cur <= 60.0 && !self.spoke_one_minute {
                out.push(Candidate {
                    priority: SpeechPriority::RACE,
                    plan: wrap_with_radio(settings, SpeechPlan::clip("race_one_minute")),
                    mark: Mark::RaceClock,
                });
            }
        }
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::RaceClock) {
            if ctx.snap.session_laps_remain == Some(5) {
                self.spoke_five_laps = true;
            }
            if ctx.snap.session_laps_remain == Some(1) {
                self.spoke_final_lap = true;
            }
            if ctx
                .snap
                .session_time_remain_s
                .map(|t| t <= 300.0)
                .unwrap_or(false)
            {
                self.spoke_five_minutes = true;
            }
            if ctx
                .snap
                .session_time_remain_s
                .map(|t| t <= 60.0)
                .unwrap_or(false)
            {
                self.spoke_one_minute = true;
            }
        }
    }

    fn on_session_reset(&mut self) {
        self.spoke_five_laps = false;
        self.spoke_final_lap = false;
        self.spoke_five_minutes = false;
        self.spoke_one_minute = false;
        self.last_time_remain_s = None;
    }
}
