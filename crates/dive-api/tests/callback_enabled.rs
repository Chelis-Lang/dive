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
                    let report = dive_api::check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        compat_tlc,
                        store,
                        workers: Some(workers),
                        seed: Some(0),
                        fp: Some(0),
                        ..Default::default()
                    })?;
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

// Native-only metamorphic source and admission controls; these do not add
// primary observations or claim new original-model conformance.
#[test]
fn query_callback_replay_keeps_renamed_and_nested_scopes_and_requires_its_own_cover()
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
                    let value = dive_api::check_file(&request)?
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
            assert!(
                matches!(model.read(&word).0.validation,Validation::Unsupported(ref message)
                if message.contains("no finite cover for primed variable a"))
            );
        }
        Ok(())
    })();
    std::fs::remove_dir_all(directory)?;
    result
}
