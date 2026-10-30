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

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "retain the original refusal matrix with exact typed source and execution evidence"
)]
fn recursive_local_function_domains_remain_explicitly_refused() {
    use dive_api::temporal::{Error, Stage};
    use dive_liveness::{analyze::AnalysisError, plan::PlanningError};

    fn refusal(error: &Error) -> &dive_liveness::plan::Refusal {
        let Error::Analysis(AnalysisError::Planning(PlanningError::Unsupported(refusal))) = error
        else {
            panic!("expected exact planning Unsupported, got {error:?}");
        };
        refusal
    }

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
        let property = ir.def(ir.root_def("Live").expect("property")).body;
        let dive_ir::Expr::Quant { bounds, .. } = ir.expr(property) else {
            panic!("actual temporal quantifier");
        };
        let domain = bounds[0].domain.expect("original Domain operand");
        for profile in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let options = CheckOptions {
                    store,
                    ..profile.clone()
                };
                let legacy = temporal::check_final(&ir, &cfg, &options, Limits::default())
                    .expect_err("recursive temporal domain not established constant");
                let failure =
                    temporal::check_final_with_evidence(&ir, &cfg, &options, Limits::default())
                        .expect_err("no outcome or word before planning completes");
                let actual = refusal(failure.error());
                assert_eq!(refusal(&legacy), actual);
                assert_eq!(legacy.code(), "DIVE-L0020");
                assert_eq!(failure.error().code(), legacy.code());
                assert_eq!(failure.error().to_string(), legacy.to_string());
                // Recursive self/formals remain conservative state unknowns,
                // even though this particular recursive call is closed. This
                // is an analysis limitation, not an actual state dependency.
                assert_eq!(actual.detail, "nonconstant predicate quantifier domain");
                assert_eq!(actual.request.expr, property);
                assert_eq!(actual.cause.expr, domain);
                assert_eq!(actual.request.origin, ir.node(property).origin);
                assert_eq!(actual.cause.origin, ir.node(domain).origin);
                assert_eq!(actual.request.origin.begin, (14, 9));
                assert_eq!(actual.request.origin.end, (14, 46));
                assert_eq!(actual.cause.origin.begin, (14, 18));
                assert_eq!(actual.cause.origin.end, (14, 23));
                assert_eq!(
                    actual.request.location,
                    format!("line 14, col 9 to line 14, col 46 of module {name}")
                );
                assert_eq!(
                    actual.cause.location,
                    format!("line 14, col 18 to line 14, col 23 of module {name}")
                );
                assert_eq!(
                    failure.stage(),
                    Stage::Analysis(dive_liveness::analyze::Stage::Planning)
                );
                assert_eq!(failure.capture_completed(), Some(false));
                assert!(failure.exploration().is_none());
                assert!(failure.action_evaluation().is_none());
                assert_eq!(
                    failure.workers(options.workers).expect("known width").used,
                    0
                );
            }
        }
    }
}
