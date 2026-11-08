//! D-005 uses full concrete identities and independently checked literal words.
#![allow(clippy::expect_used, clippy::panic, reason = "finite primary controls")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{BudgetSetting, CheckRequest, ExitConvention, SymmetryLiveness, TemporalMode};
use dive_runtime::CheckOptions;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn request(
    spec: PathBuf,
    policy: Option<SymmetryLiveness>,
    tlc: bool,
    workers: usize,
) -> CheckRequest {
    CheckRequest {
        file: spec,
        compat_tlc: tlc,
        symmetry_liveness: policy,
        seed: Some(0),
        fp: Some(0),
        workers: Some(workers),
        time_limit: BudgetSetting::Unlimited,
        max_memory: BudgetSetting::Unlimited,
        ..Default::default()
    }
}
fn p4() -> PathBuf {
    root().join("conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla")
}
fn token() -> PathBuf {
    root().join(
        "crates/dive-liveness/tests/fixtures/tlc-probes/specs/SymLiveSymProp/SymLiveSymProp.tla",
    )
}
fn check(request: &CheckRequest) -> Value {
    dive_api::check_file_engine(request, dive_api::Engine::Core)
        .expect("file check")
        .to_json_with_exit_convention(ExitConvention::Tlc)
}
fn tuple(report: &Value) -> (u64, u64, u64) {
    let s = &report["statistics"];
    (
        s["distinct"].as_u64().expect("distinct"),
        s["generated"].as_u64().expect("generated"),
        s["depth"].as_u64().expect("depth"),
    )
}

#[test]
fn declared_symmetry_true_temporal_runs_match_both_unreduced_primary_references() {
    for (file, reference) in [
        (p4(), "P4Unreduced/P4SymLive"),
        (token(), "SymmetricPropertyUnreduced/SymLiveSymProp"),
    ] {
        let primary: Value=serde_json::from_slice(&std::fs::read(root().join(format!(
            "conformance/golden/tlc-2026.10.06.014338/liveness-quotient-primary/specifications/{reference}.cfg.json"
        ))).expect("generated primary")).expect("primary JSON");
        for tlc in [false, true] {
            for workers in [1, 4, 8] {
                let policy = tlc.then_some(SymmetryLiveness::Unreduced);
                let report = check(&request(file.clone(), policy, tlc, workers));
                assert_eq!(report["verdict"], primary["result"]["verdict"]);
                assert_eq!(tuple(&report), (2, 3, 2));
                assert_eq!(
                    report["symmetry_liveness"],
                    json!({"declared":"Sym","requested":"unreduced",
                "genuine_temporal":true,"effective":"unreduced","scope":"whole-run"})
                );
                assert_eq!(
                    report["options"]["profile"],
                    if tlc { "tlc+overrides" } else { "dive" }
                );
                assert_eq!(
                    report["options"]["symmetry_liveness_source"],
                    if tlc { "explicit" } else { "profile-default" }
                );
                assert_eq!(report["options"]["workers"]["requested"], workers);
                assert_eq!(report["options"]["workers"]["used"], workers);
                assert_eq!(report["options"]["workers"]["sequential"], false);
                assert_eq!(report["execution"]["exhausted"], true);
                assert!(report["counterexample"].is_null());
            }
        }
    }
}

fn assert_symmetry_refused(report: &Value) {
    assert_eq!(report["verdict"], "unsupported", "{report}");
    assert_eq!(report["exit_code"], 3, "{report}");
    assert!(report["counterexample"].is_null(), "{report}");
    assert!(
        report["message"]
            .as_str()
            .is_some_and(|message| message.contains("SYMMETRY")),
        "{report}"
    );
}

#[test]
fn explicit_refusal_tlc_default_and_view_never_silently_use_unreduced_symmetry() {
    for tlc in [false, true] {
        for mode in [SymmetryLiveness::Refuse, SymmetryLiveness::Tlc] {
            let report = check(&request(p4(), Some(mode), tlc, 4));
            // Core refuses symmetry with temporal properties by name; the
            // retired reference engine's `symmetry_liveness` policy block
            // (#791, #910) is not part of Core's report.
            assert_symmetry_refused(&report);
        }
    }
    let report = check(&request(p4(), None, true, 1));
    assert_symmetry_refused(&report);
    assert_eq!(report["options"]["symmetry_liveness"], "tlc");
    assert_eq!(report["options"]["profile"], "tlc");
    for mode in [
        SymmetryLiveness::Unreduced,
        SymmetryLiveness::Refuse,
        SymmetryLiveness::Tlc,
    ] {
        for direction in ["Forward", "Reverse"] {
            let mut req=request(root().join("conformance/corpus/liveness-quotient-primary/specifications/ViewTwinTargets/ViewTwinTargets.tla"),Some(mode),false,1);
            req.config = Some(req.file.with_file_name(format!("{direction}.cfg")));
            let report = check(&req);
            assert_eq!(report["verdict"], "unsupported");
            assert!(report["counterexample"].is_null());
        }
    }
}

#[test]
fn concrete_violations_keep_literal_closure_fairness_and_private_fresh_replay() {
    let file = root().join(
        "crates/dive-liveness/tests/fixtures/tlc-probes/specs/SymLiveMissed/SymLiveMissed.tla",
    );
    for tlc in [false, true] {
        for workers in [1, 4, 8] {
            let report = check(&request(
                file.clone(),
                Some(SymmetryLiveness::Unreduced),
                tlc,
                workers,
            ));
            assert_eq!(report["verdict"], "liveness-violation");
            assert_eq!(tuple(&report), (3, 7, 2));
            assert_eq!(report["property"], "InfOftenA");
            let document =
                TraceDoc::from_json(&report["counterexample"]).expect("native lossless word");
            assert_eq!(report["counterexample"]["trace"]["version"], 1);
            let options = TraceOptions {
                compat_tlc: tlc,
                symmetry_liveness: Some(SymmetryLiveness::Unreduced),
                run: Some((0, 0)),
                ..Default::default()
            };
            let fresh = SpecModel::load(&file, &file.with_extension("cfg"), &options)
                .expect("fresh independent bind");
            assert_eq!(fresh.validate(&document).validation, Validation::Valid);
            // A reduced one-state representative cannot erase weak fairness.
            let mut forged = report["counterexample"]["trace"].clone();
            forged["states"].as_array_mut().expect("states").truncate(1);
            forged["ending"] = json!({"kind":"stuttering"});
            let forged = TraceDoc::from_json(&forged).expect("well-formed false word");
            assert!(matches!(
                fresh.validate(&forged).validation,
                Validation::Invalid(_)
            ));
            // Caller mutations cannot reach the Session's private original Recipe.
            let mut public = fresh.model().clone();
            public.symmetry = None;
            assert!(!public.unreduced_symmetry_liveness());
            assert_eq!(fresh.validate(&document).validation, Validation::Valid);
            let refused = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc: true,
                    run: Some((0, 0)),
                    ..Default::default()
                },
            )
            .expect("ordinary binder");
            assert!(matches!(
                refused.validate(&document).validation,
                Validation::Unsupported(_)
            ));
        }
    }
}

fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let analysis = dive_sema::analyze_source("Policy.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse(cfg).into_config().expect("cfg"),
    )
}
const SOURCE: &str = "---- MODULE Policy ----\nEXTENDS TLC\nCONSTANTS a,b\nVARIABLE x\nInit==x=a\nNext==x'=IF x=a THEN b ELSE a\nSpec==Init /\\ [][Next]_x /\\ WF_x(Next)\nSym==Permutations({a,b})\nP==[](x \\in {a,b})\nLive==<>(x=b)\n====\n";
const CONFIG: &str = "CONSTANTS a=a b=b\nSPECIFICATION Spec\nSYMMETRY Sym\nPROPERTY P\n";

#[test]
fn safety_roles_and_fairness_alone_keep_existing_reduction_and_declared_metadata() {
    for cfg in [CONFIG.to_owned(), CONFIG.replace("PROPERTY P\n", "")] {
        for mode in [
            SymmetryLiveness::Unreduced,
            SymmetryLiveness::Refuse,
            SymmetryLiveness::Tlc,
        ] {
            let mut opts = CheckOptions::dive(0);
            opts.symmetry_liveness = mode;
            let run = core_final::check(SOURCE, &cfg, &opts).expect("safety run");
            let core_final::Outcome::Safety(safety) = run.outcome() else {
                panic!("safety roles must not become liveness: {:?}", run.outcome())
            };
            assert!(matches!(safety.verdict, dive_runtime::Verdict::Success));
            let stats = safety.stats.expect("actual safety search");
            assert_eq!((stats.distinct, stats.generated, stats.depth), (1, 2, 1));
            // Core reports no symmetry-liveness resolution when no genuine
            // temporal property is checked; the reduced counts above show
            // the declared symmetry was applied (#791, #910).
            let json = run.report().to_json();
            assert!(json["symmetry_liveness"].is_null(), "{mode:?}: {json}");
        }
    }
}

#[test]
fn ignored_declaration_does_not_rebind_or_preempt_existing_role_errors() {
    let source = SOURCE.replace("P==[](x \\in {a,b})", "P==<>(x=b)");
    let missing = CONFIG.replace("SYMMETRY Sym", "SYMMETRY Missing");
    let run = core_final::check(&source, &missing, &CheckOptions::dive(0));
    let run = run.expect("ignored declaration lookup");
    assert!(matches!(run.outcome(), core_final::Outcome::Holds(_)));
    assert_eq!(
        run.report().to_json()["symmetry_liveness"]["declared"],
        "Missing"
    );
    let unknown = CONFIG.replace("PROPERTY P", "PROPERTY Unknown");
    let report = core_final::run(&source, &unknown, &CheckOptions::dive(0));
    let json = report.to_json();
    assert_eq!(json["verdict"], "model-error", "{json}");
    assert!(json["symmetry_liveness"].is_null(), "{json}");
    // Ordinary safety still looks up its actual declaration and preserves errors.
    let report = core_final::run(SOURCE, &missing, &CheckOptions::dive(0));
    assert_eq!(report.verdict(), "model-error", "{}", report.to_json());
}

#[test]
fn limits_are_not_verdicts_and_startup_metadata_is_not_a_bound_policy() {
    let mut req = request(p4(), None, false, 8);
    req.max_states = Some(1);
    let report = check(&req);
    // Only the retired reference engine stopped a temporal run at a search
    // cap (#791, #910).
    let exit = u8::try_from(report["exit_code"].as_u64().unwrap_or(255)).unwrap_or(255);
    refused::assert_refused(&report, exit, refused::TEMPORAL_CAP);
    req.max_states = None;
    req.max_memory = BudgetSetting::Explicit(1);
    let report = check(&req);
    assert_eq!(report["verdict"], "stopped");
    assert!(report["symmetry_liveness"].is_null());
    assert_eq!(report["options"]["symmetry_liveness"], "unreduced");
    req.max_memory = BudgetSetting::Unlimited;
    req.temporal = TemporalMode::FinalShort;
    let report = check(&req);
    // Only the retired reference engine ran bounded final checking.
    let exit = u8::try_from(report["exit_code"].as_u64().unwrap_or(255)).unwrap_or(255);
    refused::assert_refused(&report, exit, refused::FINAL_SHORT);
    req.temporal = TemporalMode::Ordinary;
    let report = check(&req);
    assert_eq!(report["verdict"], "success");
    assert_eq!(report["symmetry_liveness"]["scope"], "whole-run");
}

#[test]
fn full_run_policy_keeps_earlier_invariants_identical_to_the_unreduced_search() {
    let source = SOURCE.replace("P==[](x \\in {a,b})", "P==<>(x=b)\nInv==x=a");
    let declared = CONFIG.replace("PROPERTY P", "PROPERTY P\nINVARIANT Inv");
    let plain = declared.replace("SYMMETRY Sym\n", "");
    for workers in [1, 4, 8] {
        let mut opts = CheckOptions::dive(0);
        opts.workers = workers;
        let actual = core_final::check(&source, &declared, &opts).expect("safety stop");
        let reference = core_final::check(&source, &plain, &opts).expect("plain safety stop");
        let (core_final::Outcome::Safety(actual), core_final::Outcome::Safety(reference)) =
            (actual.outcome(), reference.outcome())
        else {
            panic!("earlier invariant must decide")
        };
        assert!(matches!(
            actual.verdict,
            dive_runtime::Verdict::InvariantViolated { initial: false, .. }
        ));
        assert_eq!(actual.verdict, reference.verdict);
        assert_eq!(actual.stats, reference.stats);
        assert_eq!(actual.workers, reference.workers);
        assert_eq!(
            actual
                .symmetry_liveness
                .as_ref()
                .expect("bound")
                .effective(),
            "unreduced"
        );
    }
}

#[test]
fn real_startup_output_occurs_once_and_role_errors_still_win_before_resolution() {
    let source = SOURCE
        .replace("Init==x=a", "Startup==Print(\"one-startup\",1)\nInit==x=a")
        .replace("P==[](x \\in {a,b})", "P==<>(x=b)");
    let (ir, cfg) = input(&source, CONFIG);
    let model =
        dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::dive()).expect("one binding");
    assert!(model.unreduced_symmetry_liveness());
    assert_eq!(model.output.iter().filter(|message|matches!(message,dive_model::Message::Print(text) if text.contains("one-startup"))).count(),1);
    let bad = dive_config::parse(&CONFIG.replace("PROPERTY P", "PROPERTY Missing"))
        .into_config()
        .expect("cfg grammar");
    let failure =
        dive_model::bind(ir, &bad, &dive_model::BindOptions::dive()).expect_err("real role error");
    assert_eq!(failure.output.iter().filter(|message|matches!(message,dive_model::Message::Print(text) if text.contains("one-startup"))).count(),1);
}
