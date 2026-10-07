use pitwall::SessionInfo;
use serde::{Deserialize, Serialize};

use super::car_idx_frame::CarIdxFrame;

/// Static per-driver info pulled from the session YAML roster, cached so the
/// leaderboard can be rebuilt on every telemetry frame without re-parsing YAML.
#[derive(Debug, Clone, Default)]
pub struct RosterEntry {
    pub car_idx: i32,
    pub driver_name: String,
    pub car_number: String,
    pub class_id: i32,
    pub class_color: String,
}

/// One row of the live leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompetitorEntry {
    pub car_idx: i32,
    pub driver_name: String,
    pub car_number: String,
    pub class_id: i32,
    pub class_color: String,
    pub position: i32,
    pub class_position: i32,
    pub best_lap_ms: Option<f64>,
    pub last_lap_ms: Option<f64>,
    pub on_pit_road: bool,
    pub is_player: bool,
    /// Position around the lap (0..1), used by the VR relative/radar overlays.
    pub lap_dist_pct: f32,
    /// Signed time gap derived from F2: `player_f2 - other_f2`.
    /// Positive = other is ahead of the player; negative = other is behind.
    pub gap_to_player_s: Option<f32>,
}

/// Result of merging the session roster with a live `CarIdxFrame`.
#[derive(Debug, Clone, Default)]
pub struct CompetitorSnapshot {
    pub competitors: Vec<CompetitorEntry>,
    pub player_position: Option<i32>,
    pub player_class_position: Option<i32>,
    pub session_fastest_lap_ms: Option<f64>,
    pub gap_to_car_ahead_s: Option<f32>,
    pub gap_to_car_behind_s: Option<f32>,
}

/// Extract the driver roster from the session YAML, skipping the pace car and
/// spectators so they never appear on the leaderboard.
pub fn build_roster(session: &SessionInfo) -> Vec<RosterEntry> {
    let Some(driver_info) = &session.driver_info else {
        return Vec::new();
    };
    let pace_car_idx = driver_info.pace_car_idx.unwrap_or(-1);
    let Some(drivers) = &driver_info.drivers else {
        return Vec::new();
    };

    drivers
        .iter()
        .filter(|d| d.car_idx != pace_car_idx)
        .filter(|d| d.car_is_pace_car.unwrap_or(0) == 0)
        .filter(|d| d.is_spectator.unwrap_or(0) == 0)
        .map(|d| RosterEntry {
            car_idx: d.car_idx,
            driver_name: d.user_name.clone(),
            car_number: d.car_number.clone().unwrap_or_default(),
            class_id: d.car_class_id.unwrap_or(0),
            class_color: d.car_class_color.clone().unwrap_or_default(),
        })
        .collect()
}

fn array_get<T: Copy>(arr: &[T], idx: i32) -> Option<T> {
    if idx < 0 {
        return None;
    }
    arr.get(idx as usize).copied()
}

/// iRacing reports lap times in seconds, using a negative sentinel when no lap
/// has been set yet. Convert to milliseconds, dropping the sentinel.
pub fn lap_seconds_to_ms(secs: Option<f32>) -> Option<f64> {
    match secs {
        Some(s) if s > 0.0 => Some((s as f64 * 1000.0).floor()),
        _ => None,
    }
}

pub fn build(
    roster: &[RosterEntry],
    player_car_idx: i32,
    frame: &CarIdxFrame,
) -> CompetitorSnapshot {
    let player_f2 = array_get(&frame.f2_time, player_car_idx).filter(|t| *t >= 0.0);
    let mut competitors: Vec<CompetitorEntry> = roster
        .iter()
        .map(|r| {
            let position = array_get(&frame.position, r.car_idx).unwrap_or(0);
            let class_position = array_get(&frame.class_position, r.car_idx).unwrap_or(0);
            let other_f2 = array_get(&frame.f2_time, r.car_idx).filter(|t| *t >= 0.0);
            let gap_to_player_s = match (player_f2, other_f2) {
                (Some(p), Some(o)) if r.car_idx != player_car_idx => Some(p - o),
                _ => None,
            };
            CompetitorEntry {
                car_idx: r.car_idx,
                driver_name: r.driver_name.clone(),
                car_number: r.car_number.clone(),
                class_id: r.class_id,
                class_color: r.class_color.clone(),
                position,
                class_position,
                best_lap_ms: lap_seconds_to_ms(array_get(&frame.car_idx_best_lap_time, r.car_idx)),
                last_lap_ms: lap_seconds_to_ms(array_get(&frame.car_idx_last_lap_time, r.car_idx)),
                on_pit_road: array_get(&frame.on_pit_road, r.car_idx).unwrap_or(false),
                is_player: r.car_idx == player_car_idx,
                lap_dist_pct: array_get(&frame.lap_dist_pct, r.car_idx).unwrap_or(0.0),
                gap_to_player_s,
            }
        })
        .collect();

    // Overall leaderboard order: positioned cars first (ascending), then any
    // car still without a position (pre-grid) by car number order.
    competitors.sort_by(|a, b| match (a.position > 0, b.position > 0) {
        (true, true) => a.position.cmp(&b.position),
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => a.car_idx.cmp(&b.car_idx),
    });

    let session_fastest_lap_ms =
        lap_seconds_to_ms(Some(frame.session_best_lap_time)).or_else(|| {
            competitors
                .iter()
                .filter_map(|c| c.best_lap_ms)
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        });

    let player = competitors.iter().find(|c| c.is_player);
    let player_position = player.map(|p| p.position).filter(|p| *p > 0);
    let player_class_position = player.map(|p| p.class_position).filter(|p| *p > 0);

    let (gap_to_car_ahead_s, gap_to_car_behind_s) =
        compute_gaps(&competitors, player_car_idx, &frame.f2_time);

    CompetitorSnapshot {
        competitors,
        player_position,
        player_class_position,
        session_fastest_lap_ms,
        gap_to_car_ahead_s,
        gap_to_car_behind_s,
    }
}

/// Gaps to the cars directly ahead and behind in overall order, derived from the
/// difference in `CarIdxF2Time` (time behind the leader) between adjacent cars.
fn compute_gaps(
    competitors: &[CompetitorEntry],
    player_car_idx: i32,
    f2_time: &[f32],
) -> (Option<f32>, Option<f32>) {
    let positioned: Vec<&CompetitorEntry> = competitors.iter().filter(|c| c.position > 0).collect();
    let Some(player_pos) = positioned.iter().position(|c| c.car_idx == player_car_idx) else {
        return (None, None);
    };

    // `CarIdxF2Time` is the time behind the leader; the leader's own value is a
    // legitimate 0.0, so anything >= 0 counts as present.
    let f2_of = |entry: &CompetitorEntry| -> Option<f32> {
        array_get(f2_time, entry.car_idx).filter(|t| *t >= 0.0)
    };
    let player_f2 = f2_of(positioned[player_pos]);

    let ahead = player_pos
        .checked_sub(1)
        .and_then(|i| positioned.get(i))
        .and_then(|c| f2_of(c));
    let behind = positioned.get(player_pos + 1).and_then(|c| f2_of(c));

    let gap_ahead = match (player_f2, ahead) {
        (Some(p), Some(a)) => Some((p - a).abs()),
        _ => None,
    };
    let gap_behind = match (behind, player_f2) {
        (Some(b), Some(p)) => Some((b - p).abs()),
        _ => None,
    };
    (gap_ahead, gap_behind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Vec<RosterEntry> {
        vec![
            RosterEntry {
                car_idx: 0,
                driver_name: "You".into(),
                car_number: "1".into(),
                class_id: 10,
                class_color: String::new(),
            },
            RosterEntry {
                car_idx: 1,
                driver_name: "Ahead".into(),
                car_number: "2".into(),
                class_id: 10,
                class_color: String::new(),
            },
            RosterEntry {
                car_idx: 2,
                driver_name: "Behind".into(),
                car_number: "3".into(),
                class_id: 10,
                class_color: String::new(),
            },
        ]
    }

    fn frame() -> CarIdxFrame {
        CarIdxFrame {
            player_car_idx: 0,
            car_idx_best_lap_time: vec![91.0, 90.0, 92.0],
            car_idx_last_lap_time: vec![91.5, 90.5, 92.5],
            position: vec![2, 1, 3],
            class_position: vec![2, 1, 3],
            on_pit_road: vec![false, false, true],
            f2_time: vec![1.5, 0.0, 3.0],
            lap_dist_pct: vec![0.10, 0.12, 0.05],
            current_lap_time: 45.0,
            player_last_lap_time: 91.5,
            player_best_lap_time: 91.0,
            session_best_lap_time: 90.0,
            delta_best: 0.0,
            delta_best_ok: true,
            delta_last: 0.0,
            delta_last_ok: false,
            delta_session_best: 0.0,
            delta_session_best_ok: false,
            delta_session_optimal: 0.0,
            delta_session_optimal_ok: false,
            session_flags: None,
            car_left_right: 0,
            incident_count: 0,
            session_time_remain: 0.0,
            session_laps_remain: 0,
            pits_open: false,
            on_track: true,
        }
    }

    #[test]
    fn sorts_by_overall_position() {
        let snap = build(&roster(), 0, &frame());
        assert_eq!(snap.competitors[0].driver_name, "Ahead");
        assert_eq!(snap.competitors[1].driver_name, "You");
        assert_eq!(snap.competitors[2].driver_name, "Behind");
    }

    #[test]
    fn player_positions_and_fastest() {
        let snap = build(&roster(), 0, &frame());
        assert_eq!(snap.player_position, Some(2));
        assert_eq!(snap.player_class_position, Some(2));
        assert_eq!(snap.session_fastest_lap_ms, Some(90_000.0));
    }

    #[test]
    fn gaps_from_f2_time_difference() {
        let snap = build(&roster(), 0, &frame());
        // Player F2 = 1.5, ahead = 0.0, behind = 3.0.
        assert_eq!(snap.gap_to_car_ahead_s, Some(1.5));
        assert_eq!(snap.gap_to_car_behind_s, Some(1.5));
    }

    #[test]
    fn best_lap_sentinel_dropped() {
        let mut f = frame();
        f.car_idx_best_lap_time = vec![-1.0, 90.0, -1.0];
        let snap = build(&roster(), 0, &f);
        let you = snap.competitors.iter().find(|c| c.is_player).unwrap();
        assert_eq!(you.best_lap_ms, None);
    }

    #[test]
    fn gaps_none_when_player_missing_from_order() {
        let mut f = frame();
        // Positions omit the player car_idx 0.
        f.position = vec![0, 1, 2];
        let snap = build(&roster(), 0, &f);
        assert_eq!(snap.gap_to_car_ahead_s, None);
        assert_eq!(snap.gap_to_car_behind_s, None);
    }

    #[test]
    fn gaps_leader_has_no_car_ahead() {
        let mut f = frame();
        // Make player (car 0) P1 with F2=0.
        f.position = vec![1, 2, 3];
        f.f2_time = vec![0.0, 1.2, 2.5];
        let snap = build(&roster(), 0, &f);
        assert_eq!(snap.player_position, Some(1));
        assert_eq!(snap.gap_to_car_ahead_s, None);
        assert_eq!(snap.gap_to_car_behind_s, Some(1.2));
    }

    #[test]
    fn gap_to_player_signed_relative_to_player_f2() {
        let snap = build(&roster(), 0, &frame());
        let ahead = snap
            .competitors
            .iter()
            .find(|c| c.driver_name == "Ahead")
            .unwrap();
        let behind = snap
            .competitors
            .iter()
            .find(|c| c.driver_name == "Behind")
            .unwrap();
        // gap = player_f2 - other_f2: positive when other is ahead (lower F2).
        assert!(ahead.gap_to_player_s.unwrap() > 0.0);
        assert!(behind.gap_to_player_s.unwrap() < 0.0);
    }
}
