//! Reproduce the complete selected final-only native outcome census.
#![allow(
    clippy::expect_used,
    reason = "explicit evidence-generation executable"
)]
use dive_check::CheckOptions;
use dive_liveness::{phase_graph::Limits, tlc_final};
use std::{path::Path, sync::Arc};
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let base = root.join("crates/dive-liveness/tests/fixtures/probes/tlc-final-check");
    let capture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let mut rows = Vec::new();
    for row in capture["rows"].as_array().expect("full selection") {
        let name = row["name"].as_str().expect("name");
        let source =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.tla")))
                .expect("source");
        let cfg =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.cfg")))
                .expect("config");
        for workers in [1, 3, 4, 8] {
            let options = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let a = dive_sema::analyze_source(
                &format!("{name}.tla"),
                &source,
                &dive_sema::Options::default(),
            );
            let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
            let cfg = dive_config::parse(&cfg).into_config().expect("cfg");
            let native = match dive_model::bind(
                ir,
                &cfg,
                &dive_model::BindOptions {
                    eval: options.eval,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            ) {
                Err(e) => {
                    serde_json::json!({"status":"binding-failure","detail":format!("{e:?}"),"actual_workers":0})
                }
                Ok(model) => match tlc_final::check(&model, &options, Limits::default()) {
                    Ok(tlc_final::Analysis::Holds(e)) => {
                        serde_json::json!({"status":"final-only-holds","actual_workers":e.execution.capture.workers.used,"generated":e.execution.capture.stats.generated,"distinct":e.execution.capture.stats.distinct,"checked_pems":e.checked.len()})
                    }
                    Ok(tlc_final::Analysis::Candidate(c)) => {
                        serde_json::json!({"status":"unvalidated-candidate","actual_workers":c.evidence.execution.capture.workers.used,"generated":c.evidence.execution.capture.stats.generated,"distinct":c.evidence.execution.capture.stats.distinct,"states":c.states.iter().map(|s|format!("{s:?}")).collect::<Vec<_>>(),"back_to":c.back_to,"properties":c.properties,"selected_group":c.evidence.checked.last().map(|c|c.group),"selected_pem":c.evidence.checked.last().map(|c|c.pem)})
                    }
                    Err(e) => {
                        serde_json::json!({"status":"refusal-or-error","code":e.error.code(),"detail":e.error.to_string(),"actual_workers":e.workers.as_ref().map(|w|w.used)})
                    }
                },
            };
            rows.push(serde_json::json!({"model":name,"requested_workers":workers,"reference_exit":row["exit"],"reference_final_code":row["fields"]["final_check_code"],"reference_failure_agreement_credit":false,"ordinary_strict_parity_credit":false,"native":native}));
        }
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"schema_version":1,"selection_rows":30,"widths":[1,3,4,8],"scope":"Explicit bounded TLC/fingerprint/final-only library census; every selected reference failure retained without error-class or ordinary strict parity credit; candidate validation is tested separately","rows":rows})).expect("serialize"));
}
