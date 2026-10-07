use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::wrap_with_radio;
use super::super::rule::Rule;

const HOT_TYRE_THRESHOLD: f32 = 95.0;

pub struct TyresRule {
    warned_hot: bool,
}

impl TyresRule {
    pub fn new() -> Self {
        Self { warned_hot: false }
    }
}

impl Default for TyresRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for TyresRule {
    fn id(&self) -> &'static str {
        "tyres"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_tyre_alerts_enabled || self.warned_hot {
            return;
        }
        if ctx.snap.on_pit_road || !ctx.snap.on_track {
            return;
        }
        let temps = [
            ctx.snap.lf_temp,
            ctx.snap.rf_temp,
            ctx.snap.lr_temp,
            ctx.snap.rr_temp,
        ];
        let max = temps.iter().copied().fold(0.0_f32, f32::max);
        if max < HOT_TYRE_THRESHOLD {
            return;
        }
        out.push(Candidate {
            priority: SpeechPriority::RACE,
            plan: wrap_with_radio(settings, SpeechPlan::clip("tyre_hot")),
            mark: Mark::TyreHot,
        });
    }

    fn apply_mark(&mut self, _ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::TyreHot) {
            self.warned_hot = true;
        }
    }

    fn on_session_reset(&mut self) {
        self.warned_hot = false;
    }
}
