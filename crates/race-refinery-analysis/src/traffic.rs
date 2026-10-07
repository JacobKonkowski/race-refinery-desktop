//! Sparse traffic tagging from multi-car lap-distance samples.

use super::types::TrafficEvent;

/// How close (as a fraction of the lap) another car must be to count as traffic.
pub const NEARBY_PCT: f32 = 0.015;
/// Merge hits closer than this into one event.
const MERGE_PCT: f64 = 0.02;

/// One sparse sample of the field relative to the player.
#[derive(Debug, Clone)]
pub struct TrafficSample {
    pub session_num: i32,
    pub lap: i32,
    pub dist_pct: f32,
    /// True when at least one other on-track car was within [`NEARBY_PCT`].
    pub nearby: bool,
}

/// Collapse nearby samples into sparse [`TrafficEvent`]s for one lap.
pub fn events_for_lap(
    samples: &[TrafficSample],
    session_num: i32,
    iracing_lap: i32,
) -> Vec<TrafficEvent> {
    let mut hits: Vec<f64> = samples
        .iter()
        .filter(|s| s.session_num == session_num && s.lap == iracing_lap && s.nearby)
        .map(|s| s.dist_pct as f64)
        .collect();
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut events = Vec::new();
    for pct in hits {
        if let Some(last) = events.last() {
            let TrafficEvent { dist_pct, .. } = last;
            if (pct - dist_pct).abs() < MERGE_PCT {
                continue;
            }
        }
        events.push(TrafficEvent {
            dist_pct: pct,
            kind: "nearby".into(),
        });
    }
    events
}

/// Circular lap-distance gap in [0, 0.5].
pub fn lap_gap(a: f32, b: f32) -> f32 {
    let mut d = (a - b).abs();
    if d > 0.5 {
        d = 1.0 - d;
    }
    d
}

/// Whether any other car (not in pits, valid dist) is within [`NEARBY_PCT`] of the player.
pub fn field_nearby(player_idx: i32, player_pct: f32, field_pct: &[f32], on_pit: &[bool]) -> bool {
    if !(0.0..1.0).contains(&player_pct) {
        return false;
    }
    for (i, &pct) in field_pct.iter().enumerate() {
        if i as i32 == player_idx {
            continue;
        }
        if on_pit.get(i).copied().unwrap_or(false) {
            continue;
        }
        if !(0.0..1.0).contains(&pct) {
            continue;
        }
        if lap_gap(player_pct, pct) <= NEARBY_PCT {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_close_hits() {
        let samples = vec![
            TrafficSample {
                session_num: 0,
                lap: 2,
                dist_pct: 0.20,
                nearby: true,
            },
            TrafficSample {
                session_num: 0,
                lap: 2,
                dist_pct: 0.21,
                nearby: true,
            },
            TrafficSample {
                session_num: 0,
                lap: 2,
                dist_pct: 0.50,
                nearby: true,
            },
        ];
        let events = events_for_lap(&samples, 0, 2);
        assert_eq!(events.len(), 2);
        assert!((events[0].dist_pct - 0.20).abs() < 1e-6);
        assert!((events[1].dist_pct - 0.50).abs() < 1e-6);
    }

    #[test]
    fn detects_nearby_car() {
        let field = vec![0.10f32, 0.11, 0.80];
        let pits = vec![false, false, false];
        assert!(field_nearby(0, 0.10, &field, &pits));
        assert!(!field_nearby(0, 0.10, &[0.10, 0.40, 0.70], &pits));
    }
}
