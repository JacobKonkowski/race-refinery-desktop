use race_refinery_settings::AppSettings;

use super::super::super::phrasing::{push_laps_of_fuel, push_laps_short, push_liters};
use super::super::super::queue::SpeechPriority;
use super::super::super::speech::{SpeechPlan, SpeechUnit};
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::{estimate_laps_remaining, wrap_with_radio};
use super::super::rule::Rule;

pub struct FuelRule {
    pub(crate) fuel_per_lap: Vec<f32>,
    spoke_fuel_low: bool,
    spoke_fuel_to_end: bool,
    spoke_pit_to_finish: bool,
    spoke_one_more_stop: bool,
}

impl FuelRule {
    pub fn new() -> Self {
        Self {
            fuel_per_lap: Vec::new(),
            spoke_fuel_low: false,
            spoke_fuel_to_end: false,
            spoke_pit_to_finish: false,
            spoke_one_more_stop: false,
        }
    }

    pub fn fuel_per_lap(&self) -> &[f32] {
        &self.fuel_per_lap
    }

    pub fn record_fuel_use(&mut self, start_fuel: Option<f32>, end_fuel: f32) {
        if let Some(start) = start_fuel {
            if start > end_fuel && start - end_fuel > 0.05 {
                self.fuel_per_lap.push(start - end_fuel);
                if self.fuel_per_lap.len() > 8 {
                    self.fuel_per_lap.remove(0);
                }
            }
        }
    }

    pub fn reset_fuel_low_after_lap(&mut self) {
        self.spoke_fuel_low = false;
    }
}

impl Default for FuelRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for FuelRule {
    fn id(&self) -> &'static str {
        "fuel"
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        self.gather_race_fuel(ctx, settings, out);
        self.gather_fuel_low(ctx, settings, out);
    }

    fn apply_mark(&mut self, _ctx: &RaceContext<'_>, mark: &Mark) {
        match mark {
            Mark::FuelLow => self.spoke_fuel_low = true,
            Mark::FuelToEnd => self.spoke_fuel_to_end = true,
            Mark::PitToFinish => self.spoke_pit_to_finish = true,
            Mark::FuelOneMoreStop => self.spoke_one_more_stop = true,
            _ => {}
        }
    }

    fn on_session_reset(&mut self) {
        self.fuel_per_lap.clear();
        self.spoke_fuel_low = false;
        self.spoke_fuel_to_end = false;
        self.spoke_pit_to_finish = false;
        self.spoke_one_more_stop = false;
    }
}

impl FuelRule {
    fn gather_race_fuel(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        out: &mut Vec<Candidate>,
    ) {
        if !settings.audio_fuel_race_enabled || !settings.audio_strategy_enabled {
            return;
        }
        if !ctx.session_mode.is_race() {
            return;
        }
        let Some(laps_remain) = ctx.snap.session_laps_remain else {
            return;
        };
        if laps_remain <= 0 || laps_remain > 60 {
            return;
        }
        let Some(laps_of_fuel) = estimate_laps_remaining(ctx.snap.fuel_level, &self.fuel_per_lap)
        else {
            return;
        };

        let sensitivity = settings.audio_fuel_strategy_sensitivity.as_str();
        let margin = if sensitivity == "conservative" {
            0.8
        } else {
            0.3
        };

        if laps_of_fuel + margin < laps_remain as f32 && !self.spoke_pit_to_finish {
            let short_by = (laps_remain as f32 - laps_of_fuel).ceil().max(1.0) as u32;
            let mut units = vec![SpeechUnit::Clip("fuel_short_on_fuel".into())];
            push_laps_short(&mut units, short_by);
            units.push(SpeechUnit::Clip("fuel_plan_stop".into()));
            out.push(Candidate {
                priority: SpeechPriority::RACE,
                plan: wrap_with_radio(settings, SpeechPlan::sequence(units)),
                mark: Mark::PitToFinish,
            });
        } else if laps_of_fuel >= laps_remain as f32 + 1.0
            && laps_remain <= 8
            && !self.spoke_one_more_stop
            && sensitivity != "conservative"
        {
            out.push(Candidate {
                priority: SpeechPriority::RACE,
                plan: wrap_with_radio(settings, SpeechPlan::clip("fuel_one_more_stop")),
                mark: Mark::FuelOneMoreStop,
            });
        } else if laps_of_fuel >= laps_remain as f32 && laps_remain <= 5 && !self.spoke_fuel_to_end
        {
            out.push(Candidate {
                priority: SpeechPriority::RACE,
                plan: wrap_with_radio(settings, SpeechPlan::clip("fuel_good_to_finish")),
                mark: Mark::FuelToEnd,
            });
        }
    }

    fn gather_fuel_low(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        out: &mut Vec<Candidate>,
    ) {
        if self.spoke_fuel_low
            || !settings.audio_strategy_enabled
            || settings.audio_coach_fuel_threshold <= 0.0
            || ctx.snap.fuel_level <= 0.0
            || ctx.snap.fuel_level > settings.audio_coach_fuel_threshold
        {
            return;
        }
        let mut units = vec![SpeechUnit::Clip("fuel_low".into())];
        push_liters(&mut units, ctx.snap.fuel_level);
        if let Some(laps_left) = estimate_laps_remaining(ctx.snap.fuel_level, &self.fuel_per_lap) {
            push_laps_of_fuel(&mut units, laps_left);
        }
        out.push(Candidate {
            priority: SpeechPriority::RACE,
            plan: wrap_with_radio(settings, SpeechPlan::sequence(units)),
            mark: Mark::FuelLow,
        });
    }
}
