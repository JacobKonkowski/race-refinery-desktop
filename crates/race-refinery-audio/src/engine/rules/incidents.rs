use race_refinery_settings::AppSettings;

use super::super::super::phrasing::push_incidents;
use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::wrap_with_radio;
use super::super::rule::Rule;

pub struct IncidentsRule {
    last_incident_count: i32,
    baseline_set: bool,
}

impl IncidentsRule {
    pub fn new() -> Self {
        Self {
            last_incident_count: 0,
            baseline_set: false,
        }
    }
}

impl Default for IncidentsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for IncidentsRule {
    fn id(&self) -> &'static str {
        "incidents"
    }

    fn before_tick(&mut self, ctx: &RaceContext<'_>, _settings: &AppSettings) {
        if !self.baseline_set {
            self.last_incident_count = ctx.snap.incident_count;
            self.baseline_set = true;
        }
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_incidents_enabled || !self.baseline_set {
            return;
        }
        if ctx.snap.incident_count > self.last_incident_count {
            let mut units = Vec::new();
            push_incidents(
                &mut units,
                ctx.snap.incident_count.max(0) as u32,
                ctx.snap.incident_limit.map(|l| l.max(0) as u32),
            );
            out.push(Candidate {
                priority: SpeechPriority::SAFETY,
                plan: wrap_with_radio(settings, SpeechPlan::sequence(units)),
                mark: Mark::Incident,
            });
        }
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::Incident) {
            self.last_incident_count = ctx.snap.incident_count;
        }
    }

    fn on_session_reset(&mut self) {
        self.last_incident_count = 0;
        self.baseline_set = false;
    }
}
