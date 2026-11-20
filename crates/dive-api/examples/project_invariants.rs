//! Project a completed ordinary safety proof onto a smaller invariant inventory.
use dive_api::{
    CheckRequest, Store, check_file,
    safety_projection::{Limits, Program},
};
use serde_json::json;
use std::{ffi::OsString, path::Path, time::Instant};

fn micros(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX)
}
fn bind(pair: &[OsString; 2]) -> Result<Program, Box<dyn std::error::Error>> {
    Ok(Program::load(Path::new(&pair[0]), Path::new(&pair[1]))?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).peekable();
    let fresh = args.next_if(|s| s == "--fresh").is_some();
    let relation = args.next_if(|s| s == "--relation").is_some();
    let paths = args.collect::<Vec<_>>();
    let (pairs, rest) = paths.as_chunks::<2>();
    let Some((old, edits)) = pairs.split_first() else {
        return Err("expected original.tla original.cfg followed by edits".into());
    };
    if edits.is_empty() || !rest.is_empty() || fresh && relation {
        return Err("usage: project_invariants [--fresh|--relation] original.tla original.cfg edited.tla edited.cfg ...".into());
    }
    if fresh {
        for [tla, cfg] in pairs {
            let start = Instant::now();
            let report = check_file(&CheckRequest {
                file: tla.into(),
                config: Some(cfg.into()),
                store: Store::Exact,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..Default::default()
            })?
            .to_json();
            println!(
                "{}",
                json!({"path":"fresh","elapsed_us":micros(start),"verdict":report["verdict"],"statistics":report["statistics"]})
            );
        }
        return Ok(());
    }
    let start = Instant::now();
    let limits = Limits::default();
    let p = bind(old)?;
    let previous_bind = micros(start);
    if relation {
        for pair in edits {
            let now = Instant::now();
            let current = bind(pair)?;
            let r = p.subset(&current, limits, limits)?;
            println!(
                "{}",
                json!({"path":"subset_relation","previous_bind_us":previous_bind,"current_bind_and_relation_us":micros(now),"invariants":r.invariants()})
            );
        }
        return Ok(());
    }
    let proof = p.capture(limits)?;
    let c = proof.original_counts();
    println!(
        "{}",
        json!({"path":"capture","verdict":"success","elapsed_us":micros(start),"bind_us":previous_bind,"statistics":{"distinct":c.distinct,"generated":c.generated,"initial_generated":c.initial_generated,"left_on_queue":c.left_on_queue,"depth":c.depth}})
    );
    for pair in edits {
        let time = Instant::now();
        let current = bind(pair)?;
        let scan = Instant::now();
        let c = proof.project(&current, limits)?;
        println!(
            "{}",
            json!({"path":"projection","verdict":"success","elapsed_us":micros(time),"projection_us":micros(scan),"statistics":{"distinct":c.distinct,"generated":c.generated,"initial_generated":c.initial_generated,"left_on_queue":c.left_on_queue,"depth":c.depth}})
        );
    }
    Ok(())
}
