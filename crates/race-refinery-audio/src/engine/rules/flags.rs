use std::time::{Duration, Instant};

use race_refinery_settings::AppSettings;

use super::super::super::queue::SpeechPriority;
use super::super::super::speech::SpeechPlan;
use super::super::candidate::{Candidate, Mark};
use super::super::context::RaceContext;
use super::super::flags;
use super::super::helpers::wrap_with_radio;
use super::super::rule::Rule;

const FLAG_DEBOUNCE: Duration = Duration::from_millis(800);

pub struct FlagsRule {
    last_flags: u32,
    baseline_set: bool,
    last_flag_spoken: Option<Instant>,
}

impl FlagsRule {
    pub fn new() -> Self {
        Self {
            last_flags: 0,
            baseline_set: false,
            last_flag_spoken: None,
        }
    }
}

impl Default for FlagsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for FlagsRule {
    fn id(&self) -> &'static str {
        "flags"
    }

    fn before_tick(&mut self, ctx: &RaceContext<'_>, _settings: &AppSettings) {
        if !self.baseline_set {
            self.last_flags = ctx.snap.session_flags;
            self.baseline_set = true;
        }
    }

    fn on_tick(&mut self, ctx: &RaceContext<'_>, settings: &AppSettings, out: &mut Vec<Candidate>) {
        if !settings.audio_flags_enabled || !self.baseline_set {
            return;
        }
        if self
            .last_flag_spoken
            .map(|t| t.elapsed() < FLAG_DEBOUNCE)
            .unwrap_or(false)
        {
            return;
        }

        let cur = ctx.snap.session_flags;
        let newly_set = |mask: u32| (cur & mask) != 0 && (self.last_flags & mask) == 0;

        // Player-facing SessionFlags only: blue/black mean the client is shown that
        // flag (being lapped / personal penalty), not another car's state.
        let alert = if newly_set(flags::RED) {
            Some((SpeechPriority::CRITICAL, "flag_red"))
        } else if newly_set(flags::CHECKERED) {
            Some((SpeechPriority::CRITICAL, "flag_checkered"))
        } else if newly_set(flags::BLACK) {
            Some((SpeechPriority::CRITICAL, "flag_black"))
        } else if newly_set(flags::YELLOW_WAVING) {
            Some((SpeechPriority::SAFETY, "flag_yellow_waving"))
        } else if newly_set(flags::YELLOW) {
            Some((SpeechPriority::SAFETY, "flag_yellow"))
        } else if newly_set(flags::BLUE) {
            Some((SpeechPriority::SAFETY, "flag_blue"))
        } else if newly_set(flags::GREEN) || newly_set(flags::GREEN_HELD) {
            Some((SpeechPriority::SAFETY, "flag_green"))
        } else if newly_set(flags::WHITE) {
            Some((SpeechPriority::SAFETY, "flag_white"))
        } else {
            None
        };

        if let Some((priority, clip)) = alert {
            out.push(Candidate {
                priority,
                plan: wrap_with_radio(settings, SpeechPlan::clip(clip)),
                mark: Mark::Flags,
            });
        } else {
            // Advance the edge baseline when nothing is spoken so ignored bits
            // (debris, start lights, …) do not look like a perpetual rising edge.
            self.last_flags = cur;
        }
    }

    fn apply_mark(&mut self, ctx: &RaceContext<'_>, mark: &Mark) {
        if matches!(mark, Mark::Flags) {
            self.last_flags = ctx.snap.session_flags;
            self.last_flag_spoken = Some(Instant::now());
        }
    }

    fn on_session_reset(&mut self) {
        self.last_flags = 0;
        self.baseline_set = false;
        self.last_flag_spoken = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::context::{RaceContext, SessionMeta};
    use race_refinery_live::LiveSnapshot;

    fn ctx_with_flags(flags: u32) -> (LiveSnapshot, SessionMeta) {
        let snap = LiveSnapshot {
            track: "T".into(),
            session_type: "Race".into(),
            lap: 1,
            on_track: true,
            session_flags: flags,
            ..Default::default()
        };
        (snap, SessionMeta::default())
    }

    fn baseline(rule: &mut FlagsRule, settings: &AppSettings) {
        let fuel = Vec::<f32>::new();
        let (snap0, meta) = ctx_with_flags(0);
        let ctx0 = RaceContext::new(&snap0, &meta, &fuel);
        rule.before_tick(&ctx0, settings);
        rule.on_tick(&ctx0, settings, &mut Vec::new());
    }

    fn clip_key(plan: &SpeechPlan) -> Option<&str> {
        match plan {
            SpeechPlan::Clip(k) => Some(k.as_str()),
            SpeechPlan::Sequence(units) => units.iter().find_map(|u| match u {
                crate::speech::SpeechUnit::Clip(k) if k != "radio_beep" => Some(k.as_str()),
                _ => None,
            }),
        }
    }

    #[test]
    fn blue_flag_edge_triggers_after_baseline() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        let fuel = Vec::<f32>::new();

        let (snap0, meta) = ctx_with_flags(0);
        let ctx0 = RaceContext::new(&snap0, &meta, &fuel);
        rule.before_tick(&ctx0, &settings);
        let mut out = Vec::new();
        rule.on_tick(&ctx0, &settings, &mut out);
        assert!(out.is_empty(), "baseline should not speak");

        let (snap1, meta1) = ctx_with_flags(flags::BLUE);
        let ctx1 = RaceContext::new(&snap1, &meta1, &fuel);
        rule.before_tick(&ctx1, &settings);
        out.clear();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].mark, Mark::Flags));
        assert_eq!(out[0].priority, SpeechPriority::SAFETY);
        assert_eq!(clip_key(&out[0].plan), Some("flag_blue"));
    }

    #[test]
    fn blue_flag_does_not_repeat_while_held() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            audio_radio_effects_enabled: false,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        baseline(&mut rule, &settings);
        let fuel = Vec::<f32>::new();

        let (snap1, meta1) = ctx_with_flags(flags::BLUE);
        let ctx1 = RaceContext::new(&snap1, &meta1, &fuel);
        let mut out = Vec::new();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert_eq!(out.len(), 1);
        rule.apply_mark(&ctx1, &Mark::Flags);

        out.clear();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert!(out.is_empty(), "held blue must not spam");
    }

    #[test]
    fn debris_does_not_announce_black() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            audio_radio_effects_enabled: false,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        baseline(&mut rule, &settings);
        let fuel = Vec::<f32>::new();

        let (snap1, meta1) = ctx_with_flags(flags::DEBRIS);
        let ctx1 = RaceContext::new(&snap1, &meta1, &fuel);
        let mut out = Vec::new();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert!(out.is_empty(), "debris bit must not be treated as black");
    }

    #[test]
    fn real_black_flag_announces() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            audio_radio_effects_enabled: false,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        baseline(&mut rule, &settings);
        let fuel = Vec::<f32>::new();

        let (snap1, meta1) = ctx_with_flags(flags::BLACK);
        let ctx1 = RaceContext::new(&snap1, &meta1, &fuel);
        let mut out = Vec::new();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].priority, SpeechPriority::CRITICAL);
        assert_eq!(clip_key(&out[0].plan), Some("flag_black"));
    }

    #[test]
    fn debris_then_blue_still_announces_blue() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            audio_radio_effects_enabled: false,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        baseline(&mut rule, &settings);
        let fuel = Vec::<f32>::new();

        let (snap_d, meta_d) = ctx_with_flags(flags::DEBRIS);
        let ctx_d = RaceContext::new(&snap_d, &meta_d, &fuel);
        let mut out = Vec::new();
        rule.on_tick(&ctx_d, &settings, &mut out);
        assert!(out.is_empty());

        let (snap_b, meta_b) = ctx_with_flags(flags::DEBRIS | flags::BLUE);
        let ctx_b = RaceContext::new(&snap_b, &meta_b, &fuel);
        rule.on_tick(&ctx_b, &settings, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(clip_key(&out[0].plan), Some("flag_blue"));
    }

    #[test]
    fn red_beats_yellow_on_same_tick() {
        let settings = AppSettings {
            audio_flags_enabled: true,
            ..AppSettings::default()
        };
        let mut rule = FlagsRule::new();
        let fuel = Vec::<f32>::new();
        let (snap0, meta) = ctx_with_flags(0);
        let ctx0 = RaceContext::new(&snap0, &meta, &fuel);
        rule.before_tick(&ctx0, &settings);
        rule.on_tick(&ctx0, &settings, &mut Vec::new());

        let (snap1, meta1) = ctx_with_flags(flags::RED | flags::YELLOW);
        let ctx1 = RaceContext::new(&snap1, &meta1, &fuel);
        let mut out = Vec::new();
        rule.on_tick(&ctx1, &settings, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].priority, SpeechPriority::CRITICAL);
    }
}
