//! Scoped temporal final execution, original claims and independent replay.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::phase_graph::Limits;
use dive_trace::{Ending, Validation};
use std::{path::PathBuf, sync::Arc};
fn base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/probes/tlc-final-scopes")
}
fn inputs(name: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let dir = base().join("models").join(name);
    let a = dive_sema::analyze_file(
        &dir.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("fresh source plus INSTANCE module resolution");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg");
    (ir, dive_config::parse(&cfg).into_config().expect("config"))
}
fn bound(ir: &Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig) -> dive_model::Model {
    dive_model::bind(ir.clone(), cfg, &dive_model::BindOptions::tlc())
        .expect("fresh independent model")
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete reference selection with every fresh word mutation"
)]
fn scoped_final_words_counts_and_all_independent_mutations() {
    let capture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(base().join("final-projection/capture.json")).expect("capture"),
    )
    .expect("JSON");
    let mut supported = 0;
    for row in capture["rows"].as_array().expect("selected") {
        if row["exit"] != 0 || matches!(row["name"].as_str(), Some("TfEnabledFails" | "TfEnabledHolds")) {
            continue;
        }
        supported += 1;
        let name = row["name"].as_str().expect("name");
        let (ir, cfg) = inputs(name);
        let stderr = std::fs::read_to_string(
            base()
                .join("final-projection/raw")
                .join(name)
                .join("stderr"),
        )
        .expect("raw unmodified finalCheck");
        for workers in [1, 3, 4, 8] {
            let options = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let report = tlc_temporal::check_final(&ir, &cfg, &options, Limits::default())
                .expect("pure conditional final execution");
            assert_eq!(report.scheduling(), "final-only");
            assert_eq!(report.selection(), "tlc");
            let execution = match report.outcome() {
                Outcome::Holds(e) => &e.execution,
                Outcome::Violation(v) => &v.candidate().evidence.execution,
                Outcome::Safety(_) => panic!("conditional temporal property cannot route safety"),
            };
            assert_fields(execution, &row["fields"], name);
            if row["fields"]["final_check_code"] == "0" {
                let Outcome::Holds(e) = report.outcome() else {
                    panic!("{name} expected holds");
                };
                assert_eq!(e.execution.capture.workers.used, workers);
                assert!(name.ends_with("Holds"));
                continue;
            }
            let Outcome::Violation(v) = report.outcome() else {
                panic!("{name} expected independently certified violation");
            };
            let c = v.candidate();
            assert_eq!(
                c.evidence.execution.capture.stats.distinct.to_string(),
                row["fields"]["distinct"].as_str().expect("distinct")
            );
            assert_eq!(
                c.evidence.execution.capture.stats.generated.to_string(),
                row["fields"]["generated"].as_str().expect("generated")
            );
            assert_eq!(c.evidence.execution.capture.workers.used, workers);
            assert_eq!(v.validation().validation, Validation::Valid);
            let printed: Vec<bool> = stderr
                .split("@!@!@STARTMSG 2217:4 @!@!@")
                .skip(1)
                .map(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("x = "))
                        .expect("x")
                        == "TRUE"
                })
                .collect();
            let actual: Vec<bool> = c
                .states
                .iter()
                .map(|s| {
                    s[0].as_ref()
                        .expect("x")
                        .as_bool()
                        .expect("bool")
                })
                .collect();
            assert_eq!(actual, printed, "{name} actual pinned word");
            let claims = v.document().properties.join(" and ");
            assert!(
                stderr.contains(&format!("Temporal property {claims} was violated."))
                    || stderr.contains(&format!("Temporal properties {claims} were violated.")),
                "{name} claim identity"
            );
            let fresh = bound(&ir, &cfg);
            let check = |doc: &dive_trace::TraceDoc| {
                dive_trace::validate_doc(&fresh, doc, &[], None)
                    .0
                    .validation
            };
            assert_eq!(check(v.document()), Validation::Valid);
            let mut bad = v.document().clone();
            bad.states[0] =
                serde_json::from_value(serde_json::json!({"x":99})).expect("bad initial");
            assert_ne!(check(&bad), Validation::Valid, "{name} Init mutation");
            let mut bad = v.document().clone();
            bad.states.push(
                serde_json::from_value(serde_json::json!({"x":99})).expect("bad closing state"),
            );
            assert_ne!(
                check(&bad),
                Validation::Valid,
                "{name} closing Next mutation"
            );
            let mut bad = v.document().clone();
            bad.properties = execution.role_names.clone();
            if bad.properties != v.document().properties {
                assert_ne!(check(&bad), Validation::Valid, "{name} derived role is not original claim");
            }
            let mut bad = v.document().clone();
            bad.properties.clear();
            assert_ne!(check(&bad), Validation::Valid, "{name} missing claim");
            let mut bad = v.document().clone();
            bad.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            assert_ne!(
                check(&bad),
                Validation::Valid,
                "{name} invalid closing index"
            );
            let mut bad = v.document().clone();
            bad.properties = vec!["UnknownProperty".into()];
            assert_ne!(check(&bad), Validation::Valid, "{name} foreign claim");
        }
    }
    assert_eq!(supported, 12);
}

fn assert_fields(
    run: &dive_liveness::phase_execution::Complete,
    fields: &serde_json::Value,
    name: &str,
) {
    assert_eq!(
        run.configured_properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(";"),
        fields["configured_names"]
            .as_str()
            .expect("configured names")
    );
    assert_eq!(
        run.role_names.join(";"),
        fields["role_names"].as_str().expect("role names")
    );
    for (g, group) in run.groups.iter().enumerate() {
        let roots = group
            .roots
            .iter()
            .map(|(fp, t)| format!("{fp}:{t}"))
            .collect::<Vec<_>>()
            .join(";");
        assert_eq!(
            roots,
            fields[format!("final.g{g}.roots")]
                .as_str()
                .expect("reference roots"),
            "{name} initial particle consistency"
        );
        for node in group.nodes.iter().filter(|n| n.written) {
            let key = format!("final.g{g}.node.{}.{}", node.fingerprint, node.tableau);
            let bits = node
                .state
                .iter()
                .map(|b| if *b { '1' } else { '0' })
                .collect::<String>();
            assert_eq!(
                bits,
                fields[format!("{key}.states")]
                    .as_str()
                    .expect("state bins"),
                "{name}"
            );
            let edges = node
                .transitions
                .iter()
                .map(|e| {
                    format!(
                        "{}:{}:{}",
                        e.target,
                        e.tableau,
                        e.action
                            .iter()
                            .map(|b| if *b { '1' } else { '0' })
                            .collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            assert_eq!(
                edges,
                fields[format!("{key}.transitions")]
                    .as_str()
                    .expect("edge bins"),
                "{name} closing/stutter/action order"
            );
        }
    }
}


#[test]
fn state_only_and_exported_formal_boundaries_remain_distinct() {
    for name in ["TfEnabledHolds", "TfEnabledFails", "TfInstanceHolds", "TfInstanceFails"] {
        let (ir,cfg)=inputs(name);
        for workers in [1,3,4,8] {
            let opts=CheckOptions{workers,..CheckOptions::tlc(0,0)};
            let report=tlc_temporal::check_final(&ir,&cfg,&opts,Limits::default());
            if name.starts_with("TfInstance") {
                assert!(report.is_err(), "exported temporal formal remains staged: {name}");
                continue;
            }
            let report=report.expect("state-only real safety route");
            let Outcome::Safety(s)=report.outcome() else { panic!("forced helper must not invent temporal evidence"); };
            assert_eq!(s.result().verdict.kind(),if name.ends_with("Holds") {"success"} else {"invariant-violation"});
            if name.ends_with("Fails") {
                assert_eq!(s.validation().expect("fresh finite replay").validation,Validation::Valid);
                let fresh=bound(&ir,&cfg);
                assert_eq!(dive_trace::validate_doc(&fresh,s.document().expect("JSON"),&[],None).0.validation,Validation::Valid);
            }
        }
    }
}
