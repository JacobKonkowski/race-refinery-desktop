use race_refinery_settings::AppSettings;

use super::candidate::{Candidate, Mark};
use super::context::RaceContext;

pub trait Rule {
    fn id(&self) -> &'static str;

    /// Called before candidate gathering (lap transitions, baselines, etc.).
    fn before_tick(&mut self, _ctx: &RaceContext<'_>, _settings: &AppSettings) {}

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>);

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark);

    fn on_session_reset(&mut self);
}
