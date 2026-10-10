//! Explicit final-only facade and independent validation certificates.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::phase_graph::Limits;
use std::sync::Arc;
fn inputs(source: &str, config: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let name = source.split_whitespace().nth(2).expect("module");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(config).into_config().expect("cfg"),
    )
}
#[test]
fn real_model_final_witness_is_independently_certified_at_each_width() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-final-check/models");
    for name in [
        "FinalState",
        "FinalAction",
        "FinalFairness",
        "FinalSafe",
        "ExecMultiple",
        "PhaseLocalRole",
        "PhaseTableau",
        "ExecRevisit",
    ] {
        let src =
            std::fs::read_to_string(base.join(name).join(format!("{name}.tla"))).expect("source");
        let cfg =
            std::fs::read_to_string(base.join(name).join(format!("{name}.cfg"))).expect("config");
        let (ir, cfg) = inputs(&src, &cfg);
        for workers in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let report = tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default())
                .expect("supported final check");
            assert_eq!(report.scheduling(), "final-only");
            assert_eq!(report.selection(), "tlc");
            assert_eq!(report.options().workers, workers);
            match report.outcome() {
                Outcome::Holds(e) => {
                    assert_eq!(name, "FinalSafe");
                    assert_eq!(e.execution.capture.workers.used, workers);
                }
                Outcome::Violation(v) => {
                    assert_ne!(name, "FinalSafe");
                    assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
                    assert_ne!(v.document().properties, [] as [String; 0]);
                    assert_eq!(
                        v.candidate().evidence.execution.capture.workers.used,
                        workers
                    );
                    if name == "ExecMultiple" {
                        assert_eq!(v.document().properties, ["P", "Q"]);
                    }
                    if name == "PhaseLocalRole" {
                        assert_eq!(v.document().properties, ["Live"]);
                        assert_eq!(v.candidate().evidence.execution.role_names, ["Identity"]);
                    }
                }
            }
        }
    }
}
#[test]
fn explicit_mode_admission_and_bounds_keep_actual_evidence() {
    let src = "---- MODULE ApiBound ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1}\nLive == []<>(x=2)\n====\n";
    let (ir, cfg) = inputs(src, "INIT Init\nNEXT Next\nPROPERTY Live\n");
    let mut opts = CheckOptions::tlc(0, 0);
    opts.store = dive_store::StoreMode::Exact;
    let fail =
        tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect_err("exact refusal");
    assert_eq!(fail.workers.expect("actual admission").used, 0);
    assert!(fail.evidence.is_none());
    opts = CheckOptions::dive(0);
    assert!(tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default()).is_err());
    opts = CheckOptions {
        workers: 8,
        ..CheckOptions::tlc(0, 0)
    };
    let fail = tlc_temporal::check_final(
        &ir,
        &cfg,
        &opts,
        dive_liveness::phase_graph::Limits {
            graph: dive_liveness::product::Limits {
                steps: 32,
                ..dive_liveness::product::Limits::default()
            },
            ..Limits::default()
        },
    )
    .expect_err("bound");
    assert!(fail.workers.is_some());
    opts.workers = 0;
    assert_eq!(
        tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default())
            .expect_err("badwidth")
            .workers
            .expect("admission")
            .used,
        0
    );
}
#[test]
fn startup_effects_hooks_and_opaque_source_operators_refuse_before_binding() {
    let cfg = "INIT Init\nNEXT Next\nPROPERTY Live\n";
    let body = "VARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n";
    for extra in [
        "ASSUME PrintT(TRUE)\n",
        "UnusedStartup == PrintT(TRUE)\n",
        "CONSTANT C\nReplacement == PrintT(TRUE)\n",
        "Hidden(F(_)) == F(TRUE)\nUnusedStartup == Hidden(PrintT)\n",
        "UnusedStartup == TLCGet(\"level\")\n",
    ] {
        let source = format!("---- MODULE Admission ----\nEXTENDS TLC\n{body}{extra}====\n");
        let config = if extra.starts_with("CONSTANT") {
            format!("{cfg}CONSTANT C <- Replacement\n")
        } else {
            cfg.into()
        };
        let (ir, config) = inputs(&source, &config);
        for standard in [false, true] {
            // The startup binder also visits a standard module's nonnative
            // top-level zero-arity bodies. Module classification is not authority.
            let mut json = serde_json::to_value(ir.as_ref()).expect("IR serialization");
            let root = ir.root().index();
            json["modules"][root]["standard"] = serde_json::json!(standard);
            let ir = Arc::new(serde_json::from_value(json).expect("IR mutated classification"));
            let failure = tlc_temporal::check_final(
                &ir,
                &config,
                &CheckOptions::tlc(0, 0),
                Limits::default(),
            )
            .expect_err("effects are refused at source admission");
            assert!(
                matches!(failure.error, tlc_temporal::Error::Unsupported(_)),
                "{failure:?}"
            );
            assert_eq!(failure.workers.expect("pre-binding facts").used, 0);
            assert!(failure.evidence.is_none());
        }
    }
    let source = format!("---- MODULE Hook ----\nEXTENDS TLC\n{body}Hook == PrintT(TRUE)\n====\n");
    for suffix in [
        "_PERIODIC Hook\n",
        "POSTCONDITION Hook\n",
        "_RL_REWARD Hook\n",
        "_POSSIBLE Hook\n",
    ] {
        let (ir, config) = inputs(&source, &format!("{cfg}{suffix}"));
        let failure =
            tlc_temporal::check_final(&ir, &config, &CheckOptions::tlc(0, 0), Limits::default())
                .expect_err("hook pre-bind admission");
        assert!(
            matches!(
                failure.error,
                tlc_temporal::Error::Unsupported("periodic/RL/postcondition/possible hooks")
            ),
            "suffix={suffix}, failure={failure:?}, cfg={config:?}"
        );
        assert_eq!(failure.workers.expect("no worker").used, 0);
        assert!(failure.evidence.is_none());
    }
}
#[test]
fn malformed_source_and_sequential_requests_have_no_execution_credit() {
    let source = "---- MODULE Malformed ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    let (ir, cfg) = inputs(source, "INIT Init\nNEXT Next\nPROPERTY Live\n");
    let mut json = serde_json::to_value(ir.as_ref()).expect("IR");
    json["defs"][0]["body"] = serde_json::json!(u32::MAX);
    let malformed = Arc::new(serde_json::from_value(json).expect("malformed public IR"));
    let failure = tlc_temporal::check_final(
        &malformed,
        &cfg,
        &CheckOptions::tlc(0, 0),
        Limits::default(),
    )
    .expect_err("malformed references before binder");
    assert!(matches!(
        failure.error,
        tlc_temporal::Error::Unsupported("pre-bind definition/frame references")
    ));
    assert_eq!(failure.workers.expect("no worker").used, 0);
    assert!(failure.evidence.is_none());
    let options = CheckOptions {
        sequential: true,
        workers: 8,
        ..CheckOptions::tlc(0, 0)
    };
    let failure = tlc_temporal::check_final(&ir, &cfg, &options, Limits::default())
        .expect_err("uncertified inline observer path");
    assert_eq!(failure.workers.expect("no worker").used, 0);
    assert!(failure.evidence.is_none());
}
