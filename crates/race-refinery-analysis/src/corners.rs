//! Corner-by-corner time loss between two laps.
//!
//! Each lap's trace becomes a time-vs-distance curve ([`LapTimeline`]): the
//! recorded `elapsed_ms` channel when the trace has it, otherwise speed
//! integrated over distance and scaled to the official lap time. Corners are
//! found on the reference lap's speed trace (a clear slow-down followed by a
//! clear pick-up). Each corner's segment runs from the speed peak before it to
//! the peak after it, so segments tile the lap and their deltas add up to the
//! overall gap.

use serde::{Deserialize, Serialize};

use super::compare::interp;
use super::types::TracePoint;

/// Resolution of the corner-analysis grid across the compared distance range.
const GRID_STEPS: usize = 1000;
/// Moving-average half-width (grid samples) applied before finding corners.
const SMOOTH_RADIUS: usize = 3;
/// A slow-down / pick-up must clear both of these to count as a corner.
const MIN_SPEED_SWING_MS: f64 = 2.5;
const MIN_SPEED_SWING_FRAC: f64 = 0.06;
/// Pedal thresholds for "on the brakes" and "back on full throttle".
const BRAKE_ON: f64 = 0.1;
const THROTTLE_ON: f64 = 0.9;
/// At or below this, a pedal counts as released (matches the map's deadzone).
const PEDAL_IDLE: f64 = 0.05;
/// Trail braking starts at the last sample within this fraction of peak brake.
const TRAIL_HOLD_FRAC: f64 = 0.9;
/// Assist runs separated by at most this many grid samples are merged; runs
/// shorter than `SPAN_MIN_SAMPLES` are dropped as noise.
const SPAN_MERGE_GAP: usize = 2;
const SPAN_MIN_SAMPLES: usize = 3;
/// Speed integration needs most of the lap to scale against the lap time.
const MIN_ESTIMATE_COVERAGE: f64 = 0.9;
const MIN_TIMELINE_POINTS: usize = 20;
/// Shortest shared distance range worth comparing.
const MIN_COMMON_RANGE: f64 = 0.05;

/// Where a comparison's timing came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimingSource {
    /// Both laps carry recorded elapsed time on every trace sample.
    Recorded,
    /// At least one lap's time curve was integrated from speed (sessions
    /// imported before the time channel was stored).
    Estimated,
}

/// Elapsed lap time as a function of lap distance.
#[derive(Debug, Clone)]
pub struct LapTimeline {
    pct: Vec<f64>,
    ms: Vec<f64>,
    pub source: TimingSource,
    /// Full-lap track length implied by this lap's speed and time, in metres.
    pub track_length_m: Option<f64>,
}

impl LapTimeline {
    /// `points` must be sorted ascending by `dist_pct`.
    pub fn from_trace(points: &[TracePoint], lap_time_ms: Option<f64>) -> Option<Self> {
        Self::recorded(points).or_else(|| Self::estimated(points, lap_time_ms?))
    }

    fn recorded(points: &[TracePoint]) -> Option<Self> {
        let mut timed: Vec<(f64, f64, f64)> = points
            .iter()
            .filter_map(|p| p.elapsed_ms.map(|t| (t, p.dist_pct, p.speed)))
            .collect();
        if timed.len() < MIN_TIMELINE_POINTS || timed.len() < points.len() * 9 / 10 {
            return None;
        }
        timed.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Samples right at the start/finish line can still carry the previous
        // (or already the next) lap's distance; trim them so distance rises
        // with time.
        let start = timed.iter().position(|s| s.1 < 0.5)?;
        let end = timed.iter().rposition(|s| s.1 >= 0.5)? + 1;
        if start >= end {
            return None;
        }

        let mut pct = Vec::with_capacity(end - start);
        let mut ms = Vec::with_capacity(end - start);
        let mut metres = 0.0;
        let mut prev: Option<(f64, f64, f64)> = None;
        for &(t, d, v) in &timed[start..end] {
            if let Some((pt, pd, pv)) = prev {
                if d <= pd || t <= pt {
                    continue;
                }
                metres += (v + pv) / 2.0 * (t - pt) / 1000.0;
            }
            pct.push(d);
            ms.push(t);
            prev = Some((t, d, v));
        }
        if pct.len() < MIN_TIMELINE_POINTS {
            return None;
        }
        let covered = pct[pct.len() - 1] - pct[0];
        let track_length_m = (covered > 0.5 && metres > 0.0).then(|| metres / covered);
        Some(Self {
            pct,
            ms,
            source: TimingSource::Recorded,
            track_length_m,
        })
    }

    fn estimated(points: &[TracePoint], lap_time_ms: f64) -> Option<Self> {
        let (first, last) = (points.first()?, points.last()?);
        if points.len() < MIN_TIMELINE_POINTS
            || lap_time_ms <= 0.0
            || last.dist_pct - first.dist_pct < MIN_ESTIMATE_COVERAGE
        {
            return None;
        }
        // Integrate distance / speed ("lap fraction per m/s"); scaling that to
        // the lap time yields both time and track length.
        let speed = |v: f64| v.max(1.0);
        let mut acc = first.dist_pct / speed(first.speed);
        let mut pct = vec![first.dist_pct];
        let mut raw = vec![acc];
        for w in points.windows(2) {
            let dd = w[1].dist_pct - w[0].dist_pct;
            if dd <= 0.0 {
                continue;
            }
            acc += dd / speed((w[0].speed + w[1].speed) / 2.0);
            pct.push(w[1].dist_pct);
            raw.push(acc);
        }
        let full = acc + (1.0 - last.dist_pct) / speed(last.speed);
        let scale = lap_time_ms / full;
        Some(Self {
            pct,
            ms: raw.into_iter().map(|u| u * scale).collect(),
            source: TimingSource::Estimated,
            track_length_m: Some(scale / 1000.0),
        })
    }

    pub(crate) fn range(&self) -> (f64, f64) {
        (self.pct[0], self.pct[self.pct.len() - 1])
    }

    /// Elapsed ms at lap distance `x`, or `None` outside this lap's coverage.
    pub fn at(&self, x: f64) -> Option<f64> {
        let (first, last) = self.range();
        if x < first || x > last {
            return None;
        }
        let i = self.pct.partition_point(|&p| p <= x);
        if i == 0 {
            return Some(self.ms[0]);
        }
        if i >= self.pct.len() {
            return Some(self.ms[self.ms.len() - 1]);
        }
        let (p0, p1) = (self.pct[i - 1], self.pct[i]);
        let t = (x - p0) / (p1 - p0);
        Some(self.ms[i - 1] + (self.ms[i] - self.ms[i - 1]) * t)
    }
}

/// Running time gap (candidate minus reference, ms) over the distance range
/// both laps cover. Zero at the start of that range.
#[derive(Debug, Clone)]
pub struct DeltaCurve {
    candidate: LapTimeline,
    reference: LapTimeline,
    start: f64,
    end: f64,
    candidate_at_start: f64,
    reference_at_start: f64,
}

impl DeltaCurve {
    pub fn new(candidate: LapTimeline, reference: LapTimeline) -> Option<Self> {
        let (cs, ce) = candidate.range();
        let (rs, re) = reference.range();
        let (start, end) = (cs.max(rs), ce.min(re));
        if end - start < MIN_COMMON_RANGE {
            return None;
        }
        Some(Self {
            candidate_at_start: candidate.at(start)?,
            reference_at_start: reference.at(start)?,
            candidate,
            reference,
            start,
            end,
        })
    }

    pub fn at(&self, x: f64) -> Option<f64> {
        if x < self.start || x > self.end {
            return None;
        }
        let c = self.candidate.at(x)? - self.candidate_at_start;
        let r = self.reference.at(x)? - self.reference_at_start;
        Some(c - r)
    }

    /// [`Self::at`] with `x` pulled into the range both laps cover, so corners
    /// that run to the start/finish line still get a time.
    pub(crate) fn at_clamped(&self, x: f64) -> Option<f64> {
        self.at(x.clamp(self.start, self.end))
    }

    pub fn source(&self) -> TimingSource {
        if self.candidate.source == TimingSource::Recorded
            && self.reference.source == TimingSource::Recorded
        {
            TimingSource::Recorded
        } else {
            TimingSource::Estimated
        }
    }

    pub fn track_length_m(&self) -> Option<f64> {
        self.reference
            .track_length_m
            .or(self.candidate.track_length_m)
    }
}

/// Time lost or gained through one corner. Positive deltas mean the candidate
/// was slower / later than the reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CornerDelta {
    /// 1-based, in track order. Detected from speed, so it may not match the
    /// circuit's official turn numbering.
    pub number: u32,
    /// Segment start (speed peak before the corner, or the compared range start).
    pub entry_pct: f64,
    /// Reference lap's slowest point.
    pub apex_pct: f64,
    /// Segment end (speed peak after the corner, or the compared range end).
    pub exit_pct: f64,
    /// Time through this (reference-defined) window on the candidate timeline.
    pub candidate_time_ms: Option<f64>,
    /// Time through the same window on the reference timeline.
    pub reference_time_ms: Option<f64>,
    pub time_delta_ms: f64,
    /// Share of `time_delta_ms` from entry to the reference apex.
    pub entry_delta_ms: f64,
    /// Share of `time_delta_ms` from the reference apex to exit.
    pub exit_delta_ms: f64,
    /// Slowest speed in the segment, m/s.
    pub candidate_min_speed: Option<f64>,
    pub reference_min_speed: Option<f64>,
    /// Candidate brake point minus reference brake point, metres. Positive =
    /// candidate braked later. `None` unless both laps braked.
    pub brake_point_delta_m: Option<f64>,
    /// Candidate full-throttle point minus reference, metres. Positive =
    /// candidate got back to full throttle later. `None` when the corner is
    /// flat or either lap never reached full throttle before the exit.
    pub throttle_point_delta_m: Option<f64>,
    pub candidate: CornerTechnique,
    pub reference: CornerTechnique,
}

/// How one lap drove one corner, from its own driver pedals and timeline.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CornerTechnique {
    /// Time with ABS reducing brake pressure. `None` when the lap has no
    /// `BrakeABSactive` channel (imported before schema v6).
    pub abs_ms: Option<f64>,
    /// Highest driver brake before the reference apex; `None` if not braked.
    pub peak_brake: Option<f64>,
    /// From the last sample near peak brake to brake below 10%: how long the
    /// driver trailed off the brake.
    pub trail_brake_ms: Option<f64>,
    /// Time with neither pedal pressed.
    pub coast_ms: f64,
    /// From this lap's slowest point to full throttle; `None` when the corner
    /// is taken flat or full throttle never comes before the exit.
    pub apex_to_throttle_ms: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LapRole {
    Candidate,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistKind {
    Abs,
}

/// A stretch of lap distance where ABS was intervening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistSpan {
    pub lap: LapRole,
    pub kind: AssistKind,
    pub start_pct: f64,
    pub end_pct: f64,
}

/// Corners and assist spans for one comparison.
#[derive(Debug, Clone, Default)]
pub struct CornerAnalysis {
    pub corners: Vec<CornerDelta>,
    pub assists: Vec<AssistSpan>,
}

/// Find the reference lap's corners, measure the candidate through each, and
/// list where either lap's driver aids intervened.
pub fn analyze_corners(
    candidate: &[TracePoint],
    reference: &[TracePoint],
    delta: &DeltaCurve,
) -> CornerAnalysis {
    let grid = grid(delta.start, delta.end);
    let cand = LapLanes::new(candidate, &delta.candidate, &grid);
    let refr = LapLanes::new(reference, &delta.reference, &grid);
    let gap = fill(grid.iter().map(|&x| delta.at(x)));
    let track_length_m = delta.track_length_m();
    let metres = |a: usize, b: usize| track_length_m.map(|l| (grid[a] - grid[b]) * l);

    let corners = corner_windows(&refr.speed)
        .iter()
        .map(|w| {
            let (entry, apex, exit) = (w.entry, w.apex, w.exit);
            let cand_brake = brake_point(&cand.brake, w.brake_from, apex, w.prev_apex);
            let ref_brake = brake_point(&refr.brake, w.brake_from, apex, w.prev_apex);
            let cand_throttle = throttle_point(&cand.speed, &cand.throttle, entry, exit);
            let ref_throttle = throttle_point(&refr.speed, &refr.throttle, entry, exit);
            let lifted = cand.lifted(entry, exit) || refr.lifted(entry, exit);

            let candidate_time_ms = segment_ms(&cand.time, entry, exit);
            let reference_time_ms = segment_ms(&refr.time, entry, exit);
            CornerDelta {
                number: w.number,
                entry_pct: grid[entry],
                apex_pct: grid[apex],
                exit_pct: grid[exit],
                candidate_time_ms,
                reference_time_ms,
                time_delta_ms: gap[exit] - gap[entry],
                entry_delta_ms: gap[apex] - gap[entry],
                exit_delta_ms: gap[exit] - gap[apex],
                candidate_min_speed: min_in(&cand.speed, entry, exit),
                reference_min_speed: min_in(&refr.speed, entry, exit),
                brake_point_delta_m: cand_brake.zip(ref_brake).and_then(|(c, r)| metres(c, r)),
                throttle_point_delta_m: if lifted {
                    cand_throttle
                        .zip(ref_throttle)
                        .and_then(|(c, r)| metres(c, r))
                } else {
                    None
                },
                candidate: cand.technique(w),
                reference: refr.technique(w),
            }
        })
        .collect();

    let mut assists = cand.assist_spans(LapRole::Candidate, &grid);
    assists.extend(refr.assist_spans(LapRole::Reference, &grid));
    CornerAnalysis { corners, assists }
}

/// Evenly spaced lap-distance samples across `[start, end]`.
pub(crate) fn grid(start: f64, end: f64) -> Vec<f64> {
    (0..=GRID_STEPS)
        .map(|i| start + (end - start) * i as f64 / GRID_STEPS as f64)
        .collect()
}

/// Gaps (`None`) take the previous value; leading gaps take the first value.
fn fill(values: impl Iterator<Item = Option<f64>>) -> Vec<f64> {
    let values: Vec<Option<f64>> = values.collect();
    let mut last = values.iter().flatten().next().copied().unwrap_or(0.0);
    values
        .into_iter()
        .map(|v| {
            last = v.unwrap_or(last);
            last
        })
        .collect()
}

/// One corner's sample indices on the analysis grid, from the reference lap.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CornerWindow {
    pub number: u32,
    pub entry: usize,
    pub apex: usize,
    pub exit: usize,
    /// The previous corner's apex; brake points never walk back past it.
    pub prev_apex: usize,
    /// Where to start looking for the brake point (speed peak before the corner).
    pub brake_from: usize,
}

/// Corners from a reference speed lane: each valley, bounded by the speed
/// peaks either side (the first / last corner extend to the grid ends).
pub(crate) fn corner_windows(ref_speed: &[f64]) -> Vec<CornerWindow> {
    let turns = turning_points(&smooth(ref_speed, SMOOTH_RADIUS));
    let valleys: Vec<usize> = turns
        .iter()
        .filter(|t| t.1 == Turn::Valley)
        .map(|t| t.0)
        .collect();
    let last_idx = ref_speed.len().saturating_sub(1);

    valleys
        .iter()
        .enumerate()
        .map(|(n, &apex)| {
            let peak_before = turns
                .iter()
                .rev()
                .find(|t| t.1 == Turn::Peak && t.0 < apex)
                .map(|t| t.0);
            let peak_after = turns
                .iter()
                .find(|t| t.1 == Turn::Peak && t.0 > apex)
                .map(|t| t.0);
            let entry = if n == 0 { 0 } else { peak_before.unwrap_or(0) };
            let exit = if n + 1 == valleys.len() {
                last_idx
            } else {
                peak_after.unwrap_or(last_idx)
            };
            CornerWindow {
                number: n as u32 + 1,
                entry,
                apex,
                exit,
                prev_apex: if n == 0 { 0 } else { valleys[n - 1] },
                brake_from: peak_before.unwrap_or(entry),
            }
        })
        .collect()
}

/// One lap's channels resampled onto the analysis grid. Pedals are the
/// driver's (raw when stored), so auto-blips and TC/ABS don't move pickup points.
pub(crate) struct LapLanes {
    pub speed: Vec<f64>,
    pub brake: Vec<f64>,
    pub throttle: Vec<f64>,
    /// Elapsed ms on this lap's own timeline.
    pub time: Vec<f64>,
    /// `None` when the lap lacks the ABS channel.
    abs: Option<Vec<bool>>,
}

impl LapLanes {
    pub(crate) fn new(points: &[TracePoint], timeline: &LapTimeline, grid: &[f64]) -> Self {
        let lane = |channel: fn(&TracePoint) -> f64| -> Vec<f64> {
            fill(grid.iter().map(|&x| interp(points, x, channel)))
        };
        let throttle = lane(TracePoint::driver_throttle);
        let abs = points.iter().any(|p| p.abs_active.is_some()).then(|| {
            lane(|p| if p.abs_active == Some(true) { 1.0 } else { 0.0 })
                .into_iter()
                .map(|v| v >= 0.5)
                .collect()
        });
        Self {
            speed: lane(|p| p.speed),
            brake: lane(TracePoint::driver_brake),
            throttle,
            time: fill(grid.iter().map(|&x| timeline.at(x))),
            abs,
        }
    }

    /// Time spent from sample `i` to the next one.
    fn dt(&self, i: usize) -> f64 {
        match self.time.get(i + 1) {
            Some(next) => (next - self.time[i]).max(0.0),
            None => 0.0,
        }
    }

    fn time_where(&self, from: usize, to: usize, on: impl Fn(usize) -> bool) -> f64 {
        (from..to).filter(|&i| on(i)).map(|i| self.dt(i)).sum()
    }

    fn lifted(&self, entry: usize, exit: usize) -> bool {
        self.throttle[entry..=exit].iter().any(|&t| t < THROTTLE_ON)
    }

    pub(crate) fn technique(&self, w: &CornerWindow) -> CornerTechnique {
        let (entry, apex, exit) = (w.entry, w.apex, w.exit);
        let assist = |lane: &Option<Vec<bool>>| {
            lane.as_ref()
                .map(|on| self.time_where(entry, exit, |i| on[i]))
        };
        let peak = (entry..=apex)
            .max_by(|&a, &b| self.brake[a].total_cmp(&self.brake[b]))
            .filter(|&i| self.brake[i] >= BRAKE_ON);
        let trail_brake_ms = peak.map(|p| {
            let hold_level = self.brake[p] * TRAIL_HOLD_FRAC;
            let hold = (p..=apex)
                .rev()
                .find(|&i| self.brake[i] >= hold_level)
                .unwrap_or(p);
            let release = (hold..=exit)
                .find(|&i| self.brake[i] < BRAKE_ON)
                .unwrap_or(exit);
            self.time[release] - self.time[hold]
        });
        let apex_to_throttle_ms = if self.lifted(entry, exit) {
            (entry..=exit)
                .min_by(|&a, &b| self.speed[a].total_cmp(&self.speed[b]))
                .and_then(|slowest| {
                    (slowest..=exit)
                        .find(|&i| self.throttle[i] >= THROTTLE_ON)
                        .map(|full| self.time[full] - self.time[slowest])
                })
        } else {
            None
        };
        CornerTechnique {
            abs_ms: assist(&self.abs),
            peak_brake: peak.map(|i| self.brake[i]),
            trail_brake_ms,
            coast_ms: self.time_where(entry, exit, |i| {
                self.brake[i] <= PEDAL_IDLE && self.throttle[i] <= PEDAL_IDLE
            }),
            apex_to_throttle_ms,
        }
    }

    fn assist_spans(&self, lap: LapRole, grid: &[f64]) -> Vec<AssistSpan> {
        self.abs
            .as_ref()
            .map(|on| {
                assist_runs(on)
                    .into_iter()
                    .map(|(a, b)| AssistSpan {
                        lap,
                        kind: AssistKind::Abs,
                        start_pct: grid[a],
                        end_pct: grid[b],
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Inclusive index ranges where `on` holds, with short gaps bridged and
/// short runs dropped.
fn assist_runs(on: &[bool]) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, _) in on.iter().enumerate().filter(|(_, &v)| v) {
        match runs.last_mut() {
            Some(run) if i - run.1 <= SPAN_MERGE_GAP + 1 => run.1 = i,
            _ => runs.push((i, i)),
        }
    }
    runs.retain(|(a, b)| b - a + 1 >= SPAN_MIN_SAMPLES);
    runs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turn {
    Peak,
    Valley,
}

fn smooth(values: &[f64], radius: usize) -> Vec<f64> {
    (0..values.len())
        .map(|i| {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(values.len() - 1);
            values[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64
        })
        .collect()
}

fn swing(speed: f64) -> f64 {
    (speed.abs() * MIN_SPEED_SWING_FRAC).max(MIN_SPEED_SWING_MS)
}

/// Alternating speed peaks and valleys whose rise/fall clears [`swing`].
/// A valley on the very first sample (lap starts mid-acceleration) is dropped.
fn turning_points(speed: &[f64]) -> Vec<(usize, Turn)> {
    let mut out = Vec::new();
    let Some(&first) = speed.first() else {
        return out;
    };
    let (mut hi, mut lo) = ((0, first), (0, first));
    let mut rising: Option<bool> = None;
    for (i, &v) in speed.iter().enumerate().skip(1) {
        if v > hi.1 {
            hi = (i, v);
        }
        if v < lo.1 {
            lo = (i, v);
        }
        let fell = hi.1 - v >= swing(hi.1);
        let rose = v - lo.1 >= swing(lo.1);
        match rising {
            Some(true) | None if fell => {
                out.push((hi.0, Turn::Peak));
                rising = Some(false);
                lo = (i, v);
            }
            Some(false) | None if rose => {
                if lo.0 > 0 {
                    out.push((lo.0, Turn::Valley));
                }
                rising = Some(true);
                hi = (i, v);
            }
            _ => {}
        }
    }
    out
}

/// First sample at or after `from` (up to `apex`) with the brake applied,
/// walked back while braking was already on (not past `floor`).
pub(crate) fn brake_point(brake: &[f64], from: usize, apex: usize, floor: usize) -> Option<usize> {
    let mut i = (from..=apex).find(|&i| brake[i] >= BRAKE_ON)?;
    while i > floor && brake[i - 1] >= BRAKE_ON {
        i -= 1;
    }
    Some(i)
}

/// First full-throttle sample after this lap's own slowest point in the segment.
fn throttle_point(speed: &[f64], throttle: &[f64], entry: usize, exit: usize) -> Option<usize> {
    let slowest = (entry..=exit).min_by(|&a, &b| speed[a].total_cmp(&speed[b]))?;
    (slowest..=exit).find(|&i| throttle[i] >= THROTTLE_ON)
}

pub(crate) fn min_in(values: &[f64], from: usize, to: usize) -> Option<f64> {
    values[from..=to].iter().copied().reduce(f64::min)
}

/// Elapsed duration through `[entry, exit]` on a lap timeline lane.
fn segment_ms(time: &[f64], entry: usize, exit: usize) -> Option<f64> {
    let (Some(&t0), Some(&t1)) = (time.get(entry), time.get(exit)) else {
        return None;
    };
    let ms = t1 - t0;
    (ms.is_finite() && ms >= 0.0).then_some(ms)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Synthetic lap with slow corners centred at `apexes`. Speed dips from
    /// 70 m/s toward `apex_speed`; brake is applied from `brake_lead` before
    /// each apex; throttle is full except near the apex. When `timed`, elapsed
    /// time is integrated from speed on a 5 km track.
    pub(crate) fn lap(
        apexes: &[f64],
        apex_speed: f64,
        brake_lead: f64,
        timed: bool,
    ) -> Vec<TracePoint> {
        const N: usize = 1200;
        const LENGTH_M: f64 = 5000.0;
        let speed_at = |d: f64| {
            let dip = apexes
                .iter()
                .map(|a| (-((d - a) / 0.02).powi(2)).exp())
                .fold(0.0, f64::max);
            70.0 - (70.0 - apex_speed) * dip
        };
        let mut elapsed = 0.0;
        (0..N)
            .map(|i| {
                let d = i as f64 / N as f64;
                if i > 0 {
                    let prev = (i - 1) as f64 / N as f64;
                    let v = (speed_at(d) + speed_at(prev)) / 2.0;
                    elapsed += (d - prev) * LENGTH_M / v * 1000.0;
                }
                let braking = apexes.iter().any(|a| d > a - brake_lead && d < *a);
                let lifted = apexes.iter().any(|a| (d - a).abs() < 0.03);
                TracePoint {
                    dist_pct: d,
                    speed: speed_at(d),
                    throttle: if lifted { 0.2 } else { 1.0 },
                    brake: if braking { 0.8 } else { 0.0 },
                    gear: 4,
                    elapsed_ms: timed.then_some(elapsed),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn lap_time(points: &[TracePoint]) -> f64 {
        // Close the loop: add the last sample's step to the line.
        let last = points.last().unwrap();
        last.elapsed_ms.unwrap() + (1.0 - last.dist_pct) * 5000.0 / last.speed * 1000.0
    }

    #[test]
    fn recorded_timeline_trims_start_finish_wrap() {
        let mut points = lap(&[0.3], 30.0, 0.03, true);
        // A trailing sample from the next lap that sorts first by distance.
        let mut stray = points[0].clone();
        stray.dist_pct = 0.0001;
        stray.elapsed_ms = Some(999_999.0);
        points.insert(0, stray);
        let tl = LapTimeline::from_trace(&points, None).unwrap();
        assert_eq!(tl.source, TimingSource::Recorded);
        assert!(tl.at(0.5).unwrap() < 999_999.0);
        assert!((tl.track_length_m.unwrap() - 5000.0).abs() < 25.0);
    }

    #[test]
    fn estimate_matches_recorded_time() {
        let points = lap(&[0.25, 0.6], 30.0, 0.03, true);
        let time = lap_time(&points);
        let recorded = LapTimeline::from_trace(&points, Some(time)).unwrap();
        let untimed: Vec<_> = points
            .iter()
            .cloned()
            .map(|mut p| {
                p.elapsed_ms = None;
                p
            })
            .collect();
        let estimated = LapTimeline::from_trace(&untimed, Some(time)).unwrap();
        assert_eq!(estimated.source, TimingSource::Estimated);
        for x in [0.1, 0.4, 0.8] {
            let diff = (recorded.at(x).unwrap() - estimated.at(x).unwrap()).abs();
            assert!(diff < 30.0, "at {x}: {diff} ms apart");
        }
        assert!((estimated.track_length_m.unwrap() - 5000.0).abs() < 25.0);
        assert!(LapTimeline::from_trace(&untimed, None).is_none());
    }

    #[test]
    fn finds_corners_and_attributes_loss() {
        let reference = lap(&[0.2, 0.5, 0.8], 30.0, 0.03, true);
        // Candidate is slower only through the middle corner and brakes earlier.
        let mut candidate = lap(&[0.2, 0.8], 30.0, 0.03, false);
        let slow = lap(&[0.5], 25.0, 0.04, false);
        for (c, s) in candidate.iter_mut().zip(&slow) {
            if (c.dist_pct - 0.5).abs() < 0.12 {
                *c = s.clone();
            }
        }
        let mut elapsed = 0.0;
        for i in 0..candidate.len() {
            if i > 0 {
                let v = (candidate[i].speed + candidate[i - 1].speed) / 2.0;
                elapsed +=
                    (candidate[i].dist_pct - candidate[i - 1].dist_pct) * 5000.0 / v * 1000.0;
            }
            candidate[i].elapsed_ms = Some(elapsed);
        }

        let delta = DeltaCurve::new(
            LapTimeline::from_trace(&candidate, None).unwrap(),
            LapTimeline::from_trace(&reference, None).unwrap(),
        )
        .unwrap();
        assert_eq!(delta.source(), TimingSource::Recorded);
        let corners = analyze_corners(&candidate, &reference, &delta).corners;
        assert_eq!(corners.len(), 3, "{corners:?}");
        assert!((corners[1].apex_pct - 0.5).abs() < 0.01);

        assert!(corners[0].time_delta_ms.abs() < 5.0);
        assert!(corners[2].time_delta_ms.abs() < 5.0);
        assert!(corners[1].time_delta_ms > 100.0);
        let total: f64 = corners.iter().map(|c| c.time_delta_ms).sum();
        assert!((total - delta.at(delta.end).unwrap()).abs() < 1.0);

        let mid = &corners[1];
        assert!(mid.candidate_min_speed.unwrap() < mid.reference_min_speed.unwrap());
        // Absolute corner times match the gap-derived Δ.
        for c in &corners {
            let cand = c.candidate_time_ms.expect("candidate corner time");
            let refr = c.reference_time_ms.expect("reference corner time");
            assert!(
                ((cand - refr) - c.time_delta_ms).abs() < 1.0,
                "C{} abs Δ {} vs gap Δ {}",
                c.number,
                cand - refr,
                c.time_delta_ms
            );
        }
        // Braked 1% of 5 km = 50 m earlier.
        let brake = mid.brake_point_delta_m.unwrap();
        assert!((brake + 50.0).abs() < 10.0, "brake delta {brake}");
        assert!(corners[0].brake_point_delta_m.unwrap().abs() < 5.0);
    }

    #[test]
    fn brake_pickup_prefers_raw_pedal() {
        let reference = lap(&[0.5], 30.0, 0.03, true);
        let delta = DeltaCurve::new(
            LapTimeline::from_trace(&reference, None).unwrap(),
            LapTimeline::from_trace(&reference, None).unwrap(),
        )
        .unwrap();
        // Same applied brake, but the driver's pedal went down 1% (50 m) sooner.
        let raw: Vec<_> = reference
            .iter()
            .cloned()
            .map(|mut p| {
                let pressed = p.dist_pct > 0.46 && p.dist_pct < 0.5;
                p.brake_raw = Some(if pressed { 0.8 } else { 0.0 });
                p
            })
            .collect();

        let with_raw = analyze_corners(&raw, &reference, &delta).corners;
        let brake = with_raw[0].brake_point_delta_m.unwrap();
        assert!((brake + 50.0).abs() < 10.0, "brake delta {brake}");

        let applied_only = analyze_corners(&reference, &reference, &delta).corners;
        assert!(applied_only[0].brake_point_delta_m.unwrap().abs() < 5.0);
    }

    #[test]
    fn flat_trace_has_no_corners() {
        let points = lap(&[], 70.0, 0.0, true);
        let delta = DeltaCurve::new(
            LapTimeline::from_trace(&points, None).unwrap(),
            LapTimeline::from_trace(&points, None).unwrap(),
        )
        .unwrap();
        assert!(analyze_corners(&points, &points, &delta).corners.is_empty());
        assert_eq!(delta.at(0.7), Some(0.0));
    }

    fn timeline(points: &[TracePoint]) -> LapTimeline {
        LapTimeline::from_trace(points, None).unwrap()
    }

    pub(crate) fn self_delta(points: &[TracePoint]) -> DeltaCurve {
        DeltaCurve::new(timeline(points), timeline(points)).unwrap()
    }

    fn time_between(points: &[TracePoint], a: f64, b: f64) -> f64 {
        let tl = timeline(points);
        tl.at(b).unwrap() - tl.at(a).unwrap()
    }

    #[test]
    fn abs_time_and_spans_from_channel() {
        let reference = lap(&[0.5], 30.0, 0.03, true);
        let candidate: Vec<_> = reference
            .iter()
            .cloned()
            .map(|mut p| {
                p.abs_active = Some(p.dist_pct > 0.47 && p.dist_pct < 0.49);
                p
            })
            .collect();
        let analysis = analyze_corners(&candidate, &reference, &self_delta(&reference));
        let corner = &analysis.corners[0];

        let expected = time_between(&reference, 0.47, 0.49);
        let abs = corner.candidate.abs_ms.unwrap();
        assert!((abs - expected).abs() < 250.0, "abs {abs} vs {expected}");
        assert_eq!(corner.reference.abs_ms, None, "no channel on reference");

        assert_eq!(analysis.assists.len(), 1, "{:?}", analysis.assists);
        let span = &analysis.assists[0];
        assert_eq!((span.lap, span.kind), (LapRole::Candidate, AssistKind::Abs));
        assert!((span.start_pct - 0.47).abs() < 0.003, "{span:?}");
        assert!((span.end_pct - 0.49).abs() < 0.003, "{span:?}");
    }

    #[test]
    fn coast_trail_brake_and_pickup_timings() {
        let reference = lap(&[0.5], 30.0, 0.03, true);
        let shaped: Vec<_> = reference
            .iter()
            .cloned()
            .map(|mut p| {
                let d = p.dist_pct;
                p.throttle_raw = Some(if (0.44..0.47).contains(&d) {
                    0.0
                } else {
                    p.throttle
                });
                // Full brake, then a linear trail off to the apex.
                p.brake_raw = Some(if (0.47..0.48).contains(&d) {
                    0.8
                } else if (0.48..0.5).contains(&d) {
                    0.8 * (0.5 - d) / 0.02
                } else {
                    0.0
                });
                p
            })
            .collect();
        let corner = analyze_corners(&shaped, &reference, &self_delta(&reference)).corners[0]
            .candidate
            .clone();

        let coast = time_between(&reference, 0.44, 0.47);
        assert!(
            (corner.coast_ms - coast).abs() < 300.0,
            "coast {corner:?} vs {coast}"
        );
        assert!((corner.peak_brake.unwrap() - 0.8).abs() < 1e-6);
        // Hold ends where brake drops below 90% of peak (0.482); release below 10% (0.4975).
        let trail = time_between(&reference, 0.482, 0.4975);
        let got = corner.trail_brake_ms.unwrap();
        assert!((got - trail).abs() < 400.0, "trail {got} vs {trail}");
        let pickup = time_between(&reference, 0.5, 0.53);
        let got = corner.apex_to_throttle_ms.unwrap();
        assert!((got - pickup).abs() < 400.0, "pickup {got} vs {pickup}");
    }

    #[test]
    fn assist_runs_merge_short_gaps_and_drop_blips() {
        let on = |s: &str| s.chars().map(|c| c == '#').collect::<Vec<_>>();
        assert_eq!(assist_runs(&on("###..###...##")), vec![(0, 7)]);
        assert_eq!(assist_runs(&on("..##..")), vec![]);
        assert_eq!(assist_runs(&on("....")), vec![]);
        assert_eq!(assist_runs(&on("###...###")), vec![(0, 2), (6, 8)]);
    }
}
