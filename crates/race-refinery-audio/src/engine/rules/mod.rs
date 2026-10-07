mod flags;
mod fuel;
mod gaps;
mod incidents;
mod intro;
mod pace;
mod pack;
mod pits;
mod race_clock;
mod tyres;

pub use flags::FlagsRule;
pub use fuel::FuelRule;
pub use gaps::GapsRule;
pub use incidents::IncidentsRule;
pub use intro::IntroRule;
pub use pace::PaceRule;
pub use pack::PackRule;
pub use pits::PitsRule;
pub use race_clock::RaceClockRule;
pub use tyres::TyresRule;

use race_refinery_settings::AppSettings;

use super::candidate::{Candidate, Mark};
use super::context::RaceContext;
use super::rule::Rule;

pub struct RuleSet {
    pub intro: IntroRule,
    pub flags: FlagsRule,
    pub incidents: IncidentsRule,
    pub pack: PackRule,
    pub gaps: GapsRule,
    pub fuel: FuelRule,
    pub race_clock: RaceClockRule,
    pub pits: PitsRule,
    pub pace: PaceRule,
    pub tyres: TyresRule,
}

impl RuleSet {
    pub fn new() -> Self {
        Self {
            intro: IntroRule::new(),
            flags: FlagsRule::new(),
            incidents: IncidentsRule::new(),
            pack: PackRule::new(),
            gaps: GapsRule::new(),
            fuel: FuelRule::new(),
            race_clock: RaceClockRule::new(),
            pits: PitsRule::new(),
            pace: PaceRule::new(),
            tyres: TyresRule::new(),
        }
    }

    pub fn on_session_reset(&mut self) {
        self.intro.on_session_reset();
        self.flags.on_session_reset();
        self.incidents.on_session_reset();
        self.pack.on_session_reset();
        self.gaps.on_session_reset();
        self.fuel.on_session_reset();
        self.race_clock.on_session_reset();
        self.pits.on_session_reset();
        self.pace.on_session_reset();
        self.tyres.on_session_reset();
    }

    pub fn before_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings) {
        self.flags.before_tick(ctx, settings);
        self.incidents.before_tick(ctx, settings);
        self.pace.before_tick(ctx, settings);
        self.pack.update_after_maintenance(ctx.snap);
    }

    pub fn after_tick(&mut self, ctx: &RaceContext<'_>) {
        self.race_clock.after_tick(ctx);
        self.pits.sync_prev(ctx.snap.pits_open);
    }

    pub fn gather(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings) -> Vec<Candidate> {
        let mut out = Vec::new();
        self.intro.on_tick(ctx, settings, &mut out);
        self.flags.on_tick(ctx, settings, &mut out);
        self.incidents.on_tick(ctx, settings, &mut out);
        self.pack.on_tick(ctx, settings, &mut out);
        self.fuel.on_tick(ctx, settings, &mut out);
        self.race_clock.on_tick(ctx, settings, &mut out);
        self.pits.on_tick(ctx, settings, &mut out);
        self.gaps.on_tick(ctx, settings, &mut out);
        self.pace.on_tick(ctx, settings, &mut out);
        self.tyres.on_tick(ctx, settings, &mut out);
        out
    }

    pub fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark, settings: &AppSettings) {
        self.intro.apply_mark(ctx, mark);
        self.flags.apply_mark(ctx, mark);
        self.incidents.apply_mark(ctx, mark);
        self.pack.apply_mark(ctx, mark);
        self.gaps.apply_mark(ctx, mark);
        self.fuel.apply_mark(ctx, mark);
        self.race_clock.apply_mark(ctx, mark);
        self.pits.apply_mark(ctx, mark);
        self.pace.apply_mark(ctx, mark);
        self.tyres.apply_mark(ctx, mark);

        if matches!(mark, Mark::LapComplete | Mark::InvalidLap) {
            let start = self.pace.fuel_at_lap_start();
            self.fuel.record_fuel_use(start, ctx.snap.fuel_level);
            if settings.audio_fuel_race_enabled {
                self.fuel.reset_fuel_low_after_lap();
            }
            let (ahead, behind) = self.pace.lap_complete_gaps(ctx);
            self.gaps.sync_from_lap_complete(ahead, behind);
        }
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}
