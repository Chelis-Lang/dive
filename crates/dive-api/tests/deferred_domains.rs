//! Deferred finite filters/maps: pinned complete counts and independently validated
//! temporal counterexamples on Core. These are final/short checks, not strict
//! TLC lassos. They ran on the retired reference final route until #791;
//! exact seen storage with temporal properties is a named Core refusal.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use core_final::Outcome;
use dive_model::{BindOptions, Pred};
use dive_runtime::CheckOptions;
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
                {
                    let store = StoreMode::Fingerprint;
                    for workers in [1, 4, 8] {
                        let options = CheckOptions {
                            store,
                            workers,
                            ..profile.clone()
                        };
                        let report = core_final::check(&source, &config, &options).expect(&name);
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
/// recursive domain conservatively), so no violation is claimed: Core refuses
/// the temporal validation failure by name.
#[test]
fn startup_fixed_recursive_domains_plan_and_never_claim_unvalidated_words() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/finite-deferred-audit/specs");
    for suffix in ["Holds", "Fails"] {
        let name = format!("FdRecursivePerm{suffix}");
        let source =
            std::fs::read_to_string(root.join(&name).join(format!("{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(&name).join(format!("{name}.cfg"))).expect("config");
        for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let result = core_final::check(&source, &config, &options);
            if suffix == "Holds" {
                let run = result.expect("the fixed domain is planned");
                assert!(
                    matches!(run.outcome(), Outcome::Holds(_)),
                    "{name}: {:?}",
                    run.outcome()
                );
            } else {
                let stop = result.expect_err("an unvalidated word is not a violation");
                refused::assert_refused(
                    &stop.json(),
                    stop.exit_code(),
                    refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
                );
            }
        }
    }
}
