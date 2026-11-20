//! Check successive ordinary temporal property edits in one process.
use dive_api::{
    CheckRequest, Store, check_file,
    temporal::{Outcome, Report},
    temporal_reuse::{Artifact, Limits, Program},
};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

fn result(report: &Report) -> Option<Value> {
    let (evidence, word, native_word) = match report.outcome() {
        Outcome::NativeHolds(e) => (e.as_ref(), None, None),
        Outcome::NativeViolation(v) => (
            v.evidence(),
            Some(v.document()),
            Some(
                json!({"target_property":v.word().target_property,"properties":v.word().properties,"back_to":v.word().back_to,
                "edges":v.word().edges.iter().map(|e|json!({"to":e.to,"ordinal":e.ordinal,"label":e.label})).collect::<Vec<_>>() }),
            ),
        ),
        _ => return None,
    };
    let stats = evidence.stats();
    Some(
        json!({"verdict":if word.is_some() {"liveness-violation"} else {"success"},
                "prefix_checks":report.periodic().map(|c|c.prefixes),"native_word":native_word,"word":word.map(dive_trace::TraceDoc::to_json),"generated":stats.generated,
                "distinct":stats.distinct,"depth":stats.depth,"initial_generated":stats.initial_generated,
                "left_on_queue":stats.left_on_queue}),
    )
}
fn micros(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).peekable();
    let fresh = args.next_if(|s| s == "--fresh").is_some();
    let paths = args.collect::<Vec<_>>();
    let (pairs, remainder) = paths.as_chunks::<2>();
    if pairs.is_empty() || !remainder.is_empty() {
        return Err(
            "usage: recheck_temporal [--fresh] model.tla model.cfg [edited.tla edited.cfg ...]"
                .into(),
        );
    }
    let limits = Limits::default();
    let mut artifact: Option<Artifact> = None;
    for [tla, cfg] in pairs {
        let file = Path::new(tla);
        let config = Path::new(cfg);
        let start = Instant::now();
        let mut record = json!({"file":file,"config":config});
        if !fresh {
            let checked = Program::load(file, config).and_then(|program| {
                record["bind_us"] = micros(start).into();
                if let Some(a) = &artifact {
                    let before = Instant::now();
                    match a.recheck(&program, limits) {
                        Ok(r) => {
                            record["path"] = "recheck".into();
                            record["analysis_us"] = micros(before).into();
                            return Ok(r);
                        }
                        Err(reason) => record["recheck_miss"] = reason.to_string().into(),
                    }
                }
                let before = Instant::now();
                let (a, r) = program.capture(limits)?;
                record["path"] = "capture".into();
                record["capture_and_analysis_us"] = micros(before).into();
                artifact = Some(a);
                Ok(r)
            });
            match checked {
                Ok(r) => record["result"] = result(&r).ok_or("unrecognized final result")?,
                Err(e) => record["capture_miss"] = e.to_string().into(),
            }
        }
        if record.get("result").is_none() {
            record["path"] = "fresh".into();
            let report = check_file(&CheckRequest {
                file: file.into(),
                config: Some(config.into()),
                store: Store::Exact,
                time_limit: dive_api::BudgetSetting::Unlimited,
                max_memory: dive_api::BudgetSetting::Unlimited,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..Default::default()
            })?;
            record["result"] = report
                .temporal_result()
                .and_then(result)
                .unwrap_or_else(|| report.to_json());
        }
        record["elapsed_us"] = micros(start).into();
        println!("{record}");
    }
    Ok(())
}
