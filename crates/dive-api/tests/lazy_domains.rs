//! Finite lazy domains: pinned complete counts and independently validated
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
    reason = "retain the complete existing final-domain matrix and explicit nonexhaustive-result rejection"
)]
fn lazy_domain_oracles_match_final_truth_counts_and_independent_word_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lazy-domains");
    for kind in ["Function", "Record", "Product", "Subset"] {
        for holds in [false, true] {
            let name = format!("Ld{kind}{}", if holds { "Holds" } else { "Fails" });
            let source =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.tla")))
                    .expect("source");
            let config =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.cfg")))
                    .expect("config");
            let stdout = std::fs::read_to_string(
                root.join("tlc-2026.10.06.014338")
                    .join(format!("{name}.stdout")),
            )
            .expect("oracle");
            let exit = std::fs::read_to_string(
                root.join("tlc-2026.10.06.014338")
                    .join(format!("{name}.exit")),
            )
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
                            Outcome::PrefixViolation(prefix) => {
                                panic!("final API returned a prefix: {prefix:?}")
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
                    !evaluate_lasso(&model, &p, &word[..1], 0).expect("unfair stutter mutant")[0]
                );
            }
        }
    }
}
