//! Probe pedal / driver-aid channels in an IBT: which exist, and how often the
//! raw (driver) and applied pedals disagree.
//! cargo run --example probe_pedals -- "path\to\file.ibt"

use std::env;
use std::path::PathBuf;

use pitwall::ibt::IbtReader;
use pitwall::{VarData, VariableInfo};

const GAP: f32 = 0.05;

fn main() -> anyhow::Result<()> {
    let path: PathBuf = env::args()
        .nth(1)
        .expect("usage: probe_pedals <path.ibt>")
        .into();

    let mut reader = IbtReader::open(&path)?;
    let schema = reader.variables().clone();

    let mut names: Vec<&VariableInfo> = schema
        .variables
        .values()
        .filter(|v| {
            let n = v.name.to_ascii_lowercase();
            [
                "abs",
                "tc",
                "traction",
                "raw",
                "clutch",
                "handbrake",
                "brake",
                "throttle",
            ]
            .iter()
            .any(|k| n.contains(k))
        })
        .collect();
    names.sort_by(|a, b| a.name.cmp(&b.name));
    println!("Driver-aid / pedal channels:");
    for v in &names {
        println!(
            "  {:<24} {:?} x{} [{}] {}",
            v.name, v.data_type, v.count, v.units, v.description
        );
    }

    let get = |n: &str| schema.get_variable(n).cloned();
    let (throttle, throttle_raw) = (get("Throttle"), get("ThrottleRaw"));
    let (brake, brake_raw) = (get("Brake"), get("BrakeRaw"));
    let abs_active = get("BrakeABSactive");
    let gear = get("Gear");

    let f = |data: &[u8], v: &Option<VariableInfo>| {
        v.as_ref().and_then(|v| f32::from_bytes(data, v).ok())
    };
    let (mut frames, mut braking, mut abs_gap, mut abs_flag, mut abs_both) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut on_throttle, mut tc_gap, mut blip, mut blip_neutral) = (0u64, 0u64, 0u64, 0u64);
    let mut max_brake_gap = 0f32;
    let mut max_tc_gap = 0f32;
    while let Some((data, _, _)) = reader.read_next_frame()? {
        frames += 1;
        let flag = abs_active
            .as_ref()
            .and_then(|v| bool::from_bytes(&data, v).ok())
            .unwrap_or(false);
        if flag {
            abs_flag += 1;
        }
        if let (Some(b), Some(br)) = (f(&data, &brake), f(&data, &brake_raw)) {
            if br >= 0.1 {
                braking += 1;
                let g = br - b;
                max_brake_gap = max_brake_gap.max(g);
                if g > GAP {
                    abs_gap += 1;
                    if flag {
                        abs_both += 1;
                    }
                }
            }
        }
        if let (Some(t), Some(tr)) = (f(&data, &throttle), f(&data, &throttle_raw)) {
            if tr > GAP {
                on_throttle += 1;
                let g = tr - t;
                max_tc_gap = max_tc_gap.max(g);
                if g > GAP {
                    tc_gap += 1;
                }
            }
            if t - tr > GAP {
                blip += 1;
                let g = gear
                    .as_ref()
                    .and_then(|v| i32::from_bytes(&data, v).ok())
                    .unwrap_or(-1);
                if g == 0 {
                    blip_neutral += 1;
                }
            }
        }
    }

    println!("\n{frames} frames");
    println!("Braking (raw >= 0.1):       {braking}");
    println!("  raw - applied > {GAP}:     {abs_gap} (max gap {max_brake_gap:.3})");
    println!("  BrakeABSactive true:       {abs_flag} (both {abs_both})");
    println!("On throttle (raw > {GAP}):   {on_throttle}");
    println!("  raw - applied > {GAP}:     {tc_gap} (max gap {max_tc_gap:.3})");
    println!("Blips (applied - raw > {GAP}): {blip} ({blip_neutral} in neutral)");
    Ok(())
}
