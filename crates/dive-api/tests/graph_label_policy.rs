//! Explicit graph policy must preserve all legacy and other-owner boundaries.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite policy assertions"
)]

use std::sync::Arc;

use dive_api::temporal::{self, Error, Outcome};
use dive_check::CheckOptions;
use dive_liveness::analyze::{self, AnalysisError, GraphLabelPolicy, Limits};
use dive_liveness::formula::FormulaError;
use dive_liveness::product::AcceptancePolicy;

fn input(property: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
    );
    let analysis = dive_sema::analyze_source("M.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config"),
    )
}

#[test]
fn legacy_zero_and_exact_graph_ceilings_survive_the_additive_policy() {
    let (ir, cfg) = input("[]<>(x \\in {FALSE,TRUE})");
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let limits = Limits {
                atom_cells: 0,
                ..Limits::default()
            };
            let failed = temporal::check_final_with_policy(
                &ir,
                &cfg,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
            )
            .expect_err("legacy zero graph cells");
            assert!(matches!(
                failed.error(),
                Error::Analysis(AnalysisError::Limit("atom cells"))
            ));
            assert_eq!(
                failed
                    .configuration()
                    .expect("actual configuration")
                    .graph_labels,
                GraphLabelPolicy::Cells(0)
            );
            for cells in [0, 5, 6] {
                let result = temporal::check_final_with_policies(
                    &ir,
                    &cfg,
                    &options,
                    limits,
                    AcceptancePolicy::compact_default(),
                    GraphLabelPolicy::Cells(cells),
                );
                if cells < 6 {
                    assert!(matches!(
                        result.expect_err("one short").error(),
                        Error::Analysis(AnalysisError::Limit("atom cells"))
                    ));
                } else {
                    let report = result.expect("exact graph cells");
                    assert!(matches!(report.outcome(), Outcome::Holds(_)));
                    assert_eq!(
                        report.configuration().expect("actual cells").graph_labels,
                        GraphLabelPolicy::Cells(6)
                    );
                }
            }
            let report = temporal::check_final_with_policies(
                &ir,
                &cfg,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
                GraphLabelPolicy::NoCellLimit,
            )
            .expect("only graph limit removed");
            let Outcome::Holds(capture) = report.outcome() else {
                panic!("holding property")
            };
            assert_eq!(
                (
                    capture.stats.distinct,
                    capture.stats.generated,
                    capture.stats.depth
                ),
                (2, 3, 2)
            );
            let configured = report.configuration().expect("actual configuration");
            assert_eq!(configured.limits.atom_cells, 0);
            assert_eq!(configured.graph_labels, GraphLabelPolicy::NoCellLimit);
            // Even a default-valued caller request remains a finite policy.
            let legacy = temporal::check_final_with_policy(
                &ir,
                &cfg,
                &options,
                Limits::default(),
                AcceptancePolicy::compact_default(),
            )
            .expect("legacy default");
            assert_eq!(
                legacy.configuration().expect("legacy config").graph_labels,
                GraphLabelPolicy::Cells(8_000_000)
            );
        }
    }
}

#[test]
fn no_graph_count_limit_does_not_remove_attribution_or_word_guards() {
    let (ir, cfg) = input("[]<>x");
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for cells in [0, 1] {
            let limits = Limits {
                atom_cells: cells,
                ..Limits::default()
            };
            let failed = temporal::check_final_with_policies(
                &ir,
                &cfg,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
                GraphLabelPolicy::NoCellLimit,
            )
            .expect_err("unchanged word owners");
            assert_eq!(failed.capture_completed(), Some(true));
            assert_eq!(
                failed.stage(),
                temporal::Stage::Analysis(analyze::Stage::Attribution)
            );
            if cells == 0 {
                assert!(matches!(
                    failed.error(),
                    Error::Analysis(AnalysisError::Limit("attribution atom cells"))
                ));
            } else {
                assert!(matches!(
                    failed.error(),
                    Error::Analysis(AnalysisError::Tableau(FormulaError::Limit(
                        "word truth cells"
                    )))
                ));
            }
            let configured = failed.configuration().expect("actual custom settings");
            assert_eq!(configured.limits.atom_cells, cells);
            assert_eq!(configured.graph_labels, GraphLabelPolicy::NoCellLimit);
        }
        let report = temporal::check_final_with_policies(
            &ir,
            &cfg,
            &options,
            Limits::default(),
            AcceptancePolicy::compact_default(),
            GraphLabelPolicy::NoCellLimit,
        )
        .expect("certified witness");
        let Outcome::Violation(word) = report.outcome() else {
            panic!("stuttering falsifies GF x")
        };
        assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
        assert_eq!(word.document().properties, ["P"]);
        let model = dive_model::bind(
            ir.clone(),
            &cfg,
            &dive_model::BindOptions {
                eval: options.eval,
                symmetry_liveness: options.symmetry_liveness,
                no_deadlock: options.no_deadlock,
                host: None,
            },
        )
        .expect("model");
        let plan = dive_liveness::plan::lower_violation(
            &model,
            &model.property_claims()[0].pred,
            Limits::default().formula,
        )
        .expect("same configured plan");
        let word_cells = word.candidate().states.len() * plan.formula.nodes().len();
        let attribution_cells = word.candidate().states.len() * plan.atoms.len();
        assert!(word_cells > attribution_cells);
        for cells in [word_cells - 1, word_cells] {
            let result = temporal::check_final_with_policies(
                &ir,
                &cfg,
                &options,
                Limits {
                    atom_cells: cells,
                    ..Limits::default()
                },
                AcceptancePolicy::compact_default(),
                GraphLabelPolicy::NoCellLimit,
            );
            if cells < word_cells {
                assert!(matches!(
                    result.expect_err("word one short").error(),
                    Error::Analysis(AnalysisError::Tableau(FormulaError::Limit(
                        "word truth cells"
                    )))
                ));
            } else {
                let report = result.expect("exact word budget");
                let Outcome::Violation(word) = report.outcome() else {
                    panic!("certified exact-bound word")
                };
                assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            }
        }
    }
}

#[test]
fn periodic_entrypoints_keep_legacy_ceilings_and_explicit_new_policy() {
    use dive_liveness::analyze::periodic::{self, PeriodicAnalysis, Schedule};
    let (ir, cfg) = input("[]<>(x \\in {FALSE,TRUE})");
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let model = dive_model::bind(
                ir.clone(),
                &cfg,
                &dive_model::BindOptions {
                    eval: options.eval,
                    symmetry_liveness: options.symmetry_liveness,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            )
            .expect("model");
            let limits = Limits {
                atom_cells: 0,
                ..Limits::default()
            };
            let schedule = Schedule {
                min_states: 1,
                growth_factor: 1,
            };
            let old = periodic::analyze_periodic(
                &model,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
                schedule,
            )
            .expect_err("legacy graph ceiling");
            assert!(matches!(
                old.failure.failure().error,
                AnalysisError::Limit("atom cells")
            ));
            let (analysis, checks) = periodic::analyze_periodic_with_policies(
                &model,
                &options,
                limits,
                AcceptancePolicy::compact_default(),
                GraphLabelPolicy::NoCellLimit,
                schedule,
            )
            .expect("explicit graph policy");
            let PeriodicAnalysis::Final(analyze::Analysis::Holds(capture)) = analysis else {
                panic!("only exhausted final graph holds")
            };
            assert!(checks.exhausted && checks.final_check);
            assert_eq!(
                (
                    capture.stats.distinct,
                    capture.stats.generated,
                    capture.stats.depth
                ),
                (2, 3, 2)
            );
        }
    }
}

#[test]
fn explicit_no_cell_policy_cannot_turn_actual_exhaustion_into_a_cell_refusal() {
    let (ir, cfg) = input("[]<>(x \\in {FALSE,TRUE})");
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        options.budget = dive_check::budget::Budget::new(Some(std::time::Duration::ZERO), None);
        let failed = temporal::check_final_with_policies(
            &ir,
            &cfg,
            &options,
            Limits::default(),
            AcceptancePolicy::compact_default(),
            GraphLabelPolicy::NoCellLimit,
        )
        .expect_err("real time exhaustion");
        assert!(matches!(
            failed.error(),
            Error::Analysis(AnalysisError::Stopped(_))
        ));
        assert_eq!(failed.stage(), temporal::Stage::Startup);
        assert_eq!(failed.capture_completed(), Some(false));
        assert!(failed.exploration().is_none());
        assert_eq!(
            failed
                .configuration()
                .expect("configured before startup")
                .graph_labels,
            GraphLabelPolicy::NoCellLimit
        );
    }
}
