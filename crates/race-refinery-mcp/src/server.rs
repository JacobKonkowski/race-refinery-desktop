//! MCP tool surface over Race Refinery's local session library.

use std::sync::Arc;

use parking_lot::Mutex;
use race_refinery_analysis::{
    compare_laps as run_compare, corner_consistency as run_consistency, CompareInput,
    ConsistencyLap,
};
use race_refinery_storage::{CategoryRule, Database, SessionSummary};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Deserialize;

use crate::summary::{json_text, summarize_compare, summarize_consistency};

/// Default localhost port shown in Settings and used when `mcpPort` is unset/0.
pub const DEFAULT_MCP_PORT: u16 = 7847;

const MAX_SESSIONS: usize = 200;
const MAX_MATCH_LAPS: usize = 500;

#[derive(Clone)]
pub struct RaceRefineryMcp {
    db: Arc<Mutex<Database>>,
    /// Populated by `#[tool_router]`; read by `#[tool_handler]`.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ListSessionsArgs {
    /// Case-sensitive track name match when set.
    #[serde(default)]
    track: Option<String>,
    /// Case-sensitive car name match when set.
    #[serde(default)]
    car: Option<String>,
    /// Inclusive lower bound on session date (`YYYY-MM-DD` or RFC3339 prefix).
    #[serde(default)]
    since: Option<String>,
    /// Inclusive upper bound on session date (`YYYY-MM-DD` or RFC3339 prefix).
    #[serde(default)]
    until: Option<String>,
    /// Max sessions to return (default 50, max 200).
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SearchSessionsArgs {
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    car: Option<String>,
    #[serde(default)]
    session_type: Option<String>,
    /// When true, only return pace-eligible laps (and sessions that have them).
    #[serde(default)]
    pace_eligible_only: Option<bool>,
    #[serde(default)]
    exclude_pit: Option<bool>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SessionIdArgs {
    session_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct LapIdArgs {
    lap_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CompareArgs {
    candidate_lap_id: i64,
    reference_lap_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ConsistencyArgs {
    /// Lap whose corners define the segments (usually session best).
    reference_lap_id: i64,
    /// Laps to measure through those corners (include reference if desired).
    lap_ids: Vec<i64>,
}

#[tool_router]
impl RaceRefineryMcp {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    fn ok_json<T: serde::Serialize>(&self, value: &T) -> Result<CallToolResult, McpError> {
        let text = json_text(value).map_err(|e| McpError::internal_error(e, None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    fn db_err(err: impl ToString) -> McpError {
        McpError::internal_error(err.to_string(), None)
    }

    #[tool(
        description = "List imported iRacing sessions in the local Race Refinery library (track, car, date, lap count, best pace-eligible lap). Newest imports first. Times are milliseconds."
    )]
    fn list_sessions(
        &self,
        Parameters(args): Parameters<ListSessionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let limit = args.limit.unwrap_or(50).clamp(1, MAX_SESSIONS as u32) as usize;
        let sessions = self
            .db
            .lock()
            .list_sessions()
            .map_err(Self::db_err)?;
        let filtered: Vec<SessionSummary> = sessions
            .into_iter()
            .filter(|s| session_matches(s, &args.track, &args.car, &args.since, &args.until))
            .take(limit)
            .collect();
        self.ok_json(&filtered)
    }

    #[tool(
        description = "Search sessions/laps with filters (track, car, session type, date range, pace-eligible only, exclude pit). Returns matching laps with session metadata — use for 'what did I run last weekend', best-by-car/track, and multi-session questions. Times are milliseconds."
    )]
    fn search_sessions(
        &self,
        Parameters(args): Parameters<SearchSessionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let limit = args.limit.unwrap_or(100).clamp(1, MAX_MATCH_LAPS as u32) as usize;
        let rule = CategoryRule {
            track: args.track.clone(),
            car: args.car.clone(),
            session_type: args.session_type.clone(),
            pace_eligible_only: args.pace_eligible_only,
            exclude_pit: args.exclude_pit,
            max_delta_to_best_ms: None,
            since: args.since.clone(),
        };
        let db = self.db.lock();
        let mut matches = db.match_laps(&rule).map_err(Self::db_err)?;
        drop(db);

        if let Some(until) = filled(&args.until) {
            matches.retain(|m| session_date_on_or_before(&m.session_date, until));
        }
        matches.truncate(limit);
        self.ok_json(&matches)
    }

    #[tool(
        description = "Get one session and its lap table (sectors, paceEligible, deltaToBestMs, pit flags). Use for best lap and where time was lost vs session best once you pick candidate/reference lap ids."
    )]
    fn get_session(
        &self,
        Parameters(args): Parameters<SessionIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let detail = self
            .db
            .lock()
            .get_session(args.session_id)
            .map_err(Self::db_err)?;
        match detail {
            Some(d) => self.ok_json(&d),
            None => Err(McpError::invalid_params(
                format!("session {} not found", args.session_id),
                None,
            )),
        }
    }

    #[tool(
        description = "Get one lap's metadata and sector times (same fields as the session lap table). Times are milliseconds."
    )]
    fn get_lap(
        &self,
        Parameters(args): Parameters<LapIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let laps = self
            .db
            .lock()
            .load_laps(&[args.lap_id])
            .map_err(Self::db_err)?;
        match laps.into_iter().next() {
            Some(lap) => self.ok_json(&lap),
            None => Err(McpError::invalid_params(
                format!("lap {} not found", args.lap_id),
                None,
            )),
        }
    }

    #[tool(
        description = "Alias of get_lap: one lap summary with sectors. Times are milliseconds."
    )]
    fn get_lap_summary(
        &self,
        Parameters(args): Parameters<LapIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.get_lap(Parameters(args))
    }

    #[tool(
        description = "Distance-aligned compare of candidate vs reference lap: overall/sector deltas, per-corner time loss, and technique (trailBrakeMs, peakBrake, coastMs, apexToThrottleMs, ABS). Returns structured numbers in milliseconds / metres; omits the full channel series. Use for 'where I lost time' and trail-brake coaching from measured deltas."
    )]
    fn compare_laps(
        &self,
        Parameters(args): Parameters<CompareArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db.lock();
        let (cand_time, cand_sectors, cand_traces, cand_traffic) = db
            .get_lap_compare_data(args.candidate_lap_id)
            .map_err(Self::db_err)?;
        let (ref_time, ref_sectors, ref_traces, ref_traffic) = db
            .get_lap_compare_data(args.reference_lap_id)
            .map_err(Self::db_err)?;
        drop(db);

        let candidate = CompareInput {
            lap_id: args.candidate_lap_id,
            lap_time_ms: cand_time,
            sectors: &cand_sectors,
            traces: &cand_traces,
            traffic: &cand_traffic,
        };
        let reference = CompareInput {
            lap_id: args.reference_lap_id,
            lap_time_ms: ref_time,
            sectors: &ref_sectors,
            traces: &ref_traces,
            traffic: &ref_traffic,
        };
        let summary = summarize_compare(run_compare(&candidate, &reference));
        self.ok_json(&summary)
    }

    #[tool(
        description = "Brake-point and corner-time consistency across clean laps using the reference lap's corners. Pass pace-eligible lap ids from one sub-session (or the same car/track across sessions). Useful for brake-point consistency and same-corner questions."
    )]
    fn corner_consistency(
        &self,
        Parameters(args): Parameters<ConsistencyArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args.lap_ids.is_empty() {
            return Err(McpError::invalid_params(
                "lap_ids must include at least one lap",
                None,
            ));
        }
        let db = self.db.lock();
        let load = |lap_id: i64| {
            db.get_lap_compare_data(lap_id)
                .map(|(time, _, traces, _)| (lap_id, time, traces))
                .map_err(Self::db_err)
        };
        let reference = load(args.reference_lap_id)?;
        let others = args
            .lap_ids
            .iter()
            .copied()
            .filter(|&id| id != args.reference_lap_id)
            .map(load)
            .collect::<Result<Vec<_>, _>>()?;
        drop(db);

        fn input(lap: &(i64, Option<f64>, Vec<race_refinery_analysis::TracePoint>)) -> ConsistencyLap<'_> {
            ConsistencyLap {
                lap_id: lap.0,
                lap_time_ms: lap.1,
                traces: &lap.2,
            }
        }
        let laps: Vec<ConsistencyLap> = std::iter::once(&reference)
            .chain(&others)
            .map(input)
            .collect();
        let corners = run_consistency(&input(&reference), &laps);
        self.ok_json(&summarize_consistency(corners))
    }
}

#[tool_handler]
impl ServerHandler for RaceRefineryMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Race Refinery local read-only tools for imported iRacing sessions and laps. \
                 Prefer paceEligible laps for pace questions. Times are milliseconds unless a field name says otherwise. \
                 compare_laps includes trailBrakeMs and corner deltas — cite those numbers when advising technique. \
                 No write tools; no live telemetry.",
            )
            .with_server_info(Implementation::new("race-refinery", env!("CARGO_PKG_VERSION")))
    }
}

fn filled(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn session_matches(
    session: &SessionSummary,
    track: &Option<String>,
    car: &Option<String>,
    since: &Option<String>,
    until: &Option<String>,
) -> bool {
    if let Some(track) = filled(track) {
        if session.track != track {
            return false;
        }
    }
    if let Some(car) = filled(car) {
        if session.car != car {
            return false;
        }
    }
    if let Some(since) = filled(since) {
        if !session_date_on_or_after(&session.session_date, since) {
            return false;
        }
    }
    if let Some(until) = filled(until) {
        if !session_date_on_or_before(&session.session_date, until) {
            return false;
        }
    }
    true
}

fn session_date_on_or_after(session_date: &str, since: &str) -> bool {
    session_date >= since
}

fn session_date_on_or_before(session_date: &str, until: &str) -> bool {
    // Allow YYYY-MM-DD upper bounds against RFC3339 timestamps.
    if until.len() == 10 && session_date.len() >= 10 {
        return &session_date[..10] <= until;
    }
    session_date <= until
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_bounds_work_on_rfc3339() {
        assert!(session_date_on_or_after("2026-11-15T18:00:00Z", "2026-11-15"));
        assert!(!session_date_on_or_after("2026-11-14T18:00:00Z", "2026-11-15"));
        assert!(session_date_on_or_before("2026-11-15T18:00:00Z", "2026-11-15"));
        assert!(!session_date_on_or_before("2026-11-16T01:00:00Z", "2026-11-15"));
    }

    #[test]
    fn session_filter_matches_track_and_car() {
        let session = SessionSummary {
            id: 1,
            ibt_path: String::new(),
            track: "okayama".into(),
            car: "mx5".into(),
            session_date: "2026-11-15T12:00:00Z".into(),
            lap_count: 1,
            best_lap_ms: Some(100_000.0),
            imported_at: String::new(),
            session_type: "Practice".into(),
        };
        assert!(session_matches(
            &session,
            &Some("okayama".into()),
            &Some("mx5".into()),
            &Some("2026-11-15".into()),
            &Some("2026-11-15".into()),
        ));
        assert!(!session_matches(
            &session,
            &Some("spa".into()),
            &None,
            &None,
            &None,
        ));
    }
}
