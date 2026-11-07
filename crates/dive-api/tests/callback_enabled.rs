//! Native callbacks preserve ENABLED lookup context through INSTANCE scopes.
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, Store};
use serde_json::Value;
use std::path::Path;

#[test]
fn callback_enabled_cutoff_keeps_initial_invariant_and_real_enabled_control()
-> Result<(), Box<dyn std::error::Error>> {
    let primary: Value =
        serde_json::from_str(include_str!("fixtures/callback-enabled/primary.json"))?;
    assert_eq!(primary["oracle"], "tlc-2026.10.06.014338");
    assert_eq!(
        primary["oracle_sha256"],
        "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
    );
    assert_eq!(
        primary["upstream_commit"],
        "94d0c5070852b37b801bb8998c436383ab7ee37b"
    );
    assert_eq!(primary["workers"], 1);
    assert_eq!(primary["seed"], 0);
    assert_eq!(primary["fingerprint"], 0);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/callback-enabled");
    for name in ["disabled", "enabled"] {
        let expected = &primary["cases"][name];
        let file = root.join(name).join("CallbackEnabled.tla");
        let config = root.join(name).join("CallbackEnabled.cfg");
        for compat_tlc in [true, false] {
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4] {
                    let report = dive_api::check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(config.clone()),
                            compat_tlc,
                            store,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..Default::default()
                        },
                        dive_api::Engine::Reference,
                    )?;
                    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                    assert_eq!(
                        value["exit_code"], expected["physical_exit"],
                        "{name}: {value}"
                    );
                    assert_eq!(value["tlc"]["code"], expected["verdict_code"], "{name}");
                    assert_eq!(value["tlc"]["verified"], true);
                    assert_eq!(value["property"], expected["property"]);
                    if name == "disabled" {
                        assert_eq!(expected["verdict"], "safety-violation");
                        assert_eq!(value["verdict"], "invariant-violation");
                        assert!(
                            expected["counts"]
                                .as_object()
                                .ok_or("counts")?
                                .values()
                                .all(Value::is_null)
                        );
                        validate_counterexample(&file, &config, compat_tlc, &value, expected)?;
                    } else {
                        assert_eq!(expected["verdict"], "success");
                        assert_eq!(value["verdict"], "success");
                        assert!(value["counterexample"].is_null());
                        assert!(expected["trace"].is_null());
                        reject_forged_invariant_word(&file, &config, compat_tlc)?;
                        for (native, oracle) in [
                            ("generated", "states_generated"),
                            ("distinct", "distinct_states"),
                            ("depth", "depth"),
                            ("left_on_queue", "states_left_on_queue"),
                        ] {
                            assert_eq!(
                                value["statistics"][native], expected["counts"][oracle],
                                "{name}: {native}"
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_counterexample(
    file: &Path,
    config: &Path,
    compat_tlc: bool,
    value: &Value,
    expected: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut word = TraceDoc::from_json(&value["counterexample"]["trace"])?;
    assert_eq!(Value::from(word.states.len()), expected["trace"]["length"]);
    assert_eq!(expected["trace"]["ending"], "finite");
    let model = SpecModel::load(
        file,
        config,
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..Default::default()
        },
    )?;
    let (valid, typed) = model.read(&word);
    assert_eq!(valid.validation, Validation::Valid);
    let typed = typed.ok_or("decoded genuine trace")?;
    assert_eq!(model.validate_trace(&typed).validation, Validation::Valid);
    let mut wrong_ending = word.clone();
    wrong_ending.ending = Some(Ending::Invariant("MissingInvariant".into()));
    assert!(matches!(
        model.read(&wrong_ending).0.validation,
        Validation::Invalid(_)
    ));
    word.states[0].insert("b".into(), 99.into());
    let (invalid, typed) = model.read(&word);
    assert!(matches!(invalid.validation, Validation::Invalid(_)));
    let typed = typed.ok_or("decoded forged trace")?;
    assert!(matches!(
        model.validate_trace(&typed).validation,
        Validation::Invalid(_)
    ));
    Ok(())
}

/// Issue #312's forged word: when the callback leaves `Step` enabled, the
/// invariant holds in the initial state, so a word that ends the initial state
/// with an `Inv` violation is forged. Replay must reject it, through the
/// document and the typed API. The `callback-enabled` fixtures write the prime
/// inline (`LAMBDA e : e /= a'`); the issue's own captured primed
/// `LET Cut == a'` is the corpus spec exercised by
/// `issue_312_primed_let_capture_words_replay_in_both_profiles`.
fn reject_forged_invariant_word(
    file: &Path,
    config: &Path,
    compat_tlc: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut word = TraceDoc::behavior(vec![serde_json::json!({"b":0,"q":0,"s":{"seq":[1,2,3]}})])?;
    word.ending = Some(Ending::Invariant("Inv".into()));
    let model = SpecModel::load(
        file,
        config,
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..Default::default()
        },
    )?;
    let (document, typed) = model.read(&word);
    assert!(
        matches!(document.validation, Validation::Invalid(_)),
        "{:?}",
        document.validation
    );
    let typed = typed.ok_or("decoded forged word")?;
    assert!(matches!(
        model.validate_trace(&typed).validation,
        Validation::Invalid(_)
    ));
    Ok(())
}

/// Issue #312 on its own spec: the `lazy-enabled-capture` corpus's
/// `primed_disabled` and `primed_enabled` models, whose callback reads the
/// captured primed `LET Cut == a'`. Native checking must match the pinned TLC
/// goldens in both profiles; replay must accept the genuine initial-violation
/// word of the disabled case and reject the same word for the enabled case,
/// where `Inv` holds, through the document and typed APIs.
#[test]
fn issue_312_primed_let_capture_words_replay_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let specs = repo.join("conformance/corpus/lazy-enabled-capture/specifications");
    let goldens =
        repo.join("conformance/golden/tlc-2026.10.06.014338/lazy-enabled-capture/specifications");
    for (name, violated) in [("primed_disabled", true), ("primed_enabled", false)] {
        let file = specs.join(name).join("CallbackEnabled.tla");
        let config = specs.join(name).join("CallbackEnabled.cfg");
        assert!(
            std::fs::read_to_string(&file)?
                .contains("LET Cut == a' IN c' = SelectSeq(c, LAMBDA e : e /= Cut)"),
            "{name} must keep the issue's captured primed LET"
        );
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(
            goldens.join(name).join("CallbackEnabled.cfg.json"),
        )?)?;
        let expected = &golden["result"];
        for compat_tlc in [true, false] {
            let value = dive_api::check_file_engine(
                &CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..Default::default()
                },
                dive_api::Engine::Reference,
            )?
            .to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(value["exit_code"], expected["exit_code"], "{name}: {value}");
            assert_eq!(value["tlc"]["code"], expected["verdict_code"], "{name}");
            assert_eq!(value["property"], expected["violated_property"], "{name}");
            if violated {
                assert_eq!(value["tlc"]["verified"], true, "{name}");
                let genuine = TraceDoc::from_json(&value["counterexample"]["trace"])?;
                assert_eq!(
                    Value::from(genuine.states.len()),
                    expected["trace"]["length"],
                    "{name}"
                );
                let model = SpecModel::load(
                    &file,
                    &config,
                    &TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )?;
                let (document, typed) = model.read(&genuine);
                assert_eq!(document.validation, Validation::Valid, "{name}");
                let typed = typed.ok_or("decoded genuine word")?;
                assert_eq!(model.validate_trace(&typed).validation, Validation::Valid);
            } else {
                assert!(value["counterexample"].is_null(), "{name}");
                for (native, oracle) in [
                    ("generated", "states_generated"),
                    ("distinct", "distinct_states"),
                    ("depth", "depth"),
                    ("left_on_queue", "states_left_on_queue"),
                ] {
                    assert_eq!(
                        value["statistics"][native], expected["counts"][oracle],
                        "{name}: {native}"
                    );
                }
            }
            // The same initial-state `Inv` word is genuine for the disabled
            // case and forged for the enabled one.
            let mut word =
                TraceDoc::behavior(vec![serde_json::json!({"b":0,"q":0,"s":{"seq":[1,2,3]}})])?;
            word.ending = Some(Ending::Invariant("Inv".into()));
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..Default::default()
                },
            )?;
            let (document, typed) = model.read(&word);
            let typed = typed.ok_or("decoded initial-state word")?;
            let typed = model.validate_trace(&typed).validation;
            if violated {
                assert_eq!(document.validation, Validation::Valid, "{name}");
                assert_eq!(typed, Validation::Valid, "{name}");
            } else {
                assert!(
                    matches!(document.validation, Validation::Invalid(_)),
                    "{name}: {:?}",
                    document.validation
                );
                assert!(matches!(typed, Validation::Invalid(_)), "{name}");
            }
        }
    }
    Ok(())
}

// Native-only metamorphic source and admission controls; these do not add
// primary observations or claim new original-model conformance.
#[test]
fn query_callback_replay_keeps_renamed_and_nested_scopes_and_rejects_unassigned_reads()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/callback-enabled");
    let directory = std::env::temp_dir().join(format!(
        "dive-callback-query-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let file = directory.join("CallbackEnabled.tla");
        let config = directory.join("CallbackEnabled.cfg");
        std::fs::write(&config, "INIT Init\nNEXT Next\nINVARIANT Inv\n")?;
        for name in ["disabled", "enabled"] {
            let source = std::fs::read_to_string(root.join(name).join("CallbackEnabled.tla"))?;
            let renamed = source
                .replace("VARIABLES b, q, s", "VARIABLES outer, q, s")
                .replace("a <- b,", "a <- outer,")
                .replace("Init == b = 0", "Init == outer = 0")
                .replace("UNCHANGED <<b, q, s>>", "UNCHANGED <<outer, q, s>>")
                .replace("ELSE b /= 0", "ELSE outer /= 0");
            let nested = source.replace("INSTANCE Inner WITH", "INSTANCE Middle WITH")
                + "---- MODULE Middle ----\nVARIABLES a, b, c\nINSTANCE Inner\n====\n";
            for transformed in [renamed, nested] {
                std::fs::write(&file, transformed)?;
                for compat_tlc in [true, false] {
                    let request = CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        compat_tlc,
                        workers: Some(1),
                        seed: Some(0),
                        fp: Some(0),
                        ..Default::default()
                    };
                    let value = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?
                        .to_json_with_exit_convention(ExitConvention::Tlc);
                    assert_eq!(
                        value["exit_code"],
                        if name == "disabled" { 12 } else { 0 },
                        "{name}: {value}"
                    );
                    if name == "disabled" {
                        let word = TraceDoc::from_json(&value["counterexample"]["trace"])?;
                        let model = SpecModel::load(
                            &file,
                            &config,
                            &TraceOptions {
                                compat_tlc,
                                run: Some((0, 0)),
                                ..Default::default()
                            },
                        )?;
                        assert_eq!(model.read(&word).0.validation, Validation::Valid);
                    }
                }
            }
        }
        let source = std::fs::read_to_string(root.join("disabled/CallbackEnabled.tla"))?;
        let missing = source.replace("        /\\ a' = 1\n", "");
        assert_ne!(source, missing);
        std::fs::write(&file, missing)?;
        let mut word =
            TraceDoc::behavior(vec![serde_json::json!({"b":0,"q":0,"s":{"seq":[1,2,3]}})])?;
        word.ending = Some(Ending::Invariant("Inv".into()));
        for compat_tlc in [true, false] {
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..Default::default()
                },
            )?;
            // Without `a' = 1`, the callback reads an unassigned `a'`: the
            // invariant raises TLC's evaluation error, so an invariant-violation
            // claim is rejected rather than refused.
            assert!(matches!(
                model.read(&word).0.validation,
                Validation::Invalid(ref failure) if failure.check == dive_api::trace::Check::Invariant
            ));
        }
        Ok(())
    })();
    std::fs::remove_dir_all(directory)?;
    result
}
