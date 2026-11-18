//! Complete enumerable first stages followed by deterministic reductions.
#![allow(clippy::expect_used, reason = "bounded source regression controls")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_composition.json");

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_api::trace::{TraceDoc, Validation};
use dive_api::{ActionComposition, CheckRequest, ExitConvention, check_file, check_file_core_opts};
use serde_json::{Value, json};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

struct Input(CheckRequest);
impl Input {
    fn new(next: &str, invariant: &str, compat_tlc: bool) -> Self {
        let config = std::env::temp_dir().join(format!(
            "dive-core-composition-{}-{}.cfg",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(
            &config,
            format!("INIT Init\nNEXT {next}\nINVARIANT {invariant}\n"),
        )
        .expect("config");
        Self(CheckRequest {
            file: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/core-composition/Controls.tla"),
            config: Some(config),
            compat_tlc,
            action_composition: Some(ActionComposition::Tla),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        })
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        if let Some(config) = &self.0.config {
            let _ = std::fs::remove_file(config);
        }
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict": doc["verdict"], "property": doc["property"],
        "distinct": doc["statistics"]["distinct"], "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"], "exit": report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn deterministic_reduction_counts_only_final_successors() {
    let oracle: Value = serde_json::from_str(include_str!("fixtures/core-composition/oracle.json"))
        .expect("pinned oracle");
    assert_eq!(
        (
            oracle["generated"].as_u64(),
            oracle["distinct"].as_u64(),
            oracle["depth"].as_u64()
        ),
        (Some(13), Some(3), Some(2))
    );
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-composition/specs/Reduction/Reduction.tla");
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file: file.clone(),
            compat_tlc,
            action_composition: Some(ActionComposition::Tla),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let reference = check_file(&request).expect("reference");
        assert_eq!(
            observed(&reference),
            json!({"verdict":"success","property":null,"distinct":3,"generated":7,"depth":2,"exit":0})
        );
        for opts in ["", "-memo,-step-memo,-eager,-fold"] {
            let core = check_file_core_opts(&request, opts).expect("Core");
            agreed::check_eq(
                AGREED,
                "deterministic_reduction_counts_only_final_successors/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
        }
    }
}

#[test]
fn duplicate_intermediates_scoped_state_thunks_and_all_assigned_predicates() {
    for compat_tlc in [false, true] {
        for (next, invariant, distinct, generated) in [
            ("Duplicates", "Inv", 3, 13),
            ("Scoped", "ScopedInv", 3, 7),
            ("AllAssigned", "AllInv", 2, 3),
        ] {
            let input = Input::new(next, invariant, compat_tlc);
            for opts in ["", "-memo,-step-memo,-eager,-fold"] {
                let core = check_file_core_opts(&input.0, opts).expect("Core");
                let actual = observed(&core);
                assert_eq!(
                    actual,
                    json!({"verdict":"success","property":null,"distinct":distinct,"generated":generated,"depth":2,"exit":0}),
                    "{next}, compat={compat_tlc}, opts={opts}: {}",
                    core.to_json()
                );
            }
        }
    }
}

/// A step-mode thunk reading `x'` (`Ok`) is forced inside the first stage
/// of a composition in an action property. It must be re-evaluated for each
/// intermediate row: a value forced at `x' = 1` (FALSE) must not be reused at
/// the witness `x' = 2`, which would report a false violation.
#[test]
fn primed_step_thunks_are_not_reused_across_first_stage_rows() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-composition/specs/StepThunk/StepThunk.tla");
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file: file.clone(),
            compat_tlc,
            action_composition: Some(ActionComposition::Tla),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let reference = check_file(&request).expect("reference");
        assert_eq!(
            observed(&reference),
            json!({"verdict":"success","property":null,"distinct":3,"generated":4,"depth":3,"exit":0}),
            "{}",
            reference.to_json()
        );
        for opts in ["", "-memo,-step-memo,-eager,-fold"] {
            let core = check_file_core_opts(&request, opts).expect("Core");
            agreed::check_eq(
                AGREED,
                "primed_step_thunks_are_not_reused_across_first_stage_rows/0",
                &(observed(&core)),
                &(observed(&reference)),
            );
        }
    }
}

#[test]
fn unproved_shapes_and_composition_modes_keep_named_refusals() {
    for compat_tlc in [false, true] {
        for next in [
            "Incomplete",
            "Nondeterministic",
            "Pending",
            "Preassigned",
            "Effect",
            "Nested",
            "PrimedTail",
            "RepeatedTail",
        ] {
            let input = Input::new(next, "Inv", compat_tlc);
            let core = check_file(&input.0).expect("Core refusal").to_json();
            assert_eq!(core["verdict"], "unsupported", "{next}: {core:#}");
            assert!(
                core["message"].to_string().contains("composition"),
                "{next}: {core:#}"
            );
        }
        for mode in [ActionComposition::Off, ActionComposition::Tlc] {
            let mut input = Input::new("Basic", "Inv", compat_tlc);
            input.0.action_composition = Some(mode);
            let core = check_file(&input.0).expect("Core mode refusal").to_json();
            assert_eq!(core["verdict"], "unsupported", "{mode:?}: {core:#}");
        }
    }
}

#[test]
fn configured_startup_effects_cannot_become_pure_composition() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-composition/ConfiguredEffect.tla");
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            config: Some(file.with_extension("cfg")),
            file: file.clone(),
            compat_tlc,
            action_composition: Some(ActionComposition::Tla),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        for opts in ["", "-memo,-step-memo,-eager,-fold"] {
            let report = check_file_core_opts(&request, opts)
                .expect("configured effect refusal")
                .to_json();
            assert_eq!(report["verdict"], "unsupported", "{report:#}");
            assert!(
                report["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("composition with effects")),
                "{report:#}"
            );
        }
    }
}

#[test]
fn actual_opposite_and_forged_composed_words_use_independent_replay() {
    let source = include_str!("fixtures/core-composition/Controls.tla");
    for compat_tlc in [false, true] {
        for (invariant, final_y, generated) in [("Bad", 4, 3), ("Other", 2, 2)] {
            let input = Input::new("Basic", invariant, compat_tlc);
            let report = check_file(&input.0).expect("Core violation");
            let value = report.to_json();
            assert_eq!(
                observed(&report),
                json!({"verdict":"invariant-violation", "property":invariant,
                "distinct":generated, "generated":generated, "depth":2, "exit":12}),
                "{value:#}"
            );
            let analysis =
                dive_sema::analyze_source("Controls.tla", source, &dive_sema::Options::default());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("independent IR"));
            let cfg = dive_config::parse(
                &std::fs::read_to_string(input.0.config.as_ref().expect("config path"))
                    .expect("config text"),
            )
            .into_config()
            .expect("independent config");
            let mut options = if compat_tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::dive()
            };
            options.eval.action_composition = ActionComposition::Tla;
            let model = dive_model::bind(ir, &cfg, &options).expect("independent model");
            let mut document =
                TraceDoc::from_json(&value["counterexample"]["trace"]).expect("native word");
            assert_eq!(document.states[1]["y"], json!(final_y));
            assert_eq!(
                dive_trace::validate_doc(&model, &document, &[])
                    .0
                    .validation,
                Validation::Valid
            );
            document.states[1].insert("y".into(), json!(99));
            assert!(matches!(
                dive_trace::validate_doc(&model, &document, &[])
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
        }
    }
}
