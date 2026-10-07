use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::wrap_with_radio;
use super::super::rule::Rule;

pub struct PitsRule {
    prev_pits_open: bool,
    pits_open_announced: bool,
}

impl PitsRule {
    pub fn new() -> Self {
        Self {
            prev_pits_open: false,
            pits_open_announced: false,
        }
    }

    pub fn sync_prev(&mut self, pits_open: bool) {
        self.prev_pits_open = pits_open;
    }
}

impl Default for PitsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PitsRule {
    fn id(&self) -> &'static str {
        "pits"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_pits_open_enabled || !settings.audio_strategy_enabled {
            return;
        }
        if ctx.session_mode.is_practice() {
            return;
        }
        if ctx.snap.pits_open && !self.prev_pits_open && !self.pits_open_announced {
            out.push(Candidate {
                priority: SpeechPriority::RACE,
                plan: wrap_with_radio(settings, SpeechPlan::clip("pits_open")),
                mark: Mark::PitsOpen,
            });
        }
    }

    fn apply_mark(&mut self, _ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::PitsOpen) {
            self.pits_open_announced = true;
        }
    }

    fn on_session_reset(&mut self) {
        self.prev_pits_open = false;
        self.pits_open_announced = false;
    }
}
