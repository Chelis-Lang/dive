//! File simulation retains independent validity for executed native choices.
#![allow(clippy::expect_used, reason = "fixture setup and exact report checks")]

use dive_api::{Engine, SimulateRequest, simulate_file_engine};

#[test]
fn native_random_steps_and_startup_have_reproducible_validated_samples() {
    let root = std::env::temp_dir().join(format!("dive-finite-choices-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("fixture directory");
    let file = root.join("Choices.tla");
    std::fs::write(&file,"---- MODULE Choices ----\nEXTENDS Naturals,TLC\nCONSTANT C\nVARIABLE x\nPick==RandomElement({0,1})\nInit==x=C\nNext==x'=RandomElement(1..1000000)\n====\n").expect("source");
    std::fs::write(
        file.with_extension("cfg"),
        "CONSTANT C <- Pick\nINIT Init\nNEXT Next\n",
    )
    .expect("configuration");
    for compat_tlc in [false, true] {
        let request = SimulateRequest {
            file: file.clone(),
            compat_tlc,
            seed: Some(7),
            depth: Some(100),
            traces: 2,
            ..Default::default()
        };
        let first = simulate_file_engine(&request, Engine::Reference)
            .expect("simulation")
            .to_json();
        assert_eq!(first["result"], "sampled", "{first}");
        assert_eq!(first["exhaustive"], false);
        assert_eq!(first["model_proved"], false);
        assert_eq!(first["statistics"]["generated"], 201);
        for trace in first["traces"].as_array().expect("samples") {
            assert_eq!(trace["validation"]["result"], "valid", "{trace}");
        }
        let repeated = simulate_file_engine(&request, Engine::Reference)
            .expect("same seed")
            .to_json();
        assert_eq!(first["traces"], repeated["traces"]);
        let automatic = simulate_file_engine(&request, Engine::Auto)
            .expect("Auto request")
            .to_json();
        assert_eq!(automatic["result"], "sampled", "{automatic}");
        assert_eq!(first["traces"], automatic["traces"]);
    }
    check_action_contexts(&file);
    std::fs::write(&file,"---- MODULE Choices ----\nEXTENDS TLC\nVARIABLE x\nInit==x=TLCGet(\"config\").seed\nNext==UNCHANGED x\n====\n").expect("configuration source");
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")
        .expect("configuration fixture");
    for compat_tlc in [false, true] {
        let report = simulate_file_engine(
            &SimulateRequest {
                file: file.clone(),
                compat_tlc,
                seed: Some(7),
                depth: Some(2),
                traces: 1,
                ..Default::default()
            },
            Engine::Reference,
        )
        .expect("actual runtime config")
        .to_json();
        assert_eq!(report["result"], "sampled", "{report}");
        assert_eq!(report["traces"][0]["validation"]["result"], "valid");
        assert_eq!(
            report["traces"][0]["document"]["states"][0]["values"]["x"],
            "7"
        );
    }
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

fn check_action_contexts(file: &std::path::Path) {
    let contexts: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/finite-choices/oracle.json"
    ))
    .expect("primary context contracts");
    assert_eq!(contexts["oracle"], "tlc-2026.10.06.014338");
    for (case, source) in [
        (
            "let-action",
            include_str!("../../dive-sim/tests/fixtures/finite-choices/let-action/Choices.tla"),
        ),
        (
            "bound-action",
            include_str!("../../dive-sim/tests/fixtures/finite-choices/bound-action/Choices.tla"),
        ),
    ] {
        let expected = contexts["cases"]
            .as_array()
            .expect("primary cases")
            .iter()
            .find(|row| row["case"] == case)
            .expect("case contract");
        assert_eq!(expected["physical_exit"], 0);
        std::fs::write(file, source).expect("captured action");
        let config = if case == "let-action" {
            include_str!("../../dive-sim/tests/fixtures/finite-choices/let-action/Choices.cfg")
        } else {
            include_str!("../../dive-sim/tests/fixtures/finite-choices/bound-action/Choices.cfg")
        };
        std::fs::write(file.with_extension("cfg"), config).expect("action configuration");
        for compat_tlc in [false, true] {
            let report = simulate_file_engine(
                &SimulateRequest {
                    file: file.to_path_buf(),
                    compat_tlc,
                    seed: Some(7),
                    depth: Some(4),
                    traces: 2,
                    ..Default::default()
                },
                Engine::Reference,
            )
            .expect("captured Next")
            .to_json();
            assert_eq!(report["result"], "sampled", "{case}: {report}");
            let traces = report["traces"].as_array().expect("captured samples");
            assert!(
                traces
                    .iter()
                    .all(|trace| trace["validation"]["result"] == "valid")
            );
            if expected["some_successors_differ_within_row"] == true {
                assert!(
                    traces.iter().any(|trace| trace["document"]["states"]
                        .as_array()
                        .expect("states")
                        .iter()
                        .skip(1)
                        .any(|row| row["values"]["a"] != row["values"]["b"])),
                    "uncached LET action context: {case}: {report}"
                );
            } else {
                // getActions freezes constant arguments once during binding;
                // every reached row retains that same actual capture.
                let captured = traces[0]["document"]["states"][1]["values"]["a"].clone();
                assert!(
                    traces.iter().all(|trace| trace["document"]["states"]
                        .as_array()
                        .expect("states")
                        .iter()
                        .skip(1)
                        .all(
                            |row| row["values"]["a"] == captured && row["values"]["b"] == captured
                        )),
                    "constant action argument: {report}"
                );
            }
        }
    }
}
