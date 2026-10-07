//! Full import benchmark including DB save:
//! cargo run --release --example import_bench -- "path\to\file.ibt"

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use race_refinery_desktop_lib::ingest::ibt_importer::{parse_ibt_file_fast, save_parsed_ibt};
use race_refinery_desktop_lib::storage::Database;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let path: PathBuf = env::args()
        .nth(1)
        .expect("usage: import_bench <path.ibt>")
        .into();

    let started = Instant::now();
    let progress = Some(Box::new(|pct: f64, msg: String| {
        println!("  [{pct:.0}%] {msg}");
    })
        as race_refinery_desktop_lib::ingest::ProgressCallback);

    let (parsed, hash, elapsed) = parse_ibt_file_fast(&path, progress)?;

    let trace_points: usize = parsed.laps.iter().map(|l| l.traces.len()).sum();
    println!(
        "Parse: {} laps, {} trace points, {} ms",
        parsed.laps.len(),
        trace_points,
        elapsed
    );
    for lap in &parsed.laps {
        let max_pct = lap
            .traces
            .iter()
            .map(|p| p.dist_pct)
            .fold(0.0_f64, f64::max);
        let time_s = lap
            .lap_time_ms
            .map(|ms| format!("{:.3}s", ms / 1000.0))
            .unwrap_or_else(|| "—".into());
        println!(
            "  lap {:>2}: pace_eligible={} time={} max_pct~{:.3}",
            lap.lap_number,
            lap.pace_eligible(),
            time_s,
            max_pct
        );
    }

    let save_start = Instant::now();
    let db = Database::open()?;
    let result = save_parsed_ibt(&db, &path, parsed, &hash, elapsed)?;
    println!(
        "Save: session {} ({} laps) in {} ms",
        result.session_id,
        result.lap_count,
        save_start.elapsed().as_millis()
    );

    println!("Total wall: {} ms", started.elapsed().as_millis());
    Ok(())
}
