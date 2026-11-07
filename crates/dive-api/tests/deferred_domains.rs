//! Deferred finite filters/maps: pinned complete counts and independently validated
//! temporal counterexamples. These are final/short checks, not strict TLC lassos.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::analyze::Limits;
use dive_model::{BindOptions, Pred};
use dive_store::StoreMode;
use dive_trace::Validation;
use dive_trace::temporal::evaluate_lasso;
use dive_value::Value;
use std::{path::Path, sync::Arc};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the complete conformance and independent witness matrix is kept together"
)]
fn deferred_domains_match_final_truth_counts_and_independent_word_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/finite-deferred-audit");
    for kind in ["Filter", "Map", "Capture"] {
        for holds in [false, true] {
            let name = format!("Fd{kind}{}", if holds { "Holds" } else { "Fails" });
            let source =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.tla")))
                    .expect("source");
            let config =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.cfg")))
                    .expect("config");
            let stdout = std::fs::read_to_string(root.join("tlc").join(format!("{name}.txt")))
                .expect("oracle");
            let exit = std::fs::read_to_string(root.join("tlc").join(format!("{name}.exit")))
                .expect("exit");
            assert_eq!(exit.trim(), if holds { "0" } else { "13" });
            let expected = if holds { (3, 2, 2) } else { (2, 1, 1) };
            assert!(
                stdout.contains(&format!(
                    "{} states generated, {} distinct states found",
                    expected.0, expected.1
                )),
                "{stdout}"
            );
            assert!(
                stdout.contains(&format!(
                    "depth of the complete state graph search is {}.",
                    expected.2
                )),
                "{stdout}"
            );
            let a = dive_sema::analyze_source(
                &format!("{name}.tla"),
                &source,
                &dive_sema::Options::default(),
            );
            assert!(a.accepted(), "{a:?}");
            let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
            let cfg = dive_config::parse(&config).into_config().expect("cfg");
            for profile in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
                for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                    for workers in [1, 4, 8] {
                        let options = CheckOptions {
                            store,
                            workers,
                            ..profile.clone()
                        };
                        let report = temporal::check_final(&ir, &cfg, &options, Limits::default())
                            .expect(&name);
                        let capture = match report.outcome() {
                            Outcome::Holds(c) => {
                                assert!(holds, "{name}");
                                c.as_ref()
                            }
                            Outcome::Violation(v) => {
                                assert!(!holds, "{name}");
                                assert_eq!(v.validation().validation, Validation::Valid);
                                &v.candidate().exploration
                            }
                            Outcome::NativeHolds(_)
                            | Outcome::NativeViolation(_)
                            | Outcome::NativePrefixViolation(_) => {
                                panic!("explicit reference API must retain its legacy result type")
                            }
                            Outcome::PrefixViolation(_) => {
                                panic!("explicit Final cannot return a prefix")
                            }
                            Outcome::Safety(s) => panic!("{name}: {:?}", s.verdict),
                        };
                        assert_eq!(
                            (
                                capture.stats.generated,
                                capture.stats.distinct,
                                capture.stats.depth
                            ),
                            expected,
                            "{name}"
                        );
                    }
                }
                let bind_options = if profile.profile == dive_eval::Profile::Tlc {
                    BindOptions::tlc()
                } else {
                    BindOptions::dive()
                };
                let model = dive_model::bind(ir.clone(), &cfg, &bind_options)
                    .expect("fresh independent model");
                let d = ir.root_def("Live").expect("property");
                let p = Pred::new(
                    ir.def(d).body,
                    model.evaluator.scope(d).expect("valid scope"),
                );
                let word: Vec<dive_trace::State> = [false, true]
                    .into_iter()
                    .map(|v| Arc::from([Some(Value::from(v))]))
                    .collect();
                assert!(evaluate_lasso(&model, &p, &word, 0).expect("fair alternating word")[0]);
                assert!(
                    !evaluate_lasso(&model, &p, &word[1..], 0).expect("unfair stutter mutant")[0],
                    "{name}"
                );
            }
        }
    }
}

/// `Domain == PermSeqs({1,2})` is a zero-arity constant-level definition, so
/// start-up fixes its value (`processConstantDefns`) and the planner reads it
/// as a constant, though the recursive local function itself would be an
/// unknown. The holding property is decided; the failing one finds a
/// candidate the independent validator cannot yet decide (it still treats the
/// recursive domain conservatively), so no violation is claimed.
#[test]
fn startup_fixed_recursive_domains_plan_and_never_claim_unvalidated_words() {
    use dive_api::temporal::{Error, Stage};

    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/finite-deferred-audit/specs");
    for suffix in ["Holds", "Fails"] {
        let name = format!("FdRecursivePerm{suffix}");
        let source =
            std::fs::read_to_string(root.join(&name).join(format!("{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(&name).join(format!("{name}.cfg"))).expect("config");
        let a = dive_sema::analyze_source(
            &format!("{name}.tla"),
            &source,
            &dive_sema::Options::default(),
        );
        assert!(a.accepted(), "{a:?}");
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg = dive_config::parse(&config).into_config().expect("cfg");
        for profile in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let options = CheckOptions {
                    store,
                    ..profile.clone()
                };
                let legacy = temporal::check_final(&ir, &cfg, &options, Limits::default());
                let evidence =
                    temporal::check_final_with_evidence(&ir, &cfg, &options, Limits::default());
                if suffix == "Holds" {
                    let legacy = legacy.expect("the fixed domain is planned");
                    let evidence = evidence.expect("the fixed domain is planned");
                    for outcome in [legacy.outcome(), evidence.outcome()] {
                        assert!(
                            matches!(outcome, Outcome::Holds(_) | Outcome::NativeHolds(_)),
                            "{name}: {outcome:?}"
                        );
                    }
                } else {
                    let legacy = legacy.expect_err("an unvalidated word is not a violation");
                    let failure = evidence.expect_err("an unvalidated word is not a violation");
                    for error in [&legacy, failure.error()] {
                        // Exactly the validator's named refusal of the
                        // recursive domain, for the planned property's word.
                        let Error::Validation { candidate, report } = error else {
                            panic!("{name}: expected the validation refusal, got {error:?}");
                        };
                        assert_eq!(candidate.target_property, "Live", "{name}");
                        assert_eq!(candidate.properties, ["Live"], "{name}");
                        assert_eq!(
                            report.validation,
                            Validation::Unsupported(
                                "nonconstant predicate quantifier domain".into()
                            ),
                            "{name}"
                        );
                        assert_eq!(error.code(), "DIVE-A0032", "{name}");
                    }
                    assert_eq!(failure.error().to_string(), legacy.to_string());
                    // Refused at validation after a complete capture: no
                    // action evaluation is claimed, and the one requested
                    // worker is the one used.
                    assert_eq!(failure.stage(), Stage::Validation, "{name}");
                    assert_eq!(failure.capture_completed(), Some(true), "{name}");
                    assert!(failure.action_evaluation().is_none(), "{name}");
                    assert_eq!(
                        failure.workers(options.workers).expect("known width").used,
                        1,
                        "{name}"
                    );
                }
            }
        }
    }
}
