//! Actual bounded safety route, stopping facts and independent replay controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::{CheckOptions, Verdict};
use dive_liveness::phase_graph::Limits;
use dive_trace::Validation;
use std::sync::Arc;
fn inputs(name: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-final-safety/models")
        .join(name);
    let source = std::fs::read_to_string(dir.join(format!("{name}.tla"))).expect("source");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        &source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
    (ir, dive_config::parse(&cfg).into_config().expect("cfg"))
}
fn assert_primary(name: &str, result: &dive_check::CheckResult) {
    let primary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../conformance/probes/tlc-final-safety/primary.json"),
        )
        .expect("primary capture manifest"),
    )
    .expect("primary JSON");
    let pinned = primary["rows"]
        .as_array()
        .expect("selection")
        .iter()
        .find(|r| r["model"] == name)
        .expect("selected row");
    assert_eq!(
        result.verdict.tlc().expect("deciding class").code,
        u32::try_from(pinned["code"].as_u64().expect("code")).expect("u32 code")
    );
    if pinned["statistics"].is_object() {
        let stats = result.stats.expect("actual counts");
        assert_eq!(
            stats.generated,
            pinned["statistics"]["generated"]
                .as_u64()
                .expect("generated")
        );
        assert_eq!(
            stats.distinct,
            pinned["statistics"]["distinct"].as_u64().expect("distinct")
        );
        assert_eq!(
            stats.left_on_queue,
            pinned["statistics"]["left_on_queue"]
                .as_u64()
                .expect("queue")
        );
        assert_eq!(stats.depth, pinned["depth"].as_u64().expect("depth"));
    } else {
        assert!(!result.tlc_prints_stats);
    }
}
#[test]
fn safety_only_uses_real_engine_and_independent_finite_replay() {
    for (name, kind) in [
        ("SafetyHold", "success"),
        ("SafetyIf", "success"),
        ("SafetyInvariant", "invariant-violation"),
        ("SafetyInitial", "invariant-violation"),
        ("SafetyAction", "action-property-violation"),
        ("SafetyPropertyInitial", "property-violation"),
    ] {
        let (ir, cfg) = inputs(name);
        for workers in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let report =
                tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
            let Outcome::Safety(safety) = report.outcome() else {
                panic!("safety input incorrectly produced temporal evidence")
            };
            let result = safety.result();
            assert_eq!(result.verdict.kind(), kind, "{name} workers={workers}");
            assert_primary(name, result);
            assert_eq!(result.workers.requested, workers);
            if kind == "success" {
                assert_eq!(result.workers.used, workers);
                assert!(safety.document().is_none());
                let stats = result.stats.expect("actual stats");
                assert_eq!(
                    (
                        stats.generated,
                        stats.distinct,
                        stats.depth,
                        stats.left_on_queue
                    ),
                    (4, 3, 3, 0)
                );
            } else {
                assert_eq!(
                    safety.validation().expect("fresh replay").validation,
                    Validation::Valid
                );
                let document = safety.document().expect("JSON replay");
                let replay = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                    .expect("fresh model");
                let mut mutant = document.clone();
                mutant.length = Some(document.states.len() + 1);
                assert!(matches!(
                    dive_trace::validate_doc(&replay, &mutant, &cfg.model_values, None)
                        .0
                        .validation,
                    Validation::Invalid(_)
                ));
                let mut mutant = document.clone();
                mutant.states[0].insert("x".into(), serde_json::json!(99));
                assert!(matches!(
                    dive_trace::validate_doc(&replay, &mutant, &cfg.model_values, None)
                        .0
                        .validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
#[test]
fn bounds_do_not_become_safety_success_or_temporal_credit() {
    let (ir, cfg) = inputs("SafetyInfinite");
    for workers in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers,
            ..CheckOptions::tlc(0, 0)
        };
        let mut limits = Limits::default();
        limits.capture.states = 3;
        let report = tlc_temporal::check_final(&ir, &cfg, &opts, limits).expect("bounded receipt");
        let Outcome::Safety(safety) = report.outcome() else {
            panic!("no temporal result")
        };
        assert!(matches!(
            safety.result().verdict,
            Verdict::ResourceLimit { .. }
        ));
        assert!(safety.document().is_none());
        assert_eq!(safety.limits().capture, limits.capture);
        assert_eq!(safety.limits().workers, limits.workers);
        assert_eq!(safety.result().workers.requested, workers);
        // Collector refuses the fourth graph representative after the ordinary
        // engine inserts its deciding state; stopping counters include it.
        assert_eq!(safety.result().stats.expect("actual prefix").distinct, 4);
    }
}
#[test]
fn successful_api_receipt_retains_exact_caller_limits() {
    let (ir, cfg) = inputs("SafetyHold");
    let limits = Limits {
        capture: dive_check::capture::Limits {
            states: 3,
            edges: 13,
            generated: 9,
            successors_per_unit: 7,
        },
        workers: 4,
        graph: dive_liveness::product::Limits {
            vertices: 127,
            acceptance_cells: 0,
            ..dive_liveness::product::Limits::default()
        },
        phase_l: dive_liveness::phase_l::Limits {
            work: 0,
            ..dive_liveness::phase_l::Limits::default()
        },
    };
    let report = tlc_temporal::check_final(&ir, &cfg, &CheckOptions::tlc(0, 0), limits)
        .expect("completed safety receipt");
    let Outcome::Safety(safety) = report.outcome() else {
        panic!("no temporal product")
    };
    assert_eq!(safety.limits().capture, limits.capture);
    assert_eq!(safety.limits().workers, limits.workers);
    assert_eq!(safety.limits().graph.vertices, 127);
    assert_eq!(safety.limits().graph.acceptance_cells, 0);
    assert_eq!(safety.limits().phase_l.work, 0);
    assert_eq!(safety.result().verdict, Verdict::Success);
    assert_eq!(safety.result().stats.expect("completed counts").distinct, 3);
    assert!(safety.document().is_none());
}

#[test]
fn unproved_constraint_property_replay_refuses_before_pool_work() {
    for name in ["SafetyConstrainedInitial", "SafetyConstrainedAction"] {
        let (ir, cfg) = inputs(name);
        let fail =
            tlc_temporal::check_final(&ir, &cfg, &CheckOptions::tlc(0, 0), Limits::default())
                .expect_err("replay boundary");
        assert!(matches!(
            fail.error,
            tlc_temporal::Error::Unsupported(
                "finite implied-property replay with state/action constraints is not certified"
            )
        ));
        assert_eq!(fail.workers.expect("actual admission").used, 0);
    }
}
#[test]
fn deadlock_cannot_earn_finite_violation_credit_without_independent_cover() {
    let (ir, cfg) = inputs("SafetyDeadlock");
    let report = tlc_temporal::check_final(&ir, &cfg, &CheckOptions::tlc(0, 0), Limits::default());
    match report {
        Ok(r) => {
            let Outcome::Safety(s) = r.outcome() else {
                panic!("safety route")
            };
            assert_eq!(s.result().verdict, Verdict::Deadlock);
            assert_eq!(
                s.validation().expect("independent cover").validation,
                Validation::Valid
            );
        }
        Err(e) => {
            let tlc_temporal::Error::SafetyValidation { result, report, .. } = e.error else {
                panic!("actual replay refusal expected: {e}")
            };
            assert_eq!(result.verdict, Verdict::Deadlock);
            assert!(matches!(report.validation, Validation::Unsupported(_)));
        }
    }
}

#[test]
fn route_authority_comes_from_fresh_bound_source_not_empty_report_names() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/probes");
    let safety = root.join("tlc-final-safety/models/SafetyHold/SafetyHold");
    assert!(
        tlc_temporal::safety_route_file(
            &safety.with_extension("tla"),
            &safety.with_extension("cfg"),
            &[]
        )
        .expect("admission")
    );
    let temporal = root.join("tlc-final-check/models/FinalState/FinalState");
    assert!(
        !tlc_temporal::safety_route_file(
            &temporal.with_extension("tla"),
            &temporal.with_extension("cfg"),
            &[]
        )
        .expect("admission")
    );
}
