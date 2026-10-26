//! Configured safety roles retain the ordinary engine and one startup stream.
use std::sync::Arc;

use dive_api::temporal::{self, Outcome};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_liveness::analyze::Limits;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Input = (String, Arc<dive_ir::Ir>, dive_config::ModelConfig);

fn input(body: &str, cfg: &str) -> Result<Input, Box<dyn std::error::Error>> {
    let source = format!("---- MODULE Roles ----\nEXTENDS Naturals,TLC\n{body}\n====\n");
    let analysis = dive_sema::analyze_source("Roles.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(cfg)
        .into_config()
        .ok_or("configuration")?;
    Ok((source, ir, config))
}

fn equal(actual: &CheckResult, ordinary: &CheckResult) {
    assert_eq!(actual.verdict, ordinary.verdict);
    assert_eq!(actual.verdict.tlc(), ordinary.verdict.tlc());
    assert_eq!(actual.binding_model_error, ordinary.binding_model_error);
    assert_eq!(actual.stats, ordinary.stats);
    assert_eq!(actual.workers, ordinary.workers);
    assert_eq!(actual.output, ordinary.output);
    assert_eq!(actual.warnings, ordinary.warnings);
    assert_eq!(actual.notes, ordinary.notes);
    assert_eq!(actual.var_names, ordinary.var_names);
    assert_eq!(actual.tlc_var_order, ordinary.tlc_var_order);
    assert_eq!(actual.tlc_prints_stats, ordinary.tlc_prints_stats);
    assert_eq!(
        format!("{:?}", actual.counterexample),
        format!("{:?}", ordinary.counterexample)
    );
}

#[test]
fn configured_init_invariant_action_and_refinement_roles_are_actually_checked() -> TestResult {
    for property in [
        "P == x=0",
        "P == [](x<=2)",
        "P == [][x'>=x]_x",
        "P == x=0 /\\ [](x<=2) /\\ [][x'>=x]_x",
        "P == TRUE",
    ] {
        let (_, ir, cfg) = input(
            &format!("VARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n{property}"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        for mut opts in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for workers in [1, 4] {
                opts.workers = workers;
                let ordinary = dive_check::check(&ir, &cfg, &opts);
                let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
                let Outcome::Safety(actual) = report.outcome() else {
                    return Err("configured safety roles must use the ordinary result".into());
                };
                equal(actual, &ordinary);
                assert_eq!(actual.verdict, Verdict::Success);
                let stats = actual.stats.ok_or("statistics")?;
                assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 3, 3));
            }
        }
    }
    Ok(())
}

#[test]
fn finite_property_words_are_valid_and_forged_values_are_invalid() -> TestResult {
    for (body, initial) in [
        ("VARIABLE x\nInit == x=0\nNext == x'=x\nP == x=1", true),
        (
            "VARIABLE x\nInit == x=0\nNext == x'=(x+1)%2\nP == [][x'=x+1]_x",
            false,
        ),
    ] {
        let config = "INIT Init\nNEXT Next\nPROPERTY P\n";
        let (source, ir, cfg) = input(body, config)?;
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let ordinary = dive_check::check(&ir, &cfg, &opts);
            let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("finite safety result".into());
            };
            equal(actual, &ordinary);
            if initial {
                assert!(
                    matches!(&actual.verdict, Verdict::PropertyViolatedInitially { property } if property=="P")
                );
            } else {
                assert!(
                    matches!(&actual.verdict, Verdict::ActionPropertyViolated { property } if property=="P")
                );
            }
            let states = &actual.counterexample.as_ref().ok_or("witness")?.states;
            assert_eq!(states.len(), if initial { 1 } else { 3 });
            let values = states
                .iter()
                .map(|state| {
                    Ok(serde_json::json!({"x":dive_trace::value_json::to_json(&state.values[0],opts.eval.ctx)?}))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let doc = TraceDoc::behavior(values)?.with_ending(Ending::Property("P".into()));
            let validator = SpecModel::from_source(
                "Roles.tla",
                &source,
                config,
                &TraceOptions {
                    compat_tlc: opts.profile == dive_eval::Profile::Tlc,
                    ..Default::default()
                },
            )?;
            assert_eq!(validator.validate(&doc).validation, Validation::Valid);
            let mut forged = doc;
            let last = forged.states.last_mut().ok_or("last state")?;
            last.insert("x".into(), serde_json::json!(if initial { 1 } else { 2 }));
            assert!(matches!(
                validator.validate(&forged).validation,
                Validation::Invalid(_)
            ));
        }
    }
    Ok(())
}

#[test]
fn startup_print_and_rng_consumption_reach_initial_search_without_reset() -> TestResult {
    let (_, ir, cfg) = input(
        "VARIABLE x\nBoot == Print(\"one-startup\",RandomElement(1..32))\nInit == x=RandomElement(1..32)\nNext == x'=IF x<Boot THEN x+1 ELSE x\nP == [](x<=32)",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for tlc in [false, true] {
        let mut observed_later_draw = false;
        for seed in 0..16 {
            let mut opts = if tlc {
                CheckOptions::tlc(0, seed)
            } else {
                CheckOptions::dive(seed)
            };
            opts.workers = 4;
            let ordinary = dive_check::check(&ir, &cfg, &opts);
            let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("state-only property with ordinary RNG".into());
            };
            equal(actual, &ordinary);
            assert_eq!(actual.verdict, Verdict::Success);
            assert_eq!(actual.output.len(), 1, "one actual startup Print");
            let stats = actual.stats.ok_or("statistics")?;
            assert!(stats.distinct <= 32);
            observed_later_draw |= stats.distinct > 1;
        }
        // Resetting the host after Boot makes Init draw Boot again, giving a
        // one-state search for every seed. This requires a real contrary case.
        assert!(observed_later_draw);
    }
    Ok(())
}

#[test]
fn mixed_real_temporal_obligations_never_become_safety_success() -> TestResult {
    for property in ["P == []<>(x=1)", "P == x=0 /\\ [][x'=x]_x /\\ []<>(x=1)"] {
        let (_, ir, cfg) = input(
            &format!("VARIABLE x\nInit == x=0\nNext == x'=x\n{property}"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
            let Outcome::Violation(counterexample) = report.outcome() else {
                return Err(
                    "real temporal obligation must retain validated periodic violation".into(),
                );
            };
            assert_eq!(counterexample.validation().validation, Validation::Valid);
            assert_ne!(counterexample.document().states.len(), 0);
            assert_eq!(counterexample.document().properties, ["P"]);
            let limits = Limits {
                properties: 0,
                ..Default::default()
            };
            assert!(temporal::check_final(&ir, &cfg, &opts, limits).is_err());
        }
    }
    Ok(())
}

#[test]
fn initial_invariant_and_postcondition_first_errors_match_the_real_engine() -> TestResult {
    for (body, extra, expected_kind) in [
        (
            "VARIABLE x\nInit == x=0\nNext == x'=x\nInv == x=1\nP == x=1",
            "INVARIANT Inv\n",
            "invariant-violation",
        ),
        (
            "VARIABLE x\nInit == x=0\nNext == x'=1\\div0\nP == x=1",
            "",
            "property-violation",
        ),
        (
            "ASSUME FALSE\nVARIABLE x\nInit == x=0\nNext == x'=1\\div0\nP == x=1",
            "",
            "assumption-failure",
        ),
        (
            "VARIABLE x\nInit == x=0\nNext == x'=x\nP == x=0\nPost == TRUE",
            "POSTCONDITION Post\n",
            "model-error",
        ),
    ] {
        let (_, ir, cfg) = input(body, &format!("INIT Init\nNEXT Next\nPROPERTY P\n{extra}"))?;
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let ordinary = dive_check::check(&ir, &cfg, &opts);
            let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("safety error priority".into());
            };
            equal(actual, &ordinary);
            assert_eq!(actual.verdict.kind(), expected_kind);
            assert!(
                !actual.binding_model_error,
                "these errors occur after binding"
            );
        }
    }
    Ok(())
}

#[test]
fn excluded_preflight_and_transformed_rng_keep_existing_binding_priority() -> TestResult {
    let (_, ir, cfg) = input(
        "CONSTANT C\nVARIABLE x\nBoot == Print(\"must-not-start\",0)\nInit == x=C\nNext == x'=x\nP == x=0",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    let mut options = vec![CheckOptions::dive(0)];
    let mut predecessor = CheckOptions::dive(0);
    predecessor.predecessor_directory = Some(std::path::PathBuf::from("must-not-open"));
    options.push(predecessor);
    for opts in options {
        let failure = temporal::check_final(&ir, &cfg, &opts, Limits::default())
            .err()
            .ok_or("unassigned constant must fail before startup")?;
        let temporal::Error::Binding(failure) = failure else {
            return Err("original binding error must retain priority".into());
        };
        assert_eq!(failure.error.code(), Some(2222));
        assert_eq!(failure.output.len(), 0);
    }
    let (_, transformed_ir, transformed_cfg) = input(
        "CONSTANT C\nVARIABLE x\nBoot == Print(\"must-not-start\",0)\nUnused == RandomElement({1,2})\nInit == x=C\nNext == x'=x\nV == x\nP == x=0",
        "INIT Init\nNEXT Next\nPROPERTY P\nVIEW V\n",
    )?;
    let failure = temporal::check_final(
        &transformed_ir,
        &transformed_cfg,
        &CheckOptions::dive(0),
        Limits::default(),
    )
    .err()
    .ok_or("existing transformed-scope binding priority")?;
    assert!(
        matches!(failure, temporal::Error::Binding(ref failure) if failure.error.code()==Some(2222) && failure.output.is_empty())
    );
    Ok(())
}

#[test]
fn imported_and_lexical_property_roles_retain_their_real_scopes() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-check/tests/fixtures/disk-seen-roles");
    for name in ["InstanceProperty", "LetProperty", "PropertyAlias"] {
        let file = root.join(name).join(format!("{name}.tla"));
        let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default())?;
        assert!(analysis.accepted(), "{analysis:?}");
        let ir = Arc::new(dive_ir::lower(&analysis)?);
        let cfg = dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?)
            .into_config()
            .ok_or("configuration")?;
        for opts in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            let ordinary = dive_check::check(&ir, &cfg, &opts);
            let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("bound imported/lexical safety roles".into());
            };
            equal(actual, &ordinary);
            assert!(matches!(actual.verdict, Verdict::InvariantViolated { .. }));
        }
    }
    Ok(())
}

#[test]
fn excluded_options_keep_the_nonempty_property_refusal_and_admitted_ones_succeed() -> TestResult {
    let (_, ir, cfg) = input(
        "VARIABLE x\nInit == x=0\nNext == x'=x\nP == x=0",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    let mut options = Vec::new();
    let base = CheckOptions::dive(0);
    let mut option = base.clone();
    option.seen_disk = Some(dive_check::SeenOptions::default());
    options.push((
        option,
        "disk seen-state retention with temporal capture is not implemented",
    ));
    let mut excluded = Vec::new();
    let mut option = base.clone();
    option.frontier_memory_bytes = Some(0);
    excluded.push(option);
    let mut option = base.clone();
    option.frontier_directory = Some(std::path::PathBuf::from("must-not-open"));
    excluded.push(option);
    let mut option = base.clone();
    option.predecessor_memory_bytes = Some(0);
    excluded.push(option);
    let mut option = base.clone();
    option.predecessor_directory = Some(std::path::PathBuf::from("must-not-open"));
    excluded.push(option);
    // Store, polynomial, fingerprint, deadlock and sequential settings are
    // honored by the safety engine: they take the safety route and succeed.
    let mut admitted = Vec::new();
    let mut option = base.clone();
    option.store = dive_store::StoreMode::Exact;
    admitted.push(option);
    let mut option = base.clone();
    option.fp_index = 1;
    admitted.push(option);
    let mut option = base.clone();
    option.tlc_fingerprints = true;
    admitted.push(option);
    let mut option = base.clone();
    option.no_deadlock = true;
    admitted.push(option);
    let mut option = base.clone();
    option.sequential = true;
    admitted.push(option);
    for opts in admitted {
        let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
        assert!(
            matches!(report.outcome(), temporal::Outcome::Safety(r) if r.verdict == dive_check::Verdict::Success),
            "{opts:?}"
        );
    }
    let mut option = base.clone();
    option.eval.warnings = false;
    excluded.push(option);
    let mut option = base;
    option.chaos = Some(0);
    excluded.push(option);
    options.extend(
        excluded
            .into_iter()
            .map(|opts| (opts, "no temporal properties")),
    );
    for (opts, expected) in options {
        let error = temporal::check_final(&ir, &cfg, &opts, Limits::default())
            .err()
            .ok_or("excluded options keep the prior route")?;
        assert!(
            matches!(error, temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Unsupported(message)) if message==expected)
        );
    }
    Ok(())
}

// The accepted temporal binder's public constructor sequence, retaining its raw
// RunHost rather than the safety dispatch's forwarding wrapper. This reference
// performs real binding/analysis under a separate core; it evaluates no copied
// product implementation and issues no independent oracle/parity claim.
fn plain_temporal_binding(
    ir: &Arc<dive_ir::Ir>,
    cfg: &dive_config::ModelConfig,
    opts: &CheckOptions,
) -> Result<dive_model::Model, Box<dive_model::Failure>> {
    let mode = if opts.profile == dive_eval::Profile::Tlc {
        dive_stdlib::random::RandomMode::Tlc
    } else {
        dive_stdlib::random::RandomMode::Dive
    };
    dive_model::bind(
        ir.clone(),
        cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: opts.no_deadlock,
            host: Some(Arc::new(dive_stdlib::host::RunHost::new(
                dive_stdlib::random::Streams::new(opts.seed, mode),
                None,
            ))),
        },
    )
}

#[test]
fn startup_print_survives_later_binding_errors_in_both_profiles() -> TestResult {
    for property in ["x=0", "[]<>(x=0)"] {
        for (periodic, code) in [("Missing", 2229), ("Arg", 2228)] {
            let (_, ir, cfg) = input(
                &format!(
                    "VARIABLE x\nBoot == Print(\"before-binding-error\",0)\nInit == x=Boot\nNext == x'=x\nArg(a) == a\nP == {property}"
                ),
                &format!("INIT Init\nNEXT Next\nPROPERTY P\n_PERIODIC {periodic}\n"),
            )?;
            for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                let expected = plain_temporal_binding(&ir, &cfg, &opts)
                    .err()
                    .ok_or("reference late binding failure")?;
                assert_eq!(expected.error.code(), Some(code));
                assert_eq!(
                    expected
                        .output
                        .iter()
                        .filter(|m| matches!(m, dive_model::Message::Print(_)))
                        .count(),
                    1
                );
                let failure = temporal::check_final_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Limits {
                        properties: 0,
                        ..Default::default()
                    },
                )
                .err()
                .ok_or("binding error must precede planning limit")?;
                let temporal::Error::Binding(actual) = failure.error() else {
                    return Err("binding error was reordered".into());
                };
                assert_eq!(actual.as_ref(), expected.as_ref());
                assert_eq!(failure.stage(), temporal::Stage::Binding);
                assert_eq!(failure.capture_completed(), Some(false));
                assert!(failure.exploration().is_none());
            }
        }
    }
    Ok(())
}

#[test]
fn genuine_temporal_startup_print_retains_the_uncertified_plain_host_word() -> TestResult {
    let (_, ir, cfg) = input(
        "VARIABLE x\nBoot == Print(\"one-startup\",FALSE)\nInit == x=Boot\nNext == x'=~x\nP == []<>x",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = plain_temporal_binding(&ir, &cfg, &opts)?;
        assert_eq!(model.implied_temporals.len(), 1);
        let expected =
            dive_liveness::analyze::analyze_final_with_evidence(&model, &opts, Limits::default())?;
        let dive_liveness::analyze::Analysis::Candidate(expected) = expected else {
            return Err("initial stutter must falsify the temporal property".into());
        };
        assert_eq!(expected.exploration.output.len(), 1);
        let failure = temporal::check_final_with_evidence(&ir, &cfg, &opts, Limits::default())
            .err()
            .ok_or("side effects must remain uncertified by the independent validator")?;
        assert_eq!(failure.stage(), temporal::Stage::Validation);
        assert_eq!(failure.capture_completed(), Some(true));
        assert_eq!(failure.error().code(), "DIVE-A0032");
        let temporal::Error::Validation { candidate, report } = failure.error() else {
            return Err("startup Print must retain the independent validation refusal".into());
        };
        assert_eq!(
            report.validation,
            Validation::Unsupported(
                "unsupported temporal validation: side effects, dynamic bindings or ENABLED in periodic behavior validation".into()
            )
        );
        assert_eq!(report.length, 1);
        assert_eq!(report.notes.len(), 0);
        assert_eq!(candidate.states, expected.states);
        assert_eq!(candidate.properties, expected.properties);
        assert_eq!(candidate.back_to, expected.back_to);
        assert_eq!(candidate.exploration.stats, expected.exploration.stats);
        assert_eq!(candidate.exploration.output, expected.exploration.output);
        assert_eq!(
            candidate.exploration.warnings,
            expected.exploration.warnings
        );
        let stats = candidate.exploration.stats;
        assert_eq!((stats.generated, stats.distinct, stats.depth), (3, 2, 2));
        let exploration = failure.exploration().ok_or("completed capture evidence")?;
        assert_eq!(exploration.stats, expected.exploration.stats);
        assert_eq!(exploration.workers, expected.exploration.workers);
        assert_eq!(exploration.output, expected.exploration.output);
        assert_eq!(exploration.warnings, expected.exploration.warnings);
    }
    Ok(())
}

#[test]
fn genuine_temporal_startup_host_reads_keep_effect_and_error_priority() -> TestResult {
    for property in ["[]<>x", "<>1", "\\A i \\in {TLCGet(\"level\")}: []<>(x=i)"] {
        let (_, ir, cfg) = input(
            &format!(
                "ASSUME FALSE\nVARIABLE x\nBoot == Print(\"before-priority\",FALSE)\nInit == x=Boot /\\ TLCGet(\"level\")=0\nNext == x'=~x\nP == {property}"
            ),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let model = plain_temporal_binding(&ir, &cfg, &opts)?;
            assert_eq!(model.implied_temporals.len(), 1);
            assert_eq!(
                model
                    .output
                    .iter()
                    .filter(|m| matches!(m, dive_model::Message::Print(_)))
                    .count(),
                1
            );
            let expected = dive_liveness::analyze::analyze_final_with_evidence(
                &model,
                &opts,
                Limits::default(),
            )
            .err()
            .ok_or("old route must refuse effects or fail planning")?;
            if property == "[]<>x" {
                let dive_liveness::analyze::AnalysisError::Exploration(expected) = expected.error
                else {
                    return Err("ASSUME FALSE must stop capture before search".into());
                };
                assert_eq!(
                    expected.verdict,
                    Verdict::AssumptionFalse {
                        message:
                            "Assumption line 3, col 8 to line 3, col 12 of module Roles is false."
                                .into()
                    }
                );
                assert_eq!(expected.workers.used, 0);
                assert!(expected.counterexample.is_none());
                assert_eq!(expected.output.len(), 1);
                assert!(expected.stats.is_none());
                let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
                let Outcome::Safety(actual) = report.outcome() else {
                    return Err("assumption stop must retain the ordinary reference result".into());
                };
                equal(actual, &expected);
            } else {
                assert!(matches!(
                    expected.error,
                    dive_liveness::analyze::AnalysisError::Planning(_)
                ));
                let failure =
                    temporal::check_final_with_evidence(&ir, &cfg, &opts, Limits::default())
                        .err()
                        .ok_or("planning error must precede effects and assumptions")?;
                assert_eq!(
                    failure.stage(),
                    temporal::Stage::Analysis(dive_liveness::analyze::Stage::Planning)
                );
                assert_eq!(failure.error().code(), expected.error.code());
                assert_eq!(failure.error().to_string(), expected.error.to_string());
                assert!(failure.exploration().is_none());
            }
        }
    }
    Ok(())
}

#[test]
fn genuine_temporal_live_counter_refusal_precedes_assumptions() -> TestResult {
    let (_, ir, cfg) = input(
        "ASSUME FALSE\nVARIABLE x\nBoot == Print(\"before-counter-refusal\",FALSE)\nInit == x=Boot /\\ TLCGet(\"generated\")=0\nNext == x'=~x\nP == []<>x",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = plain_temporal_binding(&ir, &cfg, &opts)?;
        assert_eq!(model.implied_temporals.len(), 1);
        let failure =
            dive_liveness::analyze::analyze_final_with_evidence(&model, &opts, Limits::default())
                .err()
                .ok_or("live counter must refuse capture before assumptions")?;
        assert_eq!(failure.stage, dive_liveness::analyze::Stage::Capture);
        assert!(failure.exploration.is_none());
        let dive_liveness::analyze::AnalysisError::Exploration(expected) = failure.error else {
            return Err("live counter capture refusal must precede ASSUME FALSE".into());
        };
        assert!(matches!(&expected.verdict, Verdict::Unsupported { feature }
            if feature.contains("stateful") && feature.contains("TLCGet(\"generated\")")));
        assert_eq!(expected.output, ["\"before-counter-refusal\"  FALSE"]);
        assert!(expected.stats.is_none());
        assert!(expected.counterexample.is_none());
        assert_eq!(expected.workers.used, 0);
        let report = temporal::check_final(&ir, &cfg, &opts, Limits::default())?;
        let Outcome::Safety(actual) = report.outcome() else {
            return Err("live counter must retain the named reference refusal".into());
        };
        equal(actual, &expected);
    }
    Ok(())
}
