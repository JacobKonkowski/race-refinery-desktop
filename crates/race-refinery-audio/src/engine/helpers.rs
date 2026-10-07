use race_refinery_live::PackState;
use race_refinery_settings::{AppSettings, ChatterLevel};

use super::super::phrasing::push_delta;
use super::super::speech::{SpeechPlan, SpeechUnit};

pub fn pack_clear_clip(prev_pack: PackState) -> &'static str {
    match prev_pack {
        PackState::CarLeft | PackState::TwoCarsLeft => "pack_clear_left",
        PackState::CarRight | PackState::TwoCarsRight => "pack_clear_right",
        _ => "pack_clear",
    }
}

pub fn push_pace_delta_units(units: &mut Vec<SpeechUnit>, delta_ms: f64) {
    if delta_ms.abs() < 50.0 {
        units.push(SpeechUnit::Clip("pace_matching_best".into()));
        return;
    }
    if delta_ms > 0.0 {
        units.push(SpeechUnit::Clip("pace_off_pb_intro".into()));
    }
    push_delta(units, delta_ms);
}

/// Delta followed by the `suffix_clip` naming what it is relative to
/// (`versus_previous_lap`, `off_session_best`).
pub fn push_pace_delta_with_suffix(units: &mut Vec<SpeechUnit>, delta_ms: f64, suffix_clip: &str) {
    push_pace_delta_units(units, delta_ms);
    units.push(SpeechUnit::Clip(suffix_clip.into()));
}

pub fn estimate_laps_remaining(fuel_level: f32, fuel_per_lap: &[f32]) -> Option<f32> {
    if fuel_per_lap.is_empty() || fuel_level <= 0.0 {
        return None;
    }
    let avg: f32 = fuel_per_lap.iter().sum::<f32>() / fuel_per_lap.len() as f32;
    if avg < 0.05 {
        return None;
    }
    Some(fuel_level / avg)
}

pub fn chatter_allows_normal(settings: &AppSettings) -> bool {
    settings.audio_coach_chatter_level != ChatterLevel::Minimal
}

pub fn chatter_is_verbose(settings: &AppSettings) -> bool {
    settings.audio_coach_chatter_level == ChatterLevel::Verbose
}

pub fn chatter_allows_verbose(settings: &AppSettings) -> bool {
    chatter_allows_normal(settings)
}

/// Scale cooldown durations by chatter level.
pub fn chatter_cooldown_multiplier(settings: &AppSettings) -> f64 {
    match settings.audio_coach_chatter_level {
        ChatterLevel::Minimal => 1.5,
        ChatterLevel::Normal => 1.0,
        ChatterLevel::Verbose => 0.75,
    }
}

pub fn maybe_radio_prefix(settings: &AppSettings, units: &mut Vec<SpeechUnit>) {
    if settings.audio_radio_effects_enabled {
        units.insert(0, SpeechUnit::Clip("radio_beep".into()));
    }
}

pub fn wrap_with_radio(settings: &AppSettings, plan: SpeechPlan) -> SpeechPlan {
    if !settings.audio_radio_effects_enabled {
        return plan;
    }
    match plan {
        SpeechPlan::Clip(key) => SpeechPlan::sequence(vec![
            SpeechUnit::Clip("radio_beep".into()),
            SpeechUnit::Clip(key),
        ]),
        SpeechPlan::Sequence(mut units) => {
            maybe_radio_prefix(settings, &mut units);
            SpeechPlan::Sequence(units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_clear_clip_matches_prior_side() {
        assert_eq!(pack_clear_clip(PackState::CarLeft), "pack_clear_left");
        assert_eq!(pack_clear_clip(PackState::TwoCarsRight), "pack_clear_right");
        assert_eq!(pack_clear_clip(PackState::ThreeWide), "pack_clear");
        assert_eq!(pack_clear_clip(PackState::Clear), "pack_clear");
    }
}
