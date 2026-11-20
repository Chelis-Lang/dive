//! Compare the actual ordinary check with the non-deciding first-claim relation.
use dive_api::{
    BudgetSetting, CheckRequest, Store, check_file,
    temporal::Outcome,
    temporal_projection::{Limits, Program},
};
use serde_json::json;
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or("expected --relation or --fresh")?;
    let receipt = if mode == "--relation" {
        let path = args
            .next()
            .ok_or("expected previous ordinary native report JSON")?;
        serde_json::from_slice(&std::fs::read(path)?)?
    } else {
        serde_json::Value::Null
    };
    let limits = Limits {
        max_states: Some(args.next().ok_or("expected max_states")?.parse()?),
        max_depth: Some(args.next().ok_or("expected max_depth")?.parse()?),
    };
    let paths = args.collect::<Vec<_>>();
    let (pairs, remainder) = paths.as_chunks::<2>();
    if !remainder.is_empty()
        || pairs.is_empty()
        || !matches!(mode.as_str(), "--fresh" | "--relation" | "--native-report")
    {
        return Err("usage: project_first_temporal (--relation prior-report.json | --fresh | --native-report) max-states max-depth model.tla model.cfg [edited.tla edited.cfg ...]".into());
    }
    let mut prior = None;
    for [module, config] in pairs {
        let start = Instant::now();
        let mut record = if mode == "--relation" {
            match Program::load(Path::new(module), Path::new(config), limits) {
                Err(reason) => json!({"path":"miss","reason":reason.to_string()}),
                Ok(current) => {
                    if let Some(previous) = &prior {
                        match Program::first_claim(previous, &current, &receipt) {
                            Ok(name) => json!({"path":"relation","first":name}),
                            Err(reason) => json!({"path":"miss","reason":reason.to_string()}),
                        }
                    } else {
                        prior = Some(current);
                        json!({"path":"prepared"})
                    }
                }
            }
        } else {
            let r = check_file(&CheckRequest {
                file: module.into(),
                config: Some(config.into()),
                workers: Some(1),
                store: Store::Exact,
                seed: Some(0),
                fp: Some(0),
                max_states: limits.max_states,
                max_depth: limits.max_depth,
                time_limit: BudgetSetting::Unlimited,
                max_memory: BudgetSetting::Unlimited,
                ..Default::default()
            })?;
            if mode == "--native-report" {
                println!("{}", r.to_json());
                continue;
            }
            match r.temporal_result().map(dive_api::temporal::Report::outcome) {
                Some(Outcome::NativeHolds(e)) => {
                    let s = e.stats();
                    json!({"path":"fresh_native_holds","counts":[s.generated,s.distinct,s.depth,s.initial_generated,s.left_on_queue],
                        "periodic":r.to_json()["periodic"]})
                }
                _ => json!({"path":"fresh","report":r.to_json()}),
            }
        };
        record["elapsed_us"] = u64::try_from(start.elapsed().as_micros())
            .unwrap_or(u64::MAX)
            .into();
        println!("{record}");
        if mode == "--relation" && prior.is_none() {
            break;
        }
    }
    Ok(())
}
