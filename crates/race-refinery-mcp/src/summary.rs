//! Cap MCP payloads: structured numbers, no full distance-aligned series dumps.

use race_refinery_analysis::{CornerConsistency, CornerDelta, LapComparison, SectorDelta};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareSummary {
    pub candidate_lap_id: i64,
    pub reference_lap_id: i64,
    pub candidate_time_ms: Option<f64>,
    pub reference_time_ms: Option<f64>,
    pub delta_ms: Option<f64>,
    pub sector_deltas: Vec<SectorDelta>,
    pub corners: Vec<CornerDelta>,
    pub track_length_m: Option<f64>,
    /// Omitted from MCP: full distance-aligned channel series (use Analyze UI).
    pub series_omitted: bool,
    pub series_point_count: usize,
}

pub fn summarize_compare(cmp: LapComparison) -> CompareSummary {
    CompareSummary {
        candidate_lap_id: cmp.candidate_lap_id,
        reference_lap_id: cmp.reference_lap_id,
        candidate_time_ms: cmp.candidate_time_ms,
        reference_time_ms: cmp.reference_time_ms,
        delta_ms: cmp.delta_ms,
        sector_deltas: cmp.sector_deltas,
        corners: cmp.corners,
        track_length_m: cmp.track_length_m,
        series_omitted: true,
        series_point_count: cmp.series.len(),
    }
}

/// Compact consistency rows for MCP (same numbers, clearer units in JSON keys).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencySummary {
    pub corners: Vec<CornerConsistency>,
}

pub fn summarize_consistency(corners: Vec<CornerConsistency>) -> ConsistencySummary {
    ConsistencySummary { corners }
}

pub fn json_text<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| e.to_string())
}
