//! Run safety, independent counterexample replay, temporal checking and a stop.

use std::path::Path;
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{BudgetSetting, CheckRequest, check_file};
use serde_json::Value;

type ExampleResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn request(root: &Path, config: &str) -> CheckRequest {
    CheckRequest {
        file: root.join("Counter.tla"),
        config: Some(root.join(config)),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        time_limit: BudgetSetting::Explicit(Duration::from_secs(10)),
        max_memory: BudgetSetting::Explicit(1 << 30),
        ..CheckRequest::default()
    }
}

fn check(request: &CheckRequest, verdict: &str) -> ExampleResult<Value> {
    let report = check_file(request)?.to_json();
    if report["verdict"] != verdict || report["engine"] != "core" {
        return Err(format!("expected core {verdict}, received {report}").into());
    }
    Ok(report)
}

fn replay(request: &CheckRequest, report: &Value) -> ExampleResult {
    let document = TraceDoc::from_json(report)?;
    let default_config = request.file.with_extension("cfg");
    let model = SpecModel::load(
        &request.file,
        request.config.as_deref().unwrap_or(&default_config),
        &TraceOptions {
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?;
    let proof = model.try_validate(&document)?;
    if proof.validation != Validation::Valid {
        return Err(format!(
            "independent replay did not validate: {:?}",
            proof.validation
        )
        .into());
    }
    Ok(())
}

fn main() -> ExampleResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples");

    let safety = request(&root, "Counter.cfg");
    let report = check(&safety, "success")?;
    println!(
        "safety: success, {} distinct states",
        report["statistics"]["distinct"]
    );

    let violation = request(&root, "CounterViolation.cfg");
    let report = check(&violation, "invariant-violation")?;
    replay(&violation, &report)?;
    println!(
        "safety counterexample: {} independently validated",
        report["property"]
    );

    let temporal = request(&root, "CounterTemporal.cfg");
    let report = check(&temporal, "liveness-violation")?;
    replay(&temporal, &report)?;
    println!(
        "temporal counterexample: {} independently validated",
        report["property"]
    );

    let mut limited = safety;
    limited.max_states = Some(2);
    let report = check(&limited, "stopped")?;
    if report["stopped"]["resource"] != "states" {
        return Err(format!("expected a state-budget stop, received {report}").into());
    }
    println!(
        "resource stop: {} distinct states, limit {}",
        report["statistics"]["distinct"], report["stopped"]["limit"]
    );
    Ok(())
}
