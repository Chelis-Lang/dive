//! Run successive TLA+/cfg pairs in one process. Use --fresh for ordinary Core.
use dive_api::safety_reuse::{Artifact, Decision, Limits, Program};
use dive_api::{CheckRequest, Store, check_file};
use serde_json::{Value, json};
use std::{ffi::OsString, path::Path, time::Instant};

fn micros(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}
fn result(decision: &Decision) -> Value {
    let c = decision.counts();
    let mut v = json!({"generated":c.generated,"distinct":c.distinct,"depth":c.depth,
        "initial_generated":c.initial_generated,"left_on_queue":c.left_on_queue});
    match decision {
        Decision::Holds(_) => v["verdict"] = "success".into(),
        Decision::Refuted {
            property, trace, ..
        } => {
            v["verdict"] = "invariant_violation".into();
            v["property"] = property.clone().into();
            v["trace"] = trace.to_json();
        }
    }
    v
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).peekable();
    let fresh = args.next_if(|a| a == "--fresh").is_some();
    let paths: Vec<OsString> = args.collect();
    let (pairs, remainder) = paths.as_chunks::<2>();
    if pairs.is_empty() || !remainder.is_empty() {
        return Err(
            "usage: recheck_invariants [--fresh] model.tla model.cfg [edited.tla edited.cfg ...]"
                .into(),
        );
    }
    let mut artifact: Option<Artifact> = None;
    let limits = Limits::default();
    for pair in pairs {
        let file = Path::new(&pair[0]);
        let config = Path::new(&pair[1]);
        let started = Instant::now();
        let mut record = json!({"file":file,"config":config});
        if !fresh {
            let decided = Program::load(file, config).and_then(|p| {
                record["bind_us"] = micros(started).into();
                let scan = Instant::now();
                if let Some(prior) = &artifact {
                    match prior.recheck(&p, limits) {
                        Ok(d) => {
                            record["path"] = "recheck".into();
                            record["scan_us"] = micros(scan).into();
                            return Ok(d);
                        }
                        Err(reason) => record["recheck_miss"] = reason.to_string().into(),
                    }
                }
                let capture = Instant::now();
                let (new, d) = p.capture(limits)?;
                record["capture_us"] = micros(capture).into();
                record["path"] = "capture".into();
                artifact = Some(new);
                Ok(d)
            });
            match decided {
                Ok(d) => record["result"] = result(&d),
                Err(reason) => record["capture_miss"] = reason.to_string().into(),
            }
        }
        if record.get("result").is_none() {
            record["path"] = "fresh".into();
            let req = CheckRequest {
                file: file.into(),
                config: Some(config.into()),
                store: Store::Exact,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..Default::default()
            };
            record["result"] = check_file(&req)?.to_json();
        }
        record["elapsed_us"] = micros(started).into();
        println!("{record}");
    }
    Ok(())
}
