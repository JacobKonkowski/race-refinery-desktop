//! Track outline generation from GPS samples.
//!
//! iRacing ships no circuit geometry, so the outline is derived from one clean
//! lap of `Lat` / `Lon` samples paired with `LapDistPct`. The result is a closed
//! polyline parameterized by lap distance, normalized into a unit box, plus an
//! SVG path for the UI. Consumers place cars by interpolating the polyline at a
//! car's `lapDistPct`.
//!
//! Pure math: no I/O, no Tauri.

use serde::{Deserialize, Serialize};

use super::cleanup::pace_eligible_from;
use super::segment::lap_dist_range;
use super::types::LapFrames;

/// Points kept in a generated outline (evenly spaced in lap distance).
pub const OUTLINE_POINTS: usize = 256;

/// Minimum usable GPS samples before an outline is generated.
pub const MIN_GPS_SAMPLES: usize = 64;

/// Minimum lap fraction the samples must span.
pub const MIN_COVERAGE: f64 = 0.95;

/// Fraction of the unit box left empty around the circuit.
const PADDING: f64 = 0.06;

/// A sample is treated as a GPS glitch when it sits this many times the median
/// spacing away from *both* neighbours.
const JUMP_FACTOR: f64 = 12.0;

/// Floor for the glitch threshold so dense samples don't reject normal motion.
const JUMP_FLOOR_M: f64 = 25.0;

/// Meters per degree of latitude (and of longitude at the equator).
const METERS_PER_DEG: f64 = 111_320.0;

/// One GPS sample from a lap, in iRacing units (degrees).
#[derive(Debug, Clone, Copy)]
pub struct GpsSample {
    pub lap_dist_pct: f64,
    pub lat: f64,
    pub lon: f64,
}

/// One outline vertex: lap fraction plus normalized position in a unit box
/// (`x` right, `y` down, both 0..1 with north up).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutlinePoint {
    pub pct: f64,
    pub x: f64,
    pub y: f64,
}

/// Mapping from GPS degrees into an outline's unit box.
///
/// Stored alongside the outline so any other lap's GPS — a racing line, a
/// braking trace — lands in the same coordinate space as the circuit stroke.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackProjection {
    /// Centroid the source samples were projected around.
    pub origin_lat: f64,
    pub origin_lon: f64,
    /// Bounding-box corner of the projected polyline, in meters.
    pub min_x: f64,
    pub min_y: f64,
    /// Meters-to-unit-box scale, then centering offsets.
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

impl TrackProjection {
    /// Project one GPS sample into the outline's unit box.
    pub fn project(&self, lat: f64, lon: f64) -> (f64, f64) {
        let lon_scale = METERS_PER_DEG * self.origin_lat.to_radians().cos();
        let x_m = (lon - self.origin_lon) * lon_scale;
        let y_m = (self.origin_lat - lat) * METERS_PER_DEG;
        (
            self.offset_x + (x_m - self.min_x) * self.scale,
            self.offset_y + (y_m - self.min_y) * self.scale,
        )
    }
}

/// A generated circuit outline, cached per track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackOutline {
    pub track: String,
    pub points: Vec<OutlinePoint>,
    /// Closed SVG path over a `0 0 1 1` viewBox.
    pub svg_path: String,
    /// Lap fraction spanned by the source samples.
    pub coverage: f64,
    /// Usable GPS samples the outline was built from.
    pub sample_count: u32,
    /// Absent in cache files written before racing lines existed; those outlines
    /// still draw, but GPS cannot be placed on them until a re-import.
    #[serde(default)]
    pub projection: Option<TrackProjection>,
}

impl TrackOutline {
    /// Whether `other` was built from a better source lap than `self`.
    pub fn is_improved_by(&self, other: &TrackOutline) -> bool {
        if other.coverage > self.coverage + f64::EPSILON {
            return true;
        }
        other.coverage + f64::EPSILON >= self.coverage && other.sample_count > self.sample_count
    }
}

/// Build an outline from one lap's GPS samples.
///
/// Returns `None` when the lap has too few samples, lacks GPS channels, does not
/// cover the lap, or is geometrically degenerate.
pub fn build_outline(track: &str, samples: &[GpsSample]) -> Option<TrackOutline> {
    let mut sorted: Vec<GpsSample> = samples
        .iter()
        .copied()
        .filter(|s| s.lap_dist_pct.is_finite() && (0.0..=1.0).contains(&s.lap_dist_pct))
        .filter(|s| s.lat.is_finite() && s.lon.is_finite())
        // iRacing reports 0/0 before the GPS channels populate.
        .filter(|s| s.lat != 0.0 || s.lon != 0.0)
        .collect();
    if sorted.len() < MIN_GPS_SAMPLES {
        return None;
    }
    sorted.sort_by(|a, b| {
        a.lap_dist_pct
            .partial_cmp(&b.lap_dist_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let coverage = sorted.last()?.lap_dist_pct - sorted.first()?.lap_dist_pct;
    if coverage < MIN_COVERAGE {
        return None;
    }

    let (origin_lat, origin_lon) = centroid(&sorted);
    let projected = drop_glitches(project(&sorted, origin_lat, origin_lon));
    if projected.len() < MIN_GPS_SAMPLES {
        return None;
    }

    let resampled: Vec<OutlinePoint> = (0..OUTLINE_POINTS)
        .map(|i| {
            let pct = i as f64 / OUTLINE_POINTS as f64;
            let (x, y) = interpolate_at(&projected, pct);
            OutlinePoint { pct, x, y }
        })
        .collect();

    let (points, fit) = normalize(&resampled)?;
    let svg_path = svg_path(&points);

    Some(TrackOutline {
        track: track.to_string(),
        points,
        svg_path,
        coverage,
        sample_count: projected.len() as u32,
        projection: Some(TrackProjection {
            origin_lat,
            origin_lon,
            min_x: fit.min_x,
            min_y: fit.min_y,
            scale: fit.scale,
            offset_x: fit.offset_x,
            offset_y: fit.offset_y,
        }),
    })
}

/// Project a GPS sample into `outline`'s unit box.
///
/// `None` for outlines cached before the projection was stored.
pub fn project_sample(outline: &TrackOutline, lat: f64, lon: f64) -> Option<(f64, f64)> {
    Some(outline.projection?.project(lat, lon))
}

/// Project a run of GPS samples into `outline`'s unit box, in order.
///
/// Returns an empty vec when the outline has no stored projection.
pub fn project_polyline(outline: &TrackOutline, samples: &[GpsSample]) -> Vec<(f64, f64)> {
    let Some(projection) = outline.projection else {
        return Vec::new();
    };
    samples
        .iter()
        .filter(|s| s.lat.is_finite() && s.lon.is_finite())
        .filter(|s| s.lat != 0.0 || s.lon != 0.0)
        .map(|s| projection.project(s.lat, s.lon))
        .collect()
}

/// Build an outline from the best candidate lap of a session.
///
/// Pit-in / pit-out laps are excluded outright; remaining laps are tried in
/// order of sim-rated pace eligibility, then GPS sample count, so the cleanest
/// available lap defines the circuit. Returns `None` when the source has no GPS
/// channels or no lap covers the circuit.
pub fn outline_from_laps(track: &str, laps: &[LapFrames]) -> Option<TrackOutline> {
    let mut ranked: Vec<(bool, usize, &LapFrames)> = laps
        .iter()
        .filter(|lap| !lap.frames.iter().any(|f| f.on_pit_road))
        .map(|lap| (is_pace_eligible(lap), gps_sample_count(lap), lap))
        .filter(|(_, samples, _)| *samples >= MIN_GPS_SAMPLES)
        .collect();

    // Pace-eligible first, then the densest GPS coverage.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    ranked
        .iter()
        .find_map(|(_, _, lap)| build_outline(track, &gps_samples(lap)))
}

fn is_pace_eligible(lap: &LapFrames) -> bool {
    let (_, max_pct) = lap_dist_range(&lap.frames);
    pace_eligible_from(
        lap.sdk_lap_time_ms,
        lap.delta_best_ok,
        lap.delta_session_best_ok,
        max_pct,
    )
}

fn gps_sample_count(lap: &LapFrames) -> usize {
    lap.frames
        .iter()
        .filter(|f| matches!((f.lat, f.lon), (Some(lat), Some(lon)) if lat != 0.0 || lon != 0.0))
        .count()
}

fn gps_samples(lap: &LapFrames) -> Vec<GpsSample> {
    lap.frames
        .iter()
        .filter_map(|f| {
            Some(GpsSample {
                lap_dist_pct: f.lap_dist_pct as f64,
                lat: f.lat?,
                lon: f.lon?,
            })
        })
        .collect()
}

/// Position on an outline at a lap fraction, wrapping across the finish line.
pub fn point_at(points: &[OutlinePoint], pct: f64) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    Some(interpolate_at(points, pct.rem_euclid(1.0)))
}

/// Mean latitude / longitude of the samples.
fn centroid(samples: &[GpsSample]) -> (f64, f64) {
    let n = samples.len() as f64;
    (
        samples.iter().map(|s| s.lat).sum::<f64>() / n,
        samples.iter().map(|s| s.lon).sum::<f64>() / n,
    )
}

/// Project lat/lon degrees to local meters around `lat0` / `lon0`, with north
/// mapped to decreasing `y` so screen space reads the usual way. Output
/// coordinates are meters until [`normalize`] scales them into the unit box.
fn project(samples: &[GpsSample], lat0: f64, lon0: f64) -> Vec<OutlinePoint> {
    let lon_scale = METERS_PER_DEG * lat0.to_radians().cos();

    samples
        .iter()
        .map(|s| OutlinePoint {
            pct: s.lap_dist_pct,
            x: (s.lon - lon0) * lon_scale,
            y: (lat0 - s.lat) * METERS_PER_DEG,
        })
        .collect()
}

/// Drop isolated teleports (GPS dropouts / resets). Only samples far from both
/// neighbours are removed, so a normal run of motion never cascades away.
fn drop_glitches(points: Vec<OutlinePoint>) -> Vec<OutlinePoint> {
    if points.len() < 3 {
        return points;
    }
    let mut spans: Vec<f64> = points
        .windows(2)
        .map(|w| distance(w[0], w[1]))
        .filter(|d| d.is_finite())
        .collect();
    if spans.is_empty() {
        return points;
    }
    spans.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = spans[spans.len() / 2];
    let threshold = (median * JUMP_FACTOR).max(JUMP_FLOOR_M);

    points
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            let prev = if *i == 0 { None } else { points.get(i - 1) };
            let next = points.get(i + 1);
            match (prev, next) {
                (Some(a), Some(b)) => {
                    distance(*a, **p) <= threshold || distance(**p, *b) <= threshold
                }
                _ => true,
            }
        })
        .map(|(_, p)| *p)
        .collect()
}

fn distance(a: OutlinePoint, b: OutlinePoint) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

/// Interpolate `(x, y)` at lap fraction `t` over points sorted by fraction,
/// treating the gap between the last and first sample as the closing segment.
fn interpolate_at(points: &[OutlinePoint], t: f64) -> (f64, f64) {
    let first = points[0];
    let last = points[points.len() - 1];

    if t <= first.pct || t >= last.pct {
        let span = 1.0 - last.pct + first.pct;
        if span <= f64::EPSILON {
            return (first.x, first.y);
        }
        let travelled = if t >= last.pct {
            t - last.pct
        } else {
            1.0 - last.pct + t
        };
        return lerp(last, first, travelled / span);
    }

    let idx = points.partition_point(|p| p.pct <= t).max(1);
    let a = points[idx - 1];
    let b = points[idx];
    let span = b.pct - a.pct;
    let u = if span > f64::EPSILON {
        (t - a.pct) / span
    } else {
        0.0
    };
    lerp(a, b, u)
}

fn lerp(a: OutlinePoint, b: OutlinePoint, u: f64) -> (f64, f64) {
    (a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u)
}

/// Meters-to-unit-box fit produced by [`normalize`].
struct BoxFit {
    min_x: f64,
    min_y: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
}

/// Fit the projected polyline into a padded unit box, preserving aspect ratio.
fn normalize(points: &[OutlinePoint]) -> Option<(Vec<OutlinePoint>, BoxFit)> {
    let (min_x, max_x, min_y, max_y) = points.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(lo_x, hi_x, lo_y, hi_y), p| (lo_x.min(p.x), hi_x.max(p.x), lo_y.min(p.y), hi_y.max(p.y)),
    );
    let width = max_x - min_x;
    let height = max_y - min_y;
    let extent = width.max(height);
    if !extent.is_finite() || extent <= f64::EPSILON {
        return None;
    }

    let scale = (1.0 - 2.0 * PADDING) / extent;
    let offset_x = (1.0 - width * scale) / 2.0;
    let offset_y = (1.0 - height * scale) / 2.0;

    let normalized = points
        .iter()
        .map(|p| OutlinePoint {
            pct: p.pct,
            x: offset_x + (p.x - min_x) * scale,
            y: offset_y + (p.y - min_y) * scale,
        })
        .collect();

    Some((
        normalized,
        BoxFit {
            min_x,
            min_y,
            scale,
            offset_x,
            offset_y,
        },
    ))
}

/// Closed SVG path over a `0 0 1 1` viewBox.
fn svg_path(points: &[OutlinePoint]) -> String {
    let mut path = String::with_capacity(points.len() * 16);
    for (i, p) in points.iter().enumerate() {
        path.push_str(if i == 0 { "M " } else { " L " });
        path.push_str(&format!("{:.4},{:.4}", p.x, p.y));
    }
    path.push_str(" Z");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oval centred near Monza, 1 km x 500 m, sampled evenly around the lap.
    fn oval(count: usize) -> Vec<GpsSample> {
        let lat0 = 45.6156;
        let lon0 = 9.2811;
        (0..count)
            .map(|i| {
                let pct = i as f64 / count as f64;
                let theta = pct * std::f64::consts::TAU;
                GpsSample {
                    lap_dist_pct: pct,
                    lat: lat0 + (250.0 * theta.sin()) / METERS_PER_DEG,
                    lon: lon0 + (500.0 * theta.cos()) / (METERS_PER_DEG * lat0.to_radians().cos()),
                }
            })
            .collect()
    }

    #[test]
    fn builds_closed_outline_from_oval() {
        let outline = build_outline("Monza", &oval(600)).expect("outline");

        assert_eq!(outline.track, "Monza");
        assert_eq!(outline.points.len(), OUTLINE_POINTS);
        assert!(outline.svg_path.starts_with("M "));
        assert!(outline.svg_path.ends_with(" Z"));
        assert!(outline.coverage >= MIN_COVERAGE);
    }

    #[test]
    fn outline_pct_is_monotonic_and_inside_unit_box() {
        let outline = build_outline("Monza", &oval(600)).expect("outline");

        for pair in outline.points.windows(2) {
            assert!(pair[1].pct > pair[0].pct, "pct must increase");
        }
        for p in &outline.points {
            assert!((0.0..=1.0).contains(&p.x), "x out of box: {}", p.x);
            assert!((0.0..=1.0).contains(&p.y), "y out of box: {}", p.y);
        }
    }

    #[test]
    fn wide_oval_fills_the_long_axis() {
        let outline = build_outline("Monza", &oval(600)).expect("outline");
        let span_x = outline
            .points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p.x), hi.max(p.x))
            });

        // The 1 km axis should occupy the padded box, and the 500 m axis half of it.
        assert!(span_x.1 - span_x.0 > 0.85, "span {:?}", span_x);
    }

    #[test]
    fn rejects_short_or_partial_laps() {
        assert!(build_outline("Monza", &oval(10)).is_none());

        let half: Vec<GpsSample> = oval(600)
            .into_iter()
            .filter(|s| s.lap_dist_pct < 0.5)
            .collect();
        assert!(build_outline("Monza", &half).is_none());
    }

    #[test]
    fn rejects_missing_gps_channels() {
        let blank: Vec<GpsSample> = oval(600)
            .into_iter()
            .map(|s| GpsSample {
                lat: 0.0,
                lon: 0.0,
                ..s
            })
            .collect();
        assert!(build_outline("Monza", &blank).is_none());
    }

    #[test]
    fn isolated_gps_teleport_does_not_dominate_the_box() {
        let mut samples = oval(600);
        samples[300] = GpsSample {
            lap_dist_pct: samples[300].lap_dist_pct,
            lat: 48.0,
            lon: 12.0,
        };

        let outline = build_outline("Monza", &samples).expect("outline");
        let span_x = outline
            .points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p.x), hi.max(p.x))
            });

        // Without glitch rejection the oval would collapse next to the outlier.
        assert!(span_x.1 - span_x.0 > 0.85, "span {:?}", span_x);
    }

    #[test]
    fn projection_places_source_gps_back_onto_the_outline() {
        let samples = oval(600);
        let outline = build_outline("Monza", &samples).expect("outline");

        // Every source sample should land on the stroke it generated.
        for sample in samples.iter().step_by(37) {
            let (x, y) = project_sample(&outline, sample.lat, sample.lon).expect("projected");
            let (ox, oy) = point_at(&outline.points, sample.lap_dist_pct).expect("outline point");
            assert!((x - ox).abs() < 0.02, "x {x} vs outline {ox}");
            assert!((y - oy).abs() < 0.02, "y {y} vs outline {oy}");
            assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
        }
    }

    #[test]
    fn projected_polyline_follows_sample_order() {
        let samples = oval(600);
        let outline = build_outline("Monza", &samples).expect("outline");

        let line = project_polyline(&outline, &samples);
        assert_eq!(line.len(), samples.len());

        let first = project_sample(&outline, samples[0].lat, samples[0].lon).expect("first");
        assert_eq!(line[0], first);
    }

    #[test]
    fn outlines_without_a_projection_cannot_place_gps() {
        let mut outline = build_outline("Monza", &oval(600)).expect("outline");
        outline.projection = None;

        assert!(project_sample(&outline, 45.6, 9.28).is_none());
        assert!(project_polyline(&outline, &oval(600)).is_empty());
    }

    #[test]
    fn cached_outlines_without_a_projection_still_deserialize() {
        let outline = build_outline("Monza", &oval(600)).expect("outline");
        let mut json = serde_json::to_value(&outline).expect("serialize");
        json.as_object_mut().expect("object").remove("projection");

        let legacy: TrackOutline = serde_json::from_value(json).expect("deserialize");
        assert_eq!(legacy.points.len(), OUTLINE_POINTS);
        assert!(legacy.projection.is_none());
    }

    #[test]
    fn point_at_wraps_across_the_finish_line() {
        let outline = build_outline("Monza", &oval(600)).expect("outline");

        let start = point_at(&outline.points, 0.0).expect("start");
        let wrapped = point_at(&outline.points, 1.0).expect("wrapped");
        assert!((start.0 - wrapped.0).abs() < 1e-6);
        assert!((start.1 - wrapped.1).abs() < 1e-6);

        let negative = point_at(&outline.points, -0.25).expect("negative");
        let three_quarter = point_at(&outline.points, 0.75).expect("0.75");
        assert!((negative.0 - three_quarter.0).abs() < 1e-6);
    }
}
