//! Configured safety roles of `PROPERTY` are checked as safety on Core, with
//! one start-up stream. These cases compared the retired reference final route
//! with the reference safety engine until #791; they now assert Core's results.
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_runtime::{CheckOptions, Verdict};

#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use core_final::Outcome;

/// Start-up output before a false or failing `ASSUME`.
const OUTPUT_BEFORE_ASSUME: &str = "output printed at startup before a false or failing ASSUME";
/// `RandomElement` in `Init` after start-up definitions drew from the stream.
const INIT_DRAW_AFTER_STARTUP: &str =
    "RandomElement in Init after constant definitions or ASSUMEs drew";
/// `RandomElement` reachable from a configured `VIEW`.
const RANDOM_UNDER_VIEW: &str = "randomness under VIEW is not supported";

/// Asserts that Core refuses `body` and `cfg` by naming `feature`.
fn refuses(body: &str, cfg: &str, opts: &CheckOptions, feature: &str) {
    let report = core_final::run(&source(body), cfg, opts);
    refused::assert_refused(&report.to_json(), report.exit_code(), feature);
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source(body: &str) -> String {
    format!("---- MODULE Roles ----\nEXTENDS Naturals,TLC\n{body}\n====\n")
}

/// Core's safety result for `body` and `cfg`.
fn safety(
    body: &str,
    cfg: &str,
    opts: &CheckOptions,
) -> Result<dive_runtime::CheckResult, Box<dyn std::error::Error>> {
    let run = core_final::check(source(body), cfg, opts).map_err(|e| e.json().to_string())?;
    match run.outcome() {
        Outcome::Safety(result) => Ok(result.as_ref().clone()),
        other => Err(format!("expected a safety result: {other:?}").into()),
    }
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
        let body =
            format!("VARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n{property}");
        for mut opts in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for workers in [1, 4] {
                opts.workers = workers;
                let actual = safety(&body, "INIT Init\nNEXT Next\nPROPERTY P\n", &opts)?;
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
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let actual = safety(body, config, &opts)?;
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
                &source(body),
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
fn startup_print_and_rng_consumption_before_init_is_a_named_refusal() {
    // Boot draws from the stream before Init does. Core refuses an Init draw
    // after start-up draws rather than choose how the streams continue.
    let body = "VARIABLE x\nBoot == Print(\"one-startup\",RandomElement(1..32))\nInit == x=RandomElement(1..32)\nNext == x'=IF x<Boot THEN x+1 ELSE x\nP == [](x<=32)";
    for tlc in [false, true] {
        for seed in 0..4 {
            let mut opts = if tlc {
                CheckOptions::tlc(0, seed)
            } else {
                CheckOptions::dive(seed)
            };
            opts.workers = 4;
            refuses(
                body,
                "INIT Init\nNEXT Next\nPROPERTY P\n",
                &opts,
                INIT_DRAW_AFTER_STARTUP,
            );
        }
    }
}

#[test]
fn mixed_real_temporal_obligations_never_become_safety_success() -> TestResult {
    for property in ["P == []<>(x=1)", "P == x=0 /\\ [][x'=x]_x /\\ []<>(x=1)"] {
        let body = format!("VARIABLE x\nInit == x=0\nNext == x'=x\n{property}");
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let run = core_final::check(source(&body), "INIT Init\nNEXT Next\nPROPERTY P\n", &opts)
                .map_err(|e| e.json().to_string())?;
            let Outcome::Violation(counterexample) = run.outcome() else {
                return Err(
                    "real temporal obligation must retain validated periodic violation".into(),
                );
            };
            assert_eq!(counterexample.validation().validation, Validation::Valid);
            assert_ne!(counterexample.document().states.len(), 0);
            assert_eq!(counterexample.document().properties, ["P"]);
        }
    }
    Ok(())
}

#[test]
fn initial_invariant_and_postcondition_first_errors_keep_safety_priority() -> TestResult {
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
    ] {
        let cfg = format!("INIT Init\nNEXT Next\nPROPERTY P\n{extra}");
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let actual = safety(body, &cfg, &opts)?;
            assert_eq!(actual.verdict.kind(), expected_kind);
            assert!(
                !actual.binding_model_error,
                "these errors occur after binding"
            );
        }
    }
    // A configured POSTCONDITION is a named Core refusal.
    let body = "VARIABLE x\nInit == x=0\nNext == x'=x\nP == x=0\nPost == TRUE";
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let stop = core_final::check(
            source(body),
            "INIT Init\nNEXT Next\nPROPERTY P\nPOSTCONDITION Post\n",
            &opts,
        )
        .err()
        .ok_or("POSTCONDITION")?;
        refused::assert_refused(&stop.json(), stop.exit_code(), refused::POSTCONDITION);
    }
    Ok(())
}

#[test]
fn unassigned_constants_fail_binding_before_startup_effects() {
    let body = "CONSTANT C\nVARIABLE x\nBoot == Print(\"must-not-start\",0)\nInit == x=C\nNext == x'=x\nP == x=0";
    let report = core_final::run(
        &source(body),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
        &CheckOptions::dive(0),
    );
    let json = report.to_json();
    assert_eq!(json["tlc"]["code"], 2222, "{json}");
    assert_eq!(json["output"], serde_json::json!([]), "{json}");
    // With randomness reachable from a VIEW, Core refuses by name first.
    let body = "CONSTANT C\nVARIABLE x\nBoot == Print(\"must-not-start\",0)\nUnused == RandomElement({1,2})\nInit == x=C\nNext == x'=x\nV == x\nP == x=0";
    refuses(
        body,
        "INIT Init\nNEXT Next\nPROPERTY P\nVIEW V\n",
        &CheckOptions::dive(0),
        RANDOM_UNDER_VIEW,
    );
}

#[test]
fn imported_and_lexical_property_roles_retain_their_real_scopes() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checker/disk-seen-roles");
    for name in ["InstanceProperty", "LetProperty", "PropertyAlias"] {
        let file = root.join(name).join(format!("{name}.tla"));
        for opts in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            let request = dive_api::CheckRequest {
                file: file.clone(),
                ..core_final::request(&opts)
            };
            let report = dive_api::check_file_engine(&request, dive_api::Engine::Core)?;
            let actual = report
                .result()
                .ok_or("bound imported/lexical safety roles")?;
            assert!(
                matches!(actual.verdict, Verdict::InvariantViolated { .. }),
                "{name}: {:?}",
                actual.verdict
            );
        }
    }
    Ok(())
}

#[test]
fn store_fingerprint_deadlock_and_sequential_settings_take_the_safety_route() -> TestResult {
    let body = "VARIABLE x\nInit == x=0\nNext == x'=x\nP == x=0";
    let base = CheckOptions::dive(0);
    let mut admitted = Vec::new();
    let mut option = base.clone();
    option.store = dive_store::StoreMode::Exact;
    admitted.push(option);
    let mut option = base.clone();
    option.fp_index = 1;
    admitted.push(option);
    let mut option = base.clone();
    option.no_deadlock = true;
    admitted.push(option);
    let mut option = base;
    option.sequential = true;
    admitted.push(option);
    for opts in admitted {
        let actual = safety(body, "INIT Init\nNEXT Next\nPROPERTY P\n", &opts)?;
        assert_eq!(actual.verdict, Verdict::Success, "{opts:?}");
    }
    Ok(())
}

#[test]
fn startup_print_survives_later_binding_errors_in_both_profiles() {
    for property in ["x=0", "[]<>(x=0)"] {
        for (periodic, code) in [("Missing", 2229), ("Arg", 2228)] {
            let body = format!(
                "VARIABLE x\nBoot == Print(\"before-binding-error\",0)\nInit == x=Boot\nNext == x'=x\nArg(a) == a\nP == {property}"
            );
            let cfg = format!("INIT Init\nNEXT Next\nPROPERTY P\n_PERIODIC {periodic}\n");
            for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
                let report = core_final::run(&source(&body), &cfg, &opts);
                let json = report.to_json();
                assert_eq!(json["tlc"]["code"], code, "{json}");
                assert_eq!(
                    json["output"],
                    serde_json::json!(["\"before-binding-error\"  0"]),
                    "{json}"
                );
            }
        }
    }
}

#[test]
fn genuine_temporal_startup_print_is_not_certified_by_the_validator() -> TestResult {
    // The independent validator does not certify side effects in a periodic
    // word, so Core refuses this temporal validation failure by name.
    let body = "VARIABLE x\nBoot == Print(\"one-startup\",FALSE)\nInit == x=Boot\nNext == x'=~x\nP == []<>x";
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let stop = core_final::check(source(body), "INIT Init\nNEXT Next\nPROPERTY P\n", &opts)
            .err()
            .ok_or("side effects must remain uncertified by the independent validator")?;
        refused::assert_refused(
            &stop.json(),
            stop.exit_code(),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
    }
    Ok(())
}

#[test]
fn genuine_temporal_startup_output_before_a_false_assumption_is_refused() {
    // Start-up output before ASSUME FALSE is a named Core refusal, whether
    // the property plans (`[]<>x`) or is ill-formed (`<>1`, a level read in a
    // quantifier domain): never a verdict.
    for property in ["[]<>x", "<>1", "\\A i \\in {TLCGet(\"level\")}: []<>(x=i)"] {
        let body = format!(
            "ASSUME FALSE\nVARIABLE x\nBoot == Print(\"before-priority\",FALSE)\nInit == x=Boot /\\ TLCGet(\"level\")=0\nNext == x'=~x\nP == {property}"
        );
        for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            refuses(
                &body,
                "INIT Init\nNEXT Next\nPROPERTY P\n",
                &opts,
                OUTPUT_BEFORE_ASSUME,
            );
        }
    }
}

#[test]
fn genuine_temporal_live_counter_with_a_false_assumption_is_refused() {
    let body = "ASSUME FALSE\nVARIABLE x\nBoot == Print(\"before-counter-refusal\",FALSE)\nInit == x=Boot /\\ TLCGet(\"generated\")=0\nNext == x'=~x\nP == []<>x";
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        // Core refuses the start-up output before ASSUME FALSE by name, before
        // the live counter is reached: never a verdict.
        refuses(
            body,
            "INIT Init\nNEXT Next\nPROPERTY P\n",
            &opts,
            OUTPUT_BEFORE_ASSUME,
        );
    }
}
