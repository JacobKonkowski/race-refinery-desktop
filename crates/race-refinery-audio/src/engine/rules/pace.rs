use std::collections::{HashMap, HashSet};

use race_refinery_settings::AppSettings;

use super::super::super::phrasing::{
    push_delta, push_fuel_status, push_gap_seconds, push_lap_time_callout, push_position,
    push_sector_time_callout, push_uint,
};
use super::super::super::queue::SpeechPriority;
use super::super::super::speech::{SpeechPlan, SpeechUnit};
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::helpers::{
    chatter_allows_normal, chatter_allows_verbose, chatter_is_verbose, estimate_laps_remaining,
    push_pace_delta_units, push_pace_delta_with_suffix, wrap_with_radio,
};
use super::super::rule::Rule;

pub struct PaceRule {
    tracked_lap: i32,
    announced_sectors: HashSet<i32>,
    best_sector_ms: HashMap<i32, f64>,
    best_lap_ms: Option<f64>,
    fuel_at_lap_start: Option<f32>,
    last_lap_seen: Option<f64>,
    pending_lap_plan: Option<SpeechPlan>,
    last_announced_position: Option<i32>,
}

impl PaceRule {
    pub fn new() -> Self {
        Self {
            tracked_lap: 0,
            announced_sectors: HashSet::new(),
            best_sector_ms: HashMap::new(),
            best_lap_ms: None,
            fuel_at_lap_start: None,
            last_lap_seen: None,
            pending_lap_plan: None,
            last_announced_position: None,
        }
    }

    pub fn fuel_at_lap_start(&self) -> Option<f32> {
        self.fuel_at_lap_start
    }

    pub fn take_fuel_lap_reset(&mut self) -> Option<f32> {
        self.fuel_at_lap_start.take()
    }
}

impl Default for PaceRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PaceRule {
    fn id(&self) -> &'static str {
        "pace"
    }

    fn before_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings) {
        let snap = ctx.snap;
        if snap.lap != self.tracked_lap {
            let previously_announced = self.announced_sectors.clone();
            self.tracked_lap = snap.lap;
            self.announced_sectors.clear();
            for sector in &snap.sectors {
                if sector.completed && previously_announced.contains(&sector.sector_num) {
                    self.announced_sectors.insert(sector.sector_num);
                }
            }
            if snap.lap > 0 && self.fuel_at_lap_start.is_none() {
                self.fuel_at_lap_start = Some(snap.fuel_level);
            }
        }

        for sector in &snap.sectors {
            if sector.completed
                && !self.announced_sectors.contains(&sector.sector_num)
                && sector.time_ms.map(|ms| ms < 1000.0).unwrap_or(true)
            {
                self.announced_sectors.insert(sector.sector_num);
            }
        }

        if let Some(lap_ms) = snap.last_lap_ms {
            if lap_ms > 10_000.0 && self.last_lap_seen != Some(lap_ms) {
                self.last_lap_seen = Some(lap_ms);
                let completed_lap = snap.lap.saturating_sub(1).max(1);
                if snap.last_lap_valid {
                    self.pending_lap_plan = Some(wrap_with_radio(
                        settings,
                        self.lap_complete_plan(ctx, settings, completed_lap, lap_ms),
                    ));
                    self.best_lap_ms =
                        Some(self.best_lap_ms.map(|b| b.min(lap_ms)).unwrap_or(lap_ms));
                } else if settings.audio_invalid_lap_enabled {
                    let mut units = vec![
                        SpeechUnit::Clip("lap_invalid".into()),
                        SpeechUnit::Clip("lap".into()),
                    ];
                    push_lap_time_callout(&mut units, completed_lap, lap_ms);
                    self.pending_lap_plan =
                        Some(wrap_with_radio(settings, SpeechPlan::sequence(units)));
                } else {
                    let mut units = vec![SpeechUnit::Clip("lap".into())];
                    push_lap_time_callout(&mut units, completed_lap, lap_ms);
                    units.push(SpeechUnit::Clip("out_lap".into()));
                    self.pending_lap_plan =
                        Some(wrap_with_radio(settings, SpeechPlan::sequence(units)));
                }
                self.fuel_at_lap_start = Some(snap.fuel_level);
            }
        }
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if let Some(plan) = &self.pending_lap_plan {
            out.push(Candidate {
                priority: SpeechPriority::PACE,
                plan: plan.clone(),
                mark: if ctx.snap.last_lap_valid {
                    Mark::LapComplete
                } else {
                    Mark::InvalidLap
                },
            });
        }

        if !settings.audio_pace_enabled {
            return;
        }
        for sector in &ctx.snap.sectors {
            if !sector.completed || self.announced_sectors.contains(&sector.sector_num) {
                continue;
            }
            let Some(ms) = sector.time_ms else { continue };
            if ms < 1000.0 {
                continue;
            }
            let prev_best = self.best_sector_ms.get(&sector.sector_num).copied();
            let is_pb = prev_best.map(|b| ms < b - 20.0).unwrap_or(true);
            let plan = wrap_with_radio(
                settings,
                self.sector_plan(ctx, settings, sector.sector_num, ms, prev_best, is_pb),
            );
            out.push(Candidate {
                priority: SpeechPriority::PACE,
                plan,
                mark: Mark::Sector {
                    num: sector.sector_num,
                    is_pb,
                    ms,
                },
            });
            break;
        }
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        match mark {
            Mark::Sector { num, is_pb, ms } => {
                if *is_pb {
                    self.best_sector_ms.insert(*num, *ms);
                }
                self.announced_sectors.insert(*num);
            }
            Mark::LapComplete | Mark::InvalidLap => {
                self.pending_lap_plan = None;
                if let Some(pos) = ctx.snap.player_class_position.or(ctx.snap.player_position) {
                    if pos > 0 {
                        self.last_announced_position = Some(pos);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_session_reset(&mut self) {
        *self = Self::new();
    }
}

impl PaceRule {
    pub fn lap_complete_gaps(&self, ctx: &RaceContext<'_>) -> (Option<f32>, Option<f32>) {
        (ctx.snap.gap_to_car_ahead_s, ctx.snap.gap_to_car_behind_s)
    }

    pub fn last_announced_position(&self) -> Option<i32> {
        self.last_announced_position
    }

    fn sector_plan(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        sector_num: i32,
        ms: f64,
        prev_best: Option<f64>,
        is_pb: bool,
    ) -> SpeechPlan {
        let mut units = vec![SpeechUnit::Clip("sector".into())];
        push_sector_time_callout(&mut units, sector_num, ms);

        if is_pb && prev_best.is_some() {
            units.push(SpeechUnit::Clip("pb_sector".into()));
        } else if let Some(b) = prev_best {
            if !is_pb {
                push_pace_delta_units(&mut units, ms - b);
            }
        }

        let show_live_pace = !(!is_pb && prev_best.is_some() && ms > prev_best.unwrap() + 20.0);
        if show_live_pace {
            if let Some(d) = ctx
                .snap
                .delta_to_best_ms
                .filter(|d| d.abs() > 80.0 && ctx.snap.lap_dist_pct > 0.05)
            {
                if d > 0.0 {
                    units.push(SpeechUnit::Clip("pace_off_pb_intro".into()));
                    push_delta(&mut units, d);
                } else {
                    units.push(SpeechUnit::Clip("pace_on_pb".into()));
                }
            }
        }

        if chatter_allows_verbose(settings)
            && (ctx.session_mode.is_qual() || ctx.session_mode.is_practice())
        {
            if let Some(d) = ctx.snap.delta_to_session_best_ms.filter(|d| d.abs() > 80.0) {
                push_pace_delta_units(&mut units, d);
            }
        }

        SpeechPlan::sequence(units)
    }

    fn lap_complete_plan(
        &self,
        ctx: &RaceContext<'_>,
        settings: &AppSettings,
        lap_num: i32,
        lap_ms: f64,
    ) -> SpeechPlan {
        let prev_best = self.best_lap_ms;
        let is_pb = prev_best.map(|b| lap_ms < b - 50.0).unwrap_or(true);

        let mut units = vec![SpeechUnit::Clip("lap".into())];
        push_lap_time_callout(&mut units, lap_num, lap_ms);

        if is_pb && prev_best.is_some() {
            units.push(SpeechUnit::Clip("pb_new".into()));
        } else if let Some(best) = prev_best {
            push_pace_delta_units(&mut units, lap_ms - best);
        }

        if let Some(last) = ctx.snap.delta_to_last_ms {
            if last.abs() > 80.0 && prev_best.is_some() && !is_pb {
                push_pace_delta_with_suffix(&mut units, last, "versus_previous_lap");
            }
        }

        if ctx.session_mode.is_qual()
            || (ctx.session_mode.is_practice() && chatter_is_verbose(settings))
        {
            if let Some(d) = ctx.snap.delta_to_session_best_ms.filter(|d| d.abs() > 80.0) {
                push_pace_delta_with_suffix(&mut units, d, "off_session_best");
            }
        }

        if (ctx.session_mode.is_race() || ctx.session_mode.is_qual())
            && settings.audio_position_callouts_enabled
            && chatter_allows_normal(settings)
        {
            if let Some(pos) = ctx.snap.player_class_position.or(ctx.snap.player_position) {
                if let Some(prev) = self.last_announced_position {
                    if pos > 0 && prev > 0 && pos != prev {
                        let clip = if pos < prev {
                            "position_up"
                        } else {
                            "position_down"
                        };
                        units.push(SpeechUnit::Clip(clip.into()));
                        push_uint(&mut units, pos as u32);
                    } else if pos > 0 {
                        push_position(&mut units, pos);
                    }
                } else if pos > 0 {
                    push_position(&mut units, pos);
                }
            }
        }

        if settings.audio_gap_alerts_enabled
            && (ctx.session_mode.is_race()
                || ctx.session_mode.is_qual()
                || chatter_is_verbose(settings))
        {
            if let Some(g) = ctx.snap.gap_to_car_ahead_s.filter(|g| *g >= 0.0) {
                units.push(SpeechUnit::Clip("gap_ahead".into()));
                push_gap_seconds(&mut units, g);
            }
            if let Some(g) = ctx.snap.gap_to_car_behind_s.filter(|g| *g >= 0.0) {
                units.push(SpeechUnit::Clip("gap_behind".into()));
                push_gap_seconds(&mut units, g);
            }
        }

        if ctx.session_mode.is_race()
            && settings.audio_strategy_enabled
            && ctx.snap.fuel_level > 0.0
        {
            let laps_left = estimate_laps_remaining(ctx.snap.fuel_level, ctx.fuel_per_lap);
            push_fuel_status(&mut units, ctx.snap.fuel_level, laps_left);
        }

        SpeechPlan::sequence(units)
    }
}
