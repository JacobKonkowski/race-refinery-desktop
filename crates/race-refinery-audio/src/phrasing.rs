//! Racing-radio phrasing for live numbers, composed from voice pack clips
//! (`n0`-`n20`, round tens, `point`, `minute`, ...) so every number is spoken in
//! the same recorded voice as the fixed callouts.

use super::speech::SpeechUnit;

/// Break between a lap or sector number and its time, so "lap 20, 1 minute" is
/// not heard as "lap twenty-one minute".
const NUMBER_BREAK_MS: u32 = 180;

fn clip(units: &mut Vec<SpeechUnit>, key: impl Into<String>) {
    units.push(SpeechUnit::Clip(key.into()));
}

fn digit(units: &mut Vec<SpeechUnit>, d: u64) {
    clip(units, format!("n{d}"));
}

/// 0-20 and round tens are one clip, 21-99 are tens + ones (29 = `n20` `n9`),
/// 100 and up are read digit by digit.
pub fn push_uint(units: &mut Vec<SpeechUnit>, n: u32) {
    if n >= 100 {
        for d in n.to_string().bytes() {
            digit(units, u64::from(d - b'0'));
        }
    } else if n <= 20 || n.is_multiple_of(10) {
        clip(units, format!("n{n}"));
    } else {
        clip(units, format!("n{}", n / 10 * 10));
        clip(units, format!("n{}", n % 10));
    }
}

/// Lap / sector time to the tenth: 89.45 s -> "1 minute 29 point 5",
/// 65.3 s -> "1 oh 5 point 3", 42.34 s -> "42 point 3".
pub fn push_lap_time(units: &mut Vec<SpeechUnit>, ms: f64) {
    let total = (ms / 100.0).round().max(0.0) as u64;
    let (min, rem) = (total / 600, total % 600);
    let (sec, tenth) = (rem / 10, rem % 10);
    if min > 0 {
        push_uint(units, min as u32);
        if sec < 10 {
            clip(units, "oh");
        } else {
            clip(units, if min == 1 { "minute" } else { "minutes" });
        }
    }
    push_uint(units, sec as u32);
    clip(units, "point");
    digit(units, tenth);
}

/// Follows the `lap` clip: "12, 1 minute 29 point 5".
pub fn push_lap_time_callout(units: &mut Vec<SpeechUnit>, lap_num: i32, lap_ms: f64) {
    push_uint(units, lap_num.max(0) as u32);
    units.push(SpeechUnit::Pause(NUMBER_BREAK_MS));
    push_lap_time(units, lap_ms);
}

/// Follows the `sector` clip: "2, 31 point 4".
pub fn push_sector_time_callout(units: &mut Vec<SpeechUnit>, sector_num: i32, ms: f64) {
    push_uint(units, sector_num.max(0) as u32);
    units.push(SpeechUnit::Pause(NUMBER_BREAK_MS));
    push_lap_time(units, ms);
}

/// "1 second", "2 seconds", "1 point 4 seconds" (rounded to tenths).
pub fn push_seconds(units: &mut Vec<SpeechUnit>, seconds: f64) {
    let tenths = (seconds.abs() * 10.0).round() as u32;
    let (whole, frac) = (tenths / 10, tenths % 10);
    push_uint(units, whole);
    if frac == 0 {
        clip(units, if whole == 1 { "second" } else { "seconds" });
    } else {
        clip(units, "point");
        digit(units, u64::from(frac));
        clip(units, "seconds");
    }
}

/// Tenths under a second, seconds above, then faster / slower. Never says
/// "0 tenths": anything under a tenth rounds up to one.
pub fn push_delta(units: &mut Vec<SpeechUnit>, delta_ms: f64) {
    let tenths = (delta_ms.abs() / 100.0).round().max(1.0) as u32;
    if tenths < 10 {
        push_uint(units, tenths);
        clip(units, if tenths == 1 { "tenth" } else { "tenths" });
    } else {
        push_seconds(units, delta_ms / 1000.0);
    }
    clip(units, if delta_ms > 0.0 { "slower" } else { "faster" });
}

pub fn push_gap_seconds(units: &mut Vec<SpeechUnit>, gap_s: f32) {
    push_seconds(units, f64::from(gap_s));
}

/// "Position" + number; shown as "P3" in the UI.
pub fn push_position(units: &mut Vec<SpeechUnit>, pos: i32) {
    clip(units, "position");
    push_uint(units, pos.max(0) as u32);
}

/// Whole liters: "12 liters", "1 liter".
pub fn push_liters(units: &mut Vec<SpeechUnit>, liters: f32) {
    let n = liters.max(0.0).round() as u32;
    push_uint(units, n);
    clip(units, if n == 1 { "liter" } else { "liters" });
}

/// Fuel range: "pit this lap or next" when nearly dry, else "about 6 laps of fuel".
pub fn push_laps_of_fuel(units: &mut Vec<SpeechUnit>, laps_left: f32) {
    if laps_left <= 1.5 {
        clip(units, "fuel_pit_this_lap");
        return;
    }
    clip(units, "about");
    push_uint(units, laps_left.round() as u32);
    clip(units, "laps");
    clip(units, "of_fuel");
}

/// "Fuel 12 liters, about 6 laps of fuel" (the range only when it is known).
pub fn push_fuel_status(units: &mut Vec<SpeechUnit>, liters: f32, laps_left: Option<f32>) {
    clip(units, "fuel");
    push_liters(units, liters);
    if let Some(laps) = laps_left {
        push_laps_of_fuel(units, laps);
    }
}

/// "About 2 laps short" (the singular uses the `lap` callout clip).
pub fn push_laps_short(units: &mut Vec<SpeechUnit>, laps: u32) {
    clip(units, "about");
    push_uint(units, laps);
    clip(units, if laps == 1 { "lap" } else { "laps" });
    clip(units, "short");
}

/// "Incident 12", plus "limit is 17" once the count is within two of the limit.
pub fn push_incidents(units: &mut Vec<SpeechUnit>, count: u32, limit: Option<u32>) {
    clip(units, "incident_intro");
    push_uint(units, count);
    if let Some(limit) = limit.filter(|l| *l > 0 && count >= l.saturating_sub(2)) {
        clip(units, "limit");
        push_uint(units, limit);
    }
}

/// Value of a number clip (`n29` is never emitted, but `n20` and `n9` are).
pub fn number_clip_value(key: &str) -> Option<u32> {
    key.strip_prefix('n')?.parse().ok()
}

/// How a word clip reads in the UI / logs; `None` for callout clips shown as `[key]`.
pub fn clip_label(key: &str) -> Option<&'static str> {
    Some(match key {
        "minute" => "minute",
        "minutes" => "minutes",
        "oh" => "oh",
        "second" => "second",
        "seconds" => "seconds",
        "tenth" => "tenth",
        "tenths" => "tenths",
        "faster" => "faster.",
        "slower" => "slower.",
        "position" => "P",
        "out_lap" => "out lap.",
        "versus_previous_lap" => "versus previous lap.",
        "off_session_best" => "off session best.",
        "fuel" => "fuel",
        "liter" => "liter",
        "liters" => "liters",
        "laps" => "laps",
        "about" => "about",
        "short" => "short.",
        "of_fuel" => "of fuel.",
        "limit" => "limit",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(build: impl FnOnce(&mut Vec<SpeechUnit>)) -> Vec<String> {
        let mut units = Vec::new();
        build(&mut units);
        units
            .into_iter()
            .map(|u| match u {
                SpeechUnit::Clip(k) => k,
                SpeechUnit::Pause(_) => ",".into(),
            })
            .collect()
    }

    #[test]
    fn uint_composition() {
        assert_eq!(keys(|u| push_uint(u, 0)), ["n0"]);
        assert_eq!(keys(|u| push_uint(u, 7)), ["n7"]);
        assert_eq!(keys(|u| push_uint(u, 13)), ["n13"]);
        assert_eq!(keys(|u| push_uint(u, 20)), ["n20"]);
        assert_eq!(keys(|u| push_uint(u, 21)), ["n20", "n1"]);
        assert_eq!(keys(|u| push_uint(u, 29)), ["n20", "n9"]);
        assert_eq!(keys(|u| push_uint(u, 40)), ["n40"]);
        assert_eq!(keys(|u| push_uint(u, 59)), ["n50", "n9"]);
        assert_eq!(keys(|u| push_uint(u, 60)), ["n60"]);
        assert_eq!(keys(|u| push_uint(u, 75)), ["n70", "n5"]);
        assert_eq!(keys(|u| push_uint(u, 99)), ["n90", "n9"]);
        assert_eq!(keys(|u| push_uint(u, 105)), ["n1", "n0", "n5"]);
    }

    #[test]
    fn lap_time_cadence() {
        assert_eq!(
            keys(|u| push_lap_time(u, 89_843.0)),
            ["n1", "minute", "n20", "n9", "point", "n8"]
        );
        assert_eq!(
            keys(|u| push_lap_time(u, 65_300.0)),
            ["n1", "oh", "n5", "point", "n3"]
        );
        assert_eq!(
            keys(|u| push_lap_time(u, 42_340.0)),
            ["n40", "n2", "point", "n3"]
        );
        assert_eq!(
            keys(|u| push_lap_time(u, 135_010.0)),
            ["n2", "minutes", "n15", "point", "n0"]
        );
    }

    #[test]
    fn lap_time_rounding_carries_into_minute() {
        assert_eq!(
            keys(|u| push_lap_time(u, 119_970.0)),
            ["n2", "oh", "n0", "point", "n0"]
        );
        assert_eq!(
            keys(|u| push_lap_time(u, 89_960.0)),
            ["n1", "minute", "n30", "point", "n0"]
        );
    }

    #[test]
    fn lap_and_sector_callouts_pause_after_number() {
        assert_eq!(
            keys(|u| push_lap_time_callout(u, 12, 89_452.0)),
            ["n12", ",", "n1", "minute", "n20", "n9", "point", "n5"]
        );
        assert_eq!(
            keys(|u| push_sector_time_callout(u, 2, 31_420.0)),
            ["n2", ",", "n30", "n1", "point", "n4"]
        );
    }

    #[test]
    fn delta_tenths() {
        assert_eq!(keys(|u| push_delta(u, 350.0)), ["n4", "tenths", "slower"]);
        assert_eq!(keys(|u| push_delta(u, -280.0)), ["n3", "tenths", "faster"]);
    }

    #[test]
    fn delta_never_says_zero_tenths() {
        assert_eq!(keys(|u| push_delta(u, 60.0)), ["n1", "tenth", "slower"]);
        assert_eq!(keys(|u| push_delta(u, -120.0)), ["n1", "tenth", "faster"]);
    }

    #[test]
    fn delta_switches_to_seconds() {
        assert_eq!(keys(|u| push_delta(u, 960.0)), ["n1", "second", "slower"]);
        assert_eq!(
            keys(|u| push_delta(u, -1_440.0)),
            ["n1", "point", "n4", "seconds", "faster"]
        );
    }

    #[test]
    fn gaps_drop_trailing_zero() {
        assert_eq!(keys(|u| push_gap_seconds(u, 1.04)), ["n1", "second"]);
        assert_eq!(keys(|u| push_gap_seconds(u, 3.0)), ["n3", "seconds"]);
        assert_eq!(
            keys(|u| push_gap_seconds(u, 2.36)),
            ["n2", "point", "n4", "seconds"]
        );
    }

    #[test]
    fn position() {
        assert_eq!(keys(|u| push_position(u, 3)), ["position", "n3"]);
        assert_eq!(keys(|u| push_position(u, 24)), ["position", "n20", "n4"]);
    }

    #[test]
    fn fuel_uses_whole_numbers() {
        assert_eq!(
            keys(|u| push_fuel_status(u, 12.4, Some(5.6))),
            ["fuel", "n12", "liters", "about", "n6", "laps", "of_fuel"]
        );
        assert_eq!(
            keys(|u| push_fuel_status(u, 1.2, Some(1.1))),
            ["fuel", "n1", "liter", "fuel_pit_this_lap"]
        );
        assert_eq!(
            keys(|u| push_fuel_status(u, 30.0, None)),
            ["fuel", "n30", "liters"]
        );
        assert_eq!(
            keys(|u| push_laps_short(u, 2)),
            ["about", "n2", "laps", "short"]
        );
        assert_eq!(
            keys(|u| push_laps_short(u, 1)),
            ["about", "n1", "lap", "short"]
        );
    }

    #[test]
    fn incident_limit_only_near_the_limit() {
        assert_eq!(
            keys(|u| push_incidents(u, 4, Some(17))),
            ["incident_intro", "n4"]
        );
        assert_eq!(
            keys(|u| push_incidents(u, 15, Some(17))),
            ["incident_intro", "n15", "limit", "n17"]
        );
        assert_eq!(
            keys(|u| push_incidents(u, 15, Some(0))),
            ["incident_intro", "n15"]
        );
    }

    /// Every clip the phrasing helpers can emit is a registry phrase, so packs
    /// can record it.
    #[test]
    fn every_emitted_clip_has_a_phrase() {
        let mut units = Vec::new();
        for n in 0..1000 {
            push_uint(&mut units, n);
        }
        for ms in [0.0, 9_999.0, 65_300.0, 89_843.0, 135_010.0, 600_000.0] {
            push_lap_time(&mut units, ms);
        }
        for d in [-60.0, 60.0, 350.0, -1_440.0, 960.0] {
            push_delta(&mut units, d);
        }
        push_seconds(&mut units, 1.0);
        push_position(&mut units, 3);
        push_fuel_status(&mut units, 1.0, Some(1.0));
        push_fuel_status(&mut units, 12.0, Some(6.0));
        push_laps_short(&mut units, 1);
        push_laps_short(&mut units, 3);
        push_incidents(&mut units, 16, Some(17));
        for unit in units {
            if let SpeechUnit::Clip(key) = unit {
                assert!(
                    crate::phrases::phrase(&key).is_some(),
                    "missing phrase for clip '{key}'"
                );
            }
        }
    }
}
