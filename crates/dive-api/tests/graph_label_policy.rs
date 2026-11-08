//! Graph-label policy: Core's unlimited graph cells, and the explicit policy
//! of the periodic analysis entrypoints.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite policy assertions"
)]

use std::sync::Arc;

#[path = "support/core_final.rs"]
mod core_final;

use dive_liveness::analyze::{self, AnalysisError, GraphLabelPolicy, Limits};
use dive_liveness::product::AcceptancePolicy;
use dive_runtime::CheckOptions;

const CFG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

fn source(property: &str) -> String {
    format!(
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
    )
}

fn input(property: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = source(property);
    let analysis = dive_sema::analyze_source("M.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse(CFG).into_config().expect("config"),
    )
}

/// Core runs with no graph-label cell limit; the caller-set graph and word
/// cell ceilings belonged to the retired reference final route (#791).
#[test]
fn core_graph_labels_have_no_cell_limit_and_keep_validated_words() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            let options = CheckOptions {
                workers,
                ..options.clone()
            };
            let run = core_final::check(source("[]<>(x \\in {FALSE,TRUE})"), CFG, &options)
                .expect("holds");
            let core_final::Outcome::Holds(capture) = run.outcome() else {
                panic!("holding property: {:?}", run.outcome())
            };
            assert_eq!(
                (
                    capture.stats.distinct,
                    capture.stats.generated,
                    capture.stats.depth
                ),
                (2, 3, 2)
            );
            let run = core_final::check(source("[]<>x"), CFG, &options).expect("violation");
            let core_final::Outcome::Violation(word) = run.outcome() else {
                panic!("stuttering falsifies GF x: {:?}", run.outcome())
            };
            assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            assert_eq!(word.document().properties, ["P"]);
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
