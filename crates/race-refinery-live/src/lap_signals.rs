//! Lightweight lap signals for live coach validity (no stored LapKind taxonomy).

const MIN_LAP_MAX_PCT: f32 = 0.95;
const MIN_LAP_MAX_PCT_FALLBACK: f32 = 0.90;
const MIN_LAP_SPAN_FALLBACK: f32 = 0.85;
const PIT_LANE_RATIO: f64 = 0.85;
const PIT_MEANINGFUL_RATIO: f64 = 0.15;

/// Whether the car reached the start/finish line on this lap.
pub fn lap_completed(min_pct: f32, max_pct: f32) -> bool {
    if !min_pct.is_finite() || !max_pct.is_finite() {
        return false;
    }
    if min_pct < 0.1 && max_pct > 0.9 {
        return true;
    }
    if max_pct >= MIN_LAP_MAX_PCT {
        return true;
    }
    let span = max_pct - min_pct;
    max_pct >= MIN_LAP_MAX_PCT_FALLBACK && span >= MIN_LAP_SPAN_FALLBACK
}

/// True when the finished lap looks like a flying (non-pit) lap.
pub fn is_flying_lap(
    pit_ratio: f64,
    completed: bool,
    start_on_pit: bool,
    end_on_pit: bool,
) -> bool {
    if pit_ratio > PIT_LANE_RATIO
        || (start_on_pit && end_on_pit && pit_ratio > PIT_MEANINGFUL_RATIO)
    {
        return false;
    }
    if start_on_pit || end_on_pit {
        return false;
    }
    completed
}

/// Live coach `lastLapValid`: flying, completed, and iRacing OK flags.
pub fn include_in_stats_live(flying: bool, completed: bool, iracing_ok: bool) -> bool {
    flying && completed && iracing_ok
}
