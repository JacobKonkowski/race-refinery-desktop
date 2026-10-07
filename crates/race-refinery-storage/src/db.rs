//! SQLite storage. Persists [`AnalyzedSession`] products and serves read models.
//!
//! Schema is versioned via `PRAGMA user_version`. Current schema is **v10**.
//! Versions older than 2 are wiped once (pre-v2 had incompatible lap taxonomy);
//! upgrades from v2 onward use incremental migrations (v3 GPS … v10 traffic).
//! Full wipe remains available via the explicit `clear_database` debug command only.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

use race_refinery_analysis::{
    clear_sticky_times_in_place, is_phantom_lap, pace_eligible_from, AnalyzedSession, TracePoint,
    TrafficEvent,
};

use super::models::*;

const SCHEMA_VERSION: i64 = 10;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY,
    ibt_path TEXT UNIQUE NOT NULL,
    file_hash TEXT NOT NULL,
    track TEXT NOT NULL DEFAULT '',
    car TEXT NOT NULL DEFAULT '',
    session_date TEXT NOT NULL DEFAULT '',
    lap_count INTEGER NOT NULL DEFAULT 0,
    best_lap_ms REAL,
    imported_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS laps (
    id INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    session_num INTEGER NOT NULL DEFAULT 0,
    session_type TEXT NOT NULL DEFAULT '',
    iracing_lap INTEGER NOT NULL DEFAULT 0,
    lap_number INTEGER NOT NULL,
    lap_time_ms REAL,
    delta_best_ok INTEGER,
    delta_session_best_ok INTEGER,
    on_pit_road_start INTEGER NOT NULL DEFAULT 0,
    on_pit_road_end INTEGER NOT NULL DEFAULT 0,
    lap_dist_pct_min REAL,
    lap_dist_pct_max REAL,
    pace_eligible INTEGER NOT NULL DEFAULT 0,
    fuel_start REAL,
    fuel_used REAL,
    avg_speed REAL,
    lf_temp REAL,
    rf_temp REAL,
    lr_temp REAL,
    rr_temp REAL,
    lf_pressure REAL,
    rf_pressure REAL,
    lr_pressure REAL,
    rr_pressure REAL,
    UNIQUE(session_id, session_num, lap_number)
);

CREATE TABLE IF NOT EXISTS sectors (
    id INTEGER PRIMARY KEY,
    lap_id INTEGER NOT NULL REFERENCES laps(id) ON DELETE CASCADE,
    sector_num INTEGER NOT NULL,
    time_ms REAL NOT NULL,
    UNIQUE(lap_id, sector_num)
);

CREATE TABLE IF NOT EXISTS lap_traces (
    id INTEGER PRIMARY KEY,
    lap_id INTEGER NOT NULL REFERENCES laps(id) ON DELETE CASCADE,
    dist_pct REAL NOT NULL,
    speed REAL NOT NULL,
    throttle REAL NOT NULL,
    brake REAL NOT NULL,
    gear INTEGER NOT NULL,
    steering REAL NOT NULL,
    lat REAL,
    lon REAL,
    elapsed_ms REAL,
    throttle_raw REAL,
    brake_raw REAL,
    clutch REAL,
    clutch_raw REAL,
    handbrake_raw REAL,
    abs_active INTEGER,
    rpm REAL,
    lat_accel REAL,
    long_accel REAL,
    yaw_rate REAL
);

CREATE TABLE IF NOT EXISTS lap_traffic_events (
    id INTEGER PRIMARY KEY,
    lap_id INTEGER NOT NULL REFERENCES laps(id) ON DELETE CASCADE,
    dist_pct REAL NOT NULL,
    kind TEXT NOT NULL DEFAULT 'nearby'
);

CREATE INDEX IF NOT EXISTS idx_laps_session ON laps(session_id);
CREATE INDEX IF NOT EXISTS idx_sectors_lap ON sectors(lap_id);
CREATE INDEX IF NOT EXISTS idx_traces_lap ON lap_traces(lap_id);
CREATE INDEX IF NOT EXISTS idx_traffic_lap ON lap_traffic_events(lap_id);
";

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open() -> Result<Self> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path).context("open sqlite database")?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Apply schema migrations.
    ///
    /// - `version < 2`: one-time drop/recreate (incompatible pre-v2 layout).
    /// - `version >= 2`: create-if-not-exists + future incremental steps only.
    ///
    /// Explicit wipe: [`Database::clear_all`] / `clear_database_cmd` (debug builds).
    fn migrate(conn: &Connection) -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 2 {
            conn.execute_batch(
                "DROP TABLE IF EXISTS session_standings;
                 DROP TABLE IF EXISTS lap_traces;
                 DROP TABLE IF EXISTS sectors;
                 DROP TABLE IF EXISTS laps;
                 DROP TABLE IF EXISTS sessions;",
            )?;
            conn.execute_batch(SCHEMA)?;
            // A fresh schema already matches the current version.
            conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        } else {
            conn.execute_batch(SCHEMA)?;
            // v3 keeps GPS on trace samples for racing lines. Existing rows stay
            // NULL until their session is re-imported.
            if version < 3 {
                add_column_if_missing(conn, "lap_traces", "lat", "REAL")?;
                add_column_if_missing(conn, "lap_traces", "lon", "REAL")?;
            }
            // v4 keeps elapsed lap time on trace samples for corner timing.
            if version < 4 {
                add_column_if_missing(conn, "lap_traces", "elapsed_ms", "REAL")?;
            }
            // v5 keeps the driver's raw pedals (before auto-blip / TC / ABS)
            // alongside the applied values, plus clutch and handbrake.
            if version < 5 {
                for col in [
                    "throttle_raw",
                    "brake_raw",
                    "clutch",
                    "clutch_raw",
                    "handbrake_raw",
                ] {
                    add_column_if_missing(conn, "lap_traces", col, "REAL")?;
                }
            }
            // v6 keeps `BrakeABSactive` for assist analysis.
            if version < 6 {
                add_column_if_missing(conn, "lap_traces", "abs_active", "INTEGER")?;
            }
            // v7 RPM.
            if version < 7 {
                add_column_if_missing(conn, "lap_traces", "rpm", "REAL")?;
            }
            // v8 dynamics.
            if version < 8 {
                for col in ["lat_accel", "long_accel", "yaw_rate"] {
                    add_column_if_missing(conn, "lap_traces", col, "REAL")?;
                }
            }
            // v9 tire pressures on lap aggregates.
            if version < 9 {
                for col in ["lf_pressure", "rf_pressure", "lr_pressure", "rr_pressure"] {
                    add_column_if_missing(conn, "laps", col, "REAL")?;
                }
            }
            // v10 sparse traffic events (table created by SCHEMA above).
            if version < SCHEMA_VERSION {
                conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
            }
        }
        Ok(())
    }

    pub fn hash_exists(&self, hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE file_hash = ?1",
            params![hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn path_exists(&self, path: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE ibt_path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Look up an existing session by file identity hash (path+size+mtime key).
    pub fn find_session_id_by_hash(&self, hash: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT id FROM sessions WHERE file_hash = ?1 LIMIT 1",
                params![hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Look up an existing session by absolute IBT path.
    pub fn find_session_id_by_path(&self, path: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT id FROM sessions WHERE ibt_path = ?1 LIMIT 1",
                params![path],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Persist an analyzed session and all of its laps/sectors/traces.
    pub fn insert_session(
        &self,
        ibt_path: &str,
        file_hash: &str,
        session: &AnalyzedSession,
    ) -> Result<i64> {
        let imported_at = chrono::Utc::now().to_rfc3339();
        let best_lap_ms = session.best_lap_ms();
        let lap_count = session.laps.len() as i32;

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO sessions (ibt_path, file_hash, track, car, session_date, lap_count, best_lap_ms, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                ibt_path,
                file_hash,
                session.track,
                session.car,
                session.session_date,
                lap_count,
                best_lap_ms,
                imported_at
            ],
        )?;
        let session_id = tx.last_insert_rowid();

        let mut lap_stmt = tx.prepare(
            "INSERT INTO laps (
                session_id, session_num, session_type, iracing_lap, lap_number, lap_time_ms,
                delta_best_ok, delta_session_best_ok, on_pit_road_start, on_pit_road_end,
                lap_dist_pct_min, lap_dist_pct_max, pace_eligible,
                fuel_start, fuel_used, avg_speed, lf_temp, rf_temp, lr_temp, rr_temp,
                lf_pressure, rf_pressure, lr_pressure, rr_pressure
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
        )?;
        let mut sector_stmt =
            tx.prepare("INSERT INTO sectors (lap_id, sector_num, time_ms) VALUES (?1, ?2, ?3)")?;
        let mut trace_stmt = tx.prepare(
            "INSERT INTO lap_traces (lap_id, dist_pct, speed, throttle, brake, gear, steering, lat, lon, elapsed_ms,
                 throttle_raw, brake_raw, clutch, clutch_raw, handbrake_raw, abs_active,
                 rpm, lat_accel, long_accel, yaw_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        )?;
        let mut traffic_stmt = tx.prepare(
            "INSERT INTO lap_traffic_events (lap_id, dist_pct, kind) VALUES (?1, ?2, ?3)",
        )?;

        for lap in &session.laps {
            lap_stmt.execute(params![
                session_id,
                lap.session_num,
                lap.session_type,
                lap.iracing_lap,
                lap.lap_number,
                lap.lap_time_ms,
                lap.delta_best_ok.map(|b| b as i32),
                lap.delta_session_best_ok.map(|b| b as i32),
                lap.on_pit_road_start as i32,
                lap.on_pit_road_end as i32,
                lap.lap_dist_pct_min as f64,
                lap.lap_dist_pct_max as f64,
                lap.pace_eligible() as i32,
                lap.fuel_start,
                lap.fuel_used,
                lap.avg_speed,
                lap.lf_temp,
                lap.rf_temp,
                lap.lr_temp,
                lap.rr_temp,
                lap.lf_pressure,
                lap.rf_pressure,
                lap.lr_pressure,
                lap.rr_pressure,
            ])?;
            let lap_id = tx.last_insert_rowid();

            for (sector_num, time_ms) in &lap.sectors {
                sector_stmt.execute(params![lap_id, sector_num, time_ms])?;
            }
            for point in &lap.traces {
                trace_stmt.execute(params![
                    lap_id,
                    point.dist_pct,
                    point.speed,
                    point.throttle,
                    point.brake,
                    point.gear,
                    point.steering,
                    point.lat,
                    point.lon,
                    point.elapsed_ms,
                    point.throttle_raw,
                    point.brake_raw,
                    point.clutch,
                    point.clutch_raw,
                    point.handbrake_raw,
                    point.abs_active,
                    point.rpm,
                    point.lat_accel,
                    point.long_accel,
                    point.yaw_rate,
                ])?;
            }
            for ev in &lap.traffic_events {
                traffic_stmt.execute(params![lap_id, ev.dist_pct, ev.kind])?;
            }
        }

        drop(lap_stmt);
        drop(sector_stmt);
        drop(trace_stmt);
        drop(traffic_stmt);
        tx.commit()?;
        Ok(session_id)
    }

    pub fn clear_all(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        self.conn.execute_batch(
            "DELETE FROM lap_traffic_events;
             DELETE FROM lap_traces;
             DELETE FROM sectors;
             DELETE FROM laps;
             DELETE FROM sessions;",
        )?;
        Ok(count as usize)
    }

    pub fn delete_session(&self, session_id: i64) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        Ok(affected > 0)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ibt_path, track, car, session_date, lap_count, best_lap_ms, imported_at
             FROM sessions ORDER BY imported_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_session_summary)?;
        let mut sessions = rows.collect::<Result<Vec<_>, _>>()?;
        // Align list cards with display cleanup (phantoms / sticky / coverage).
        for session in &mut sessions {
            let mut laps = self.load_lap_summaries(session.id)?;
            apply_lap_display_cleanup(&mut laps);
            session.lap_count = laps.len() as i32;
            session.best_lap_ms = best_pace_eligible_ms(&laps);
            session.session_type = latest_session_type(&laps);
        }
        Ok(sessions)
    }

    pub fn get_session(&self, session_id: i64) -> Result<Option<SessionDetail>> {
        let mut session = match self.get_session_summary(session_id)? {
            Some(s) => s,
            None => return Ok(None),
        };
        let laps = self.get_laps_for_session(session_id)?;
        // Refresh summary fields from cleaned display rows (covers pre-cleanup imports).
        session.lap_count = laps.len() as i32;
        session.best_lap_ms = best_pace_eligible_ms(&laps);
        session.session_type = latest_session_type(&laps);
        Ok(Some(SessionDetail { session, laps }))
    }

    fn get_session_summary(&self, session_id: i64) -> Result<Option<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ibt_path, track, car, session_date, lap_count, best_lap_ms, imported_at
             FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_session_summary(row)?)),
            None => Ok(None),
        }
    }

    fn load_lap_summaries(&self, session_id: i64) -> Result<Vec<LapSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_num, session_type, iracing_lap, lap_number, lap_time_ms,
                    delta_best_ok, delta_session_best_ok, on_pit_road_start, on_pit_road_end,
                    lap_dist_pct_min, lap_dist_pct_max, pace_eligible,
                    fuel_start, fuel_used, avg_speed, lf_temp, rf_temp, lr_temp, rr_temp,
                    lf_pressure, rf_pressure, lr_pressure, rr_pressure
             FROM laps WHERE session_id = ?1 ORDER BY session_num, lap_number",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(LapSummary {
                id: row.get(0)?,
                session_num: row.get(1)?,
                session_type: row.get(2)?,
                iracing_lap: row.get(3)?,
                lap_number: row.get(4)?,
                lap_time_ms: row.get(5)?,
                delta_best_ok: row.get::<_, Option<i64>>(6)?.map(|v| v != 0),
                delta_session_best_ok: row.get::<_, Option<i64>>(7)?.map(|v| v != 0),
                on_pit_road_start: row.get::<_, i64>(8)? != 0,
                on_pit_road_end: row.get::<_, i64>(9)? != 0,
                lap_dist_pct_min: row.get(10)?,
                lap_dist_pct_max: row.get(11)?,
                pace_eligible: row.get::<_, i64>(12)? != 0,
                fuel_start: row.get(13)?,
                fuel_used: row.get(14)?,
                avg_speed: row.get(15)?,
                lf_temp: row.get(16)?,
                rf_temp: row.get(17)?,
                lr_temp: row.get(18)?,
                rr_temp: row.get(19)?,
                lf_pressure: row.get(20)?,
                rf_pressure: row.get(21)?,
                lr_pressure: row.get(22)?,
                rr_pressure: row.get(23)?,
                has_traffic: false,
                sectors: Vec::new(),
                delta_to_best_ms: None,
            })
        })?;
        let mut laps = rows.collect::<Result<Vec<_>, _>>()?;
        for lap in &mut laps {
            lap.has_traffic = self.lap_has_traffic(lap.id)?;
        }
        Ok(laps)
    }

    fn lap_has_traffic(&self, lap_id: i64) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM lap_traffic_events WHERE lap_id = ?1",
            params![lap_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn get_laps_for_session(&self, session_id: i64) -> Result<Vec<LapSummary>> {
        let mut laps = self.load_lap_summaries(session_id)?;
        apply_lap_display_cleanup(&mut laps);

        // Fastest pace-eligible lap per sub-session sets the delta baseline.
        let mut best_by_subsession: std::collections::HashMap<i32, f64> =
            std::collections::HashMap::new();
        for lap in &laps {
            if lap.pace_eligible {
                if let Some(lt) = lap.lap_time_ms {
                    best_by_subsession
                        .entry(lap.session_num)
                        .and_modify(|best| {
                            if lt < *best {
                                *best = lt;
                            }
                        })
                        .or_insert(lt);
                }
            }
        }

        for lap in &mut laps {
            lap.delta_to_best_ms = match (lap.lap_time_ms, best_by_subsession.get(&lap.session_num))
            {
                (Some(lt), Some(best)) if lt > 0.0 && *best > 0.0 => Some(lt - best),
                _ => None,
            };
            lap.sectors = self.get_sectors(lap.id)?;
        }
        Ok(laps)
    }

    fn get_sectors(&self, lap_id: i64) -> Result<Vec<SectorTime>> {
        let mut stmt = self.conn.prepare(
            "SELECT sector_num, time_ms FROM sectors WHERE lap_id = ?1 ORDER BY sector_num",
        )?;
        let rows = stmt.query_map(params![lap_id], |row| {
            Ok(SectorTime {
                sector_num: row.get(0)?,
                time_ms: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_lap_traces(&self, lap_ids: &[i64]) -> Result<Vec<LapTrace>> {
        let mut traces = Vec::new();
        for lap_id in lap_ids {
            let lap_number: i32 = self.conn.query_row(
                "SELECT lap_number FROM laps WHERE id = ?1",
                params![lap_id],
                |row| row.get(0),
            )?;
            traces.push(LapTrace {
                lap_id: *lap_id,
                lap_number,
                points: self.get_trace_points(*lap_id)?,
            });
        }
        Ok(traces)
    }

    fn get_trace_points(&self, lap_id: i64) -> Result<Vec<TracePoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT dist_pct, speed, throttle, brake, gear, steering, lat, lon, elapsed_ms,
                    throttle_raw, brake_raw, clutch, clutch_raw, handbrake_raw, abs_active,
                    rpm, lat_accel, long_accel, yaw_rate
             FROM lap_traces WHERE lap_id = ?1 ORDER BY dist_pct",
        )?;
        let points = stmt
            .query_map(params![lap_id], |row| {
                Ok(TracePoint {
                    dist_pct: row.get(0)?,
                    speed: row.get(1)?,
                    throttle: row.get(2)?,
                    brake: row.get(3)?,
                    gear: row.get(4)?,
                    steering: row.get(5)?,
                    lat: row.get(6)?,
                    lon: row.get(7)?,
                    elapsed_ms: row.get(8)?,
                    throttle_raw: row.get(9)?,
                    brake_raw: row.get(10)?,
                    clutch: row.get(11)?,
                    clutch_raw: row.get(12)?,
                    handbrake_raw: row.get(13)?,
                    abs_active: row.get(14)?,
                    rpm: row.get(15)?,
                    lat_accel: row.get(16)?,
                    long_accel: row.get(17)?,
                    yaw_rate: row.get(18)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(points)
    }

    fn get_traffic_events(&self, lap_id: i64) -> Result<Vec<TrafficEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT dist_pct, kind FROM lap_traffic_events WHERE lap_id = ?1 ORDER BY dist_pct",
        )?;
        let rows = stmt.query_map(params![lap_id], |row| {
            Ok(TrafficEvent {
                dist_pct: row.get(0)?,
                kind: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Data needed to compare a single lap: time, sectors, traces, traffic.
    pub fn get_lap_compare_data(&self, lap_id: i64) -> Result<LapCompareData> {
        let lap_time_ms: Option<f64> = self.conn.query_row(
            "SELECT lap_time_ms FROM laps WHERE id = ?1",
            params![lap_id],
            |row| row.get(0),
        )?;
        let sectors = self
            .get_sectors(lap_id)?
            .into_iter()
            .map(|s| (s.sector_num, s.time_ms))
            .collect();
        let traces = self.get_trace_points(lap_id)?;
        let traffic = self.get_traffic_events(lap_id)?;
        Ok((lap_time_ms, sectors, traces, traffic))
    }
}

/// `(lap_time_ms, sectors, traces, traffic)` for [`Database::get_lap_compare_data`].
pub type LapCompareData = (
    Option<f64>,
    Vec<(i32, f64)>,
    Vec<TracePoint>,
    Vec<TrafficEvent>,
);

fn apply_lap_display_cleanup(laps: &mut Vec<LapSummary>) {
    laps.retain(|l| {
        let max = l.lap_dist_pct_max.unwrap_or(0.0) as f32;
        !is_phantom_lap(l.iracing_lap, max)
    });

    {
        use std::collections::HashMap;
        let mut counters: HashMap<i32, i32> = HashMap::new();
        for lap in laps.iter_mut() {
            let n = counters.entry(lap.session_num).or_insert(0);
            *n += 1;
            lap.lap_number = *n;
        }
    }

    clear_sticky_times_in_place(
        laps,
        |l| l.session_num,
        |l| l.lap_time_ms,
        |l, t| l.lap_time_ms = t,
        |l| l.lap_dist_pct_max.unwrap_or(0.0) as f32,
        |l| l.delta_best_ok,
        |l| l.delta_session_best_ok,
    );

    for lap in laps.iter_mut() {
        let max = lap.lap_dist_pct_max.unwrap_or(0.0) as f32;
        lap.pace_eligible = pace_eligible_from(
            lap.lap_time_ms,
            lap.delta_best_ok,
            lap.delta_session_best_ok,
            max,
        );
    }
}

fn best_pace_eligible_ms(laps: &[LapSummary]) -> Option<f64> {
    laps.iter()
        .filter(|l| l.pace_eligible)
        .filter_map(|l| l.lap_time_ms)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn row_to_session_summary(row: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        id: row.get(0)?,
        ibt_path: row.get(1)?,
        track: row.get(2)?,
        car: row.get(3)?,
        session_date: row.get(4)?,
        lap_count: row.get(5)?,
        best_lap_ms: row.get(6)?,
        imported_at: row.get(7)?,
        session_type: String::new(),
    })
}

/// Session type of the latest stint (highest `session_num`), e.g. "Race".
fn latest_session_type(laps: &[LapSummary]) -> String {
    laps.iter()
        .max_by_key(|l| l.session_num)
        .map(|l| l.session_type.clone())
        .unwrap_or_default()
}

/// `ALTER TABLE ... ADD COLUMN` that tolerates the column already existing, so a
/// database created from the current `SCHEMA` can still run older migration steps.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, ty: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.map(|n| n == column).unwrap_or(false));
    if !exists {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty};"))?;
    }
    Ok(())
}

pub fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("race-refinery")
        .join("race-refinery.db")
}
