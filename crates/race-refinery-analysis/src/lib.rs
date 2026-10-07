//! Pure post-session analysis.
//!
//! Depends only on [`race_refinery_telemetry`]. No storage, no Tauri, no I/O. The
//! pipeline turns raw frames into an [`AnalyzedSession`]; [`compare`] measures one
//! lap against another. Cleanup drops phantom reset buckets and sticky
//! `LapLastLapTime` copies; pace eligibility still requires the sim's `_OK`
//! flags plus near-full lap distance coverage.

pub mod aggregates;
pub mod cleanup;
pub mod compare;
pub mod consistency;
pub mod corners;
pub mod pipeline;
pub mod sectors;
pub mod segment;
pub mod track_map;
pub mod traffic;
pub mod types;

pub use cleanup::{
    clear_sticky_times_in_place, finalize_analyzed_laps, has_full_coverage, is_phantom_lap,
    pace_eligible_from, FULL_LAP_PCT,
};
pub use compare::{compare_laps, AlignedPoint, CompareInput, LapComparison, SectorDelta};
pub use consistency::{corner_consistency, ConsistencyLap, ConsistencyPoint, CornerConsistency};
pub use corners::{AssistKind, AssistSpan, CornerDelta, CornerTechnique, LapRole, TimingSource};
pub use pipeline::analyze_session;
pub use track_map::{
    build_outline, outline_from_laps, point_at, project_polyline, project_sample, GpsSample,
    OutlinePoint, TrackOutline, TrackProjection,
};
pub use traffic::{events_for_lap, field_nearby, TrafficSample, NEARBY_PCT};
pub use types::{
    AnalyzedLap, AnalyzedSession, LapFrames, RawFrame, SectorBoundary, SessionMeta, TracePoint,
    TrafficEvent,
};
