//! Final temporal API boundaries, independently validated lossless reports.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_api::temporal::{self, Error, Outcome};
use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::Limits;
use dive_store::StoreMode;
use dive_trace::Validation;
use std::sync::Arc;

fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let name = source.split_whitespace().nth(2).expect("module");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{a:?}");
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(cfg).into_config().expect("cfg"),
    )
}
fn counts(stdout: &str) -> (u64, u64, u64) {
    let line = stdout
        .lines()
        .find(|s| s.contains("states generated,") && s.contains("distinct states found"))
        .expect("counts");
    let ns: Vec<u64> = line
        .split_whitespace()
        .filter_map(|s| s.replace(',', "").parse().ok())
        .collect();
    let depth = stdout
        .lines()
        .find(|s| s.contains("depth of the complete state graph search"))
        .expect("depth")
        .split_whitespace()
        .last()
        .expect("number")
        .trim_end_matches('.')
        .parse()
        .expect("number");
    (ns[0], ns[1], depth)
}

#[test]
fn pinned_final_runs_preserve_counts_and_return_only_validated_periodic_claims() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvEAPrefix",
        "LvStutterActionConstraint",
        "LvWFIntermittent",
        "LvStutterEnd",
        "LvWeakFair",
        "LvSFIntermittent",
        "LvConstraint",
    ] {
        let source =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.tla")))
                .expect("spec");
        let cfg =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.cfg")))
                .expect("cfg");
        let stdout = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .expect("oracle");
        let (ir, cfg) = input(&source, &cfg);
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for workers in [1, 8] {
                let opts = CheckOptions {
                    store,
                    workers,
                    ..CheckOptions::tlc(0, 19)
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert!(
                            name == "LvWeakFair"
                                || name == "LvSFIntermittent"
                                || name == "LvConstraint",
                            "{name}"
                        );
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_ne!(name, "LvWeakFair");
                        assert_ne!(name, "LvSFIntermittent");
                        assert_ne!(name, "LvConstraint");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, v.candidate().properties);
                        let model =
                            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                                .expect("fresh validation model");
                        let doc = dive_trace::TraceDoc::parse(&v.document().to_json().to_string())
                            .expect("lossless serialization");
                        assert_eq!(
                            dive_trace::validate_doc(&model, &doc, &cfg.model_values, None)
                                .0
                                .validation,
                            Validation::Valid,
                            "{name}"
                        );
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
                assert_eq!((c.workers.requested, c.workers.used), (workers, 1));
                assert_eq!(c.options.seed, 19);
                assert_eq!(c.options.store, store);
            }
        }
    }
}

#[test]
fn default_profile_attributes_all_false_properties_and_repeat_runs_rebind() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == <>[](~x)\nR == []<>x\n====\n";
    let (ir, cfg) = input(src, "SPECIFICATION Spec\nPROPERTY P Q R\n");
    for _ in 0..2 {
        let r = temporal::check_final(&ir, &cfg, &CheckOptions::dive(17), Limits::default())
            .expect("run");
        let Outcome::Violation(v) = r.outcome() else {
            panic!("two properties false")
        };
        assert_eq!(v.document().properties, ["P", "Q"]);
        assert_eq!(v.validation().validation, Validation::Valid);
        assert_eq!(v.candidate().exploration.options.seed, 17);
    }
}

#[test]
fn safety_stops_retain_trace_statistics_and_assumption_failures() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\nP == []<>x\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n");
    let opts = CheckOptions::tlc(0, 0);
    let r = temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect("safety stop");
    let Outcome::Safety(r) = r.outcome() else {
        panic!("invariant takes precedence")
    };
    assert!(matches!(r.verdict, Verdict::InvariantViolated { .. }));
    assert_eq!(
        (
            r.stats.expect("counters").generated,
            r.stats.expect("counters").distinct
        ),
        (2, 2)
    );
    assert_eq!(r.counterexample.as_ref().expect("trace").states.len(), 2);
    let (ir, cfg) = input(
        &src.replace("VARIABLE x", "VARIABLE x\nASSUME FALSE"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    let r = temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect("assumption stop");
    assert!(
        matches!(r.outcome(), Outcome::Safety(r) if matches!(r.verdict, Verdict::AssumptionFalse { .. }))
    );
}

#[test]
fn bounds_and_unsupported_scopes_never_become_holds() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nF(a) == []<>a\nP == F(x)\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\nPROPERTY P\n");
    assert!(matches!(
        temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default()),
        Err(Error::Analysis(_))
    ));
    let (ir, cfg) = input(
        &src.replace("P == F(x)", "P == []<>x"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    let r = temporal::check_final(
        &ir,
        &cfg,
        &CheckOptions::dive(0),
        Limits {
            capture: dive_check::capture::Limits {
                states: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("capture limit result");
    assert!(
        matches!(r.outcome(), Outcome::Safety(r) if matches!(r.verdict, Verdict::ResourceLimit { .. }))
    );
    let e = temporal::check_final(
        &ir,
        &cfg,
        &CheckOptions::dive(0),
        Limits {
            properties: 0,
            ..Default::default()
        },
    )
    .expect_err("planning bound");
    assert_eq!(e.code(), "DIVE-L0034");
}

#[test]
fn undecidable_independent_enabledness_is_a_refusal_not_a_violation() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = [n \\in {0} |-> 0]\nNext == x' \\in [{0} -> {0}] /\\ x' # x'\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>(x # x)\n====\n";
    let (ir, cfg) = input(
        src,
        "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
    );
    let e = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect_err("independent procedure cannot cover a lazy function set");
    assert!(
        matches!(&e, Error::Validation { report, .. } if matches!(report.validation, Validation::Unsupported(_))),
        "{e:?}"
    );
    assert_eq!(e.code(), "DIVE-A0032");
}
