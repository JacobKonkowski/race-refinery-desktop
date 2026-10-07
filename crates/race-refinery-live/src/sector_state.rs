use pitwall::SessionInfo;
use tracing::debug;

use race_refinery_telemetry::SectorBoundary;

/// Minimum sector duration; matches live audio coach filter.
pub const MIN_SECTOR_MS: f64 = 1000.0;

const SF_WRAP_PREV_MIN: f32 = 0.9;
const SF_WRAP_CURR_MAX: f32 = 0.1;
const PCT_REGRESSION_EPS: f32 = 0.05;
/// Ignore finish-line markers at or near 100%.
const FINISH_MAX_PCT: f64 = 0.999;
const PCT_DEDUPE_EPS: f64 = 0.001;

/// Sorted region start positions (includes 0% when sector data exists).
///
/// iRacing `SplitTimeInfo.Sectors[].SectorStartPct` marks where each sector
/// **begins**. Sector 0 at 0% is the start of the first timed region.
pub fn region_starts(boundaries: &[SectorBoundary]) -> Vec<f64> {
    let mut starts: Vec<f64> = boundaries
        .iter()
        .map(|b| b.start_pct)
        .filter(|&pct| pct < FINISH_MAX_PCT)
        .collect();

    if starts.is_empty() {
        return Vec::new();
    }

    starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    starts.dedup_by(|a, b| (*a - *b).abs() < PCT_DEDUPE_EPS);

    if starts[0] > PCT_DEDUPE_EPS {
        starts.insert(0, 0.0);
    }

    starts
}

/// Split lines used for crossing detection: region starts after 0% (interior boundaries).
pub fn normalize_sector_boundaries(boundaries: &[SectorBoundary]) -> Vec<SectorBoundary> {
    let regions = region_starts(boundaries);
    if regions.len() < 2 {
        return Vec::new();
    }

    regions
        .iter()
        .skip(1)
        .enumerate()
        .map(|(idx, &start_pct)| SectorBoundary {
            sector_num: (idx + 1) as i32,
            start_pct,
        })
        .collect()
}

/// Number of timed sectors (S1..SN) for the track layout.
pub fn sector_count(boundaries: &[SectorBoundary]) -> usize {
    region_starts(boundaries).len()
}

/// Region start pcts plus implicit finish at 1.0 ΓÇö for UI progress bars.
pub fn display_sector_boundaries(boundaries: &[SectorBoundary]) -> Vec<f64> {
    let mut display = region_starts(boundaries);
    if display.is_empty() {
        return display;
    }
    if *display.last().unwrap_or(&0.0) < 1.0 - PCT_DEDUPE_EPS {
        display.push(1.0);
    }
    display
}

/// SDK-style current sector: max region index where `pct > start`, 1-indexed for display.
pub fn current_sector_from_pct(pct: f32, boundaries: &[SectorBoundary]) -> i32 {
    let regions = region_starts(boundaries);
    if regions.is_empty() {
        return 1;
    }
    let pct = pct as f64;
    let idx = regions.iter().rposition(|&start| pct > start).unwrap_or(0);
    (idx + 1) as i32
}

/// Read sector split lines from session YAML (shared by live telemetry and IBT import).
///
/// Returns empty when split data is missing so callers can suppress sector display
/// rather than fabricating 33%/66% defaults.
pub fn extract_sector_boundaries(session: &SessionInfo) -> Vec<SectorBoundary> {
    let Some(split) = &session.split_time_info else {
        return Vec::new();
    };
    let Some(sectors) = &split.sectors else {
        return Vec::new();
    };
    sectors
        .iter()
        .filter_map(|s| {
            Some(SectorBoundary {
                sector_num: s.sector_num.unwrap_or(0),
                start_pct: s.sector_start_pct?,
            })
        })
        .collect()
}

/// Raw region starts from session YAML (for persistence / trace coach).
#[allow(dead_code)]
pub fn extract_region_starts(session: &SessionInfo) -> Vec<f64> {
    let Some(split) = &session.split_time_info else {
        return Vec::new();
    };
    let Some(sectors) = &split.sectors else {
        return Vec::new();
    };
    let raw: Vec<SectorBoundary> = sectors
        .iter()
        .filter_map(|s| {
            Some(SectorBoundary {
                sector_num: s.sector_num.unwrap_or(0),
                start_pct: s.sector_start_pct?,
            })
        })
        .collect();
    region_starts(&raw)
}

/// Per-lap sector timing state (live telemetry and batch import share this logic).
#[derive(Debug, Clone)]
pub struct SectorSplitState {
    normalized_bounds: Vec<SectorBoundary>,
    next_boundary_idx: usize,
    sector_start_time: f64,
    completed: Vec<(i32, f64)>,
}

impl SectorSplitState {
    pub fn new(boundaries: &[SectorBoundary], lap_start_time: f64, start_pct: f32) -> Self {
        let normalized_bounds = normalize_sector_boundaries(boundaries);
        let mut state = Self {
            normalized_bounds,
            next_boundary_idx: 0,
            sector_start_time: lap_start_time,
            completed: Vec::new(),
        };
        state.skip_passed_boundaries(start_pct);
        state
    }

    pub fn sync_bounds(&mut self, boundaries: &[SectorBoundary], current_pct: f32) {
        let normalized = normalize_sector_boundaries(boundaries);
        if normalized != self.normalized_bounds {
            self.normalized_bounds = normalized;
            self.next_boundary_idx = 0;
            self.skip_passed_boundaries(current_pct);
        }
    }

    pub fn reset_lap(
        &mut self,
        boundaries: &[SectorBoundary],
        lap_start_time: f64,
        start_pct: f32,
    ) {
        self.normalized_bounds = normalize_sector_boundaries(boundaries);
        self.next_boundary_idx = 0;
        self.sector_start_time = lap_start_time;
        self.completed.clear();
        self.skip_passed_boundaries(start_pct);
    }

    pub fn advance(&mut self, prev_pct: f32, prev_time: f64, curr_pct: f32, curr_time: f64) {
        if Self::is_pct_regression(prev_pct, curr_pct) {
            return;
        }
        if Self::is_sf_wrap(prev_pct, curr_pct) {
            self.next_boundary_idx = 0;
            self.skip_passed_boundaries(curr_pct);
        }
        self.detect_forward_crossings(prev_pct, prev_time, curr_pct, curr_time);
    }

    pub fn finish_lap(&mut self, lap_end_time: f64) -> Vec<(i32, f64)> {
        self.record_final_sector(lap_end_time);
        self.completed.clone()
    }

    pub fn completed_sectors(&self) -> &[(i32, f64)] {
        &self.completed
    }

    fn is_sf_wrap(prev_pct: f32, curr_pct: f32) -> bool {
        prev_pct > SF_WRAP_PREV_MIN && curr_pct < SF_WRAP_CURR_MAX
    }

    fn is_pct_regression(prev_pct: f32, curr_pct: f32) -> bool {
        !Self::is_sf_wrap(prev_pct, curr_pct) && curr_pct + PCT_REGRESSION_EPS < prev_pct
    }

    fn skip_passed_boundaries(&mut self, lap_dist_pct: f32) {
        while self.next_boundary_idx < self.normalized_bounds.len() {
            let pct = self.normalized_bounds[self.next_boundary_idx].start_pct;
            if lap_dist_pct as f64 >= pct {
                self.next_boundary_idx += 1;
            } else {
                break;
            }
        }
    }

    fn detect_forward_crossings(
        &mut self,
        prev_pct: f32,
        prev_time: f64,
        curr_pct: f32,
        curr_time: f64,
    ) {
        while self.next_boundary_idx < self.normalized_bounds.len() {
            let boundary = &self.normalized_bounds[self.next_boundary_idx];
            let pct = boundary.start_pct;
            let crossed = prev_pct as f64 <= pct && curr_pct as f64 > pct;
            if !crossed {
                break;
            }
            let ratio = if (curr_pct - prev_pct).abs() > f32::EPSILON {
                ((pct - prev_pct as f64) / (curr_pct - prev_pct) as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cross_time = prev_time + (curr_time - prev_time) * ratio;
            let sector_num = (self.next_boundary_idx + 1) as i32;
            let sector_ms = (cross_time - self.sector_start_time) * 1000.0;
            if sector_ms >= MIN_SECTOR_MS {
                debug!(
                    sector = sector_num,
                    pct,
                    ms = sector_ms,
                    "sector crossing recorded"
                );
                self.completed.push((sector_num, sector_ms));
            }
            self.sector_start_time = cross_time;
            self.next_boundary_idx += 1;
        }
    }

    fn record_final_sector(&mut self, lap_end_time: f64) {
        if self.normalized_bounds.is_empty() {
            return;
        }
        let final_sector = (self.normalized_bounds.len() + 1) as i32;
        if self.completed.iter().any(|(n, _)| *n == final_sector) {
            return;
        }
        let sector_ms = (lap_end_time - self.sector_start_time) * 1000.0;
        if sector_ms >= MIN_SECTOR_MS {
            debug!(
                sector = final_sector,
                ms = sector_ms,
                "final sector recorded at lap finish"
            );
            self.completed.push((final_sector, sector_ms));
        }
    }
}
