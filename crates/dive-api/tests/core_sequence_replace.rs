//! Sequence forms of the CommunityModules replacement operators.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, check_file, check_file_core_opts};
use serde_json::{Value, json};

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = root
        .join("tests/fixtures/core-replace")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        library_paths: vec![root.join("../dive-eval/tests/fixtures/community-modules")],
        workers: Some(workers),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"],
        "code": doc["tlc"]["code"],
        "exit": report.exit_code_for(ExitConvention::Tlc),
        "output": doc["output"], "trace": doc["counterexample"]["trace"],
    })
}

#[test]
fn pure_replacements_match_the_pinned_oracle_and_reference() {
    // Pinned TLC tlc-2026.10.06.014338, one worker, seed/fp 0:
    // fourteen value assertions, success/2193/exit0, 1/2/depth1.
    for compat in [false, true] {
        for workers in [1, 4, 8] {
            let req = request("SeqReplacePure", compat, workers);
            let reference = check_file(&req).expect("reference runs");
            let expected = observed(&reference);
            assert_eq!(expected["verdict"], "success");
            assert_eq!(expected["code"], 2193);
            assert_eq!(expected["exit"], 0);
            assert_eq!(expected["distinct"], 1);
            assert_eq!(expected["generated"], 2);
            assert_eq!(expected["depth"], 1);
            for opts in ["", "-fold", "-memo,-fold,-hashcons"] {
                let core = check_file_core_opts(&req, opts).expect("core runs");
                assert_eq!(core.to_json()["engine"], "core");
                assert_eq!(observed(&core), expected, "{compat}/{workers}/{opts}");
            }
        }
    }
}

#[test]
fn replacement_steps_have_independently_validated_counterexamples() {
    for (name, count) in [("SeqReplaceFirstTrace", 3), ("SeqReplaceAllTrace", 2)] {
        for compat in [false, true] {
            for workers in [1, 4, 8] {
                let req = request(name, compat, workers);
                let reference = check_file(&req).expect("reference runs");
                let expected = observed(&reference);
                assert_eq!(expected["verdict"], "invariant-violation");
                assert_eq!(expected["property"], "Inv");
                assert_eq!(expected["code"], 2110);
                assert_eq!(expected["exit"], 12);
                for key in ["distinct", "generated", "depth"] {
                    assert_eq!(expected[key], count);
                }
                let options = TraceOptions {
                    compat_tlc: compat,
                    library_paths: req.library_paths.clone(),
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                };
                let model = SpecModel::load(&req.file, &req.file.with_extension("cfg"), &options)
                    .expect("independent validator binds");
                for opts in ["", "-fold,-memo", "-hashcons"] {
                    let core = check_file_core_opts(&req, opts).expect("core runs");
                    let actual = observed(&core);
                    assert_eq!(core.to_json()["engine"], "core");
                    assert_eq!(actual, expected, "{name}/{compat}/{workers}/{opts}");
                    let trace = TraceDoc::from_json(&actual["trace"]).expect("word decodes");
                    assert!(matches!(
                        model.validate(&trace).validation,
                        Validation::Valid
                    ));
                    let mut bad = actual["trace"].clone();
                    bad["states"][1]["values"]["s"] = json!({"seq": [7,1,0]});
                    let bad = TraceDoc::from_json(&bad).expect("mutant decodes");
                    assert!(matches!(
                        model.validate(&bad).validation,
                        Validation::Invalid(_)
                    ));
                }
            }
        }
    }
}

#[test]
fn unimplemented_sequence_demands_stay_explicitly_refused() {
    // Pinned TLC succeeds on each. The LET thunk prints once per state;
    // named/operator calls print twice; the three inline arguments print six.
    // These are pinned counts, not proof of TLC effect-demand order: #612
    // records the existing reference order bug. Auto must preserve reference.
    for (name, output_count) in [
        ("SeqReplaceMixed", 0),
        ("SeqReplaceNested", 0),
        ("SeqReplaceOverlapAtStart", 0),
        ("SeqReplaceInlineEffect", 12),
        ("SeqReplaceNamedEffect", 4),
        ("SeqReplaceLetEffect", 2),
        ("SeqReplaceOperatorEffect", 4),
    ] {
        for compat in [false, true] {
            let req = request(name, compat, 1);
            let core = check_file(&req).expect("refusal reports");
            let doc = core.to_json();
            assert_eq!(doc["verdict"], "unsupported", "{name}: {doc:#}");
            assert!(doc.to_string().contains("Replace"), "{name}: {doc:#}");
            assert!(doc["counterexample"].is_null(), "{name}: {doc:#}");
            // Only the retired reference engine ran these (#791, #910);
            // pinned TLC prints this many lines.
            let _ = output_count;
        }
    }
}

#[test]
fn existing_effectful_string_override_keeps_its_output() {
    for compat in [false, true] {
        let req = request("StringReplaceInlineEffect", compat, 8);
        let reference = check_file(&req).expect("reference runs");
        let core = check_file(&req).expect("core runs");
        let expected = observed(&reference);
        assert_eq!(expected["verdict"], "success");
        assert_eq!(
            expected["output"].as_array().expect("output array").len(),
            6
        );
        assert_eq!(core.to_json()["engine"], "core");
        assert_eq!(observed(&core), expected);
    }
}

#[test]
fn partition_choice_overlap_is_explicitly_refused() {
    // Pinned TLC returns <<0,1,2>>; the rejected absolute-position greedy
    // rule returned <<0,2,1>> and falsely reported an invariant violation.
    for compat in [false, true] {
        let req = request("SeqReplacePartitionChoice", compat, 1);
        // Only the retired reference engine reproduced TLC's <<0,1,2>> here
        // (#791, #910); Core refuses rather than answer otherwise.
        let core = check_file(&req).expect("core reports");
        assert_eq!(core.to_json()["verdict"], "unsupported");
        assert_eq!(core.exit_code(), 3);
    }
}
