//! Strict module-cause ownership and actual candidate-word consumer controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "pinned protocol controls"
)]

use super::{OracleResult, RunFacts, Verdict, classify, classify_owned};
use crate::compare::{CompareContext, diff_results};
use crate::dive::{CheckMapping, check_result};
use crate::process::ProcessOutcome;
use crate::tlc::messages;
use crate::trace_check::validate_counterexample;
use dive_api::trace::{SpecModel, TraceOptions};
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn raw(kind: &str) -> String {
    std::fs::read_to_string(repo().join(format!(
        "crates/dive-check/tests/fixtures/errors/tlc-2026.10.06.014338/ErProp{kind}.stdout"
    )))
    .expect("pinned clean stream")
}

fn facts(exit: i32) -> RunFacts<'static> {
    RunFacts {
        exit_code: Some(exit),
        ..RunFacts::default()
    }
}

fn special(stdout: &str, exit: i32) -> bool {
    classify_owned(&messages::parse_owned(stdout), &facts(exit))
        .note
        .as_deref()
        .is_some_and(|note| note.starts_with("implied-action replay owns NONE cause "))
}

fn message(code: i32, severity: i32, body: &str) -> String {
    format!("@!@!@STARTMSG {code}:{severity} @!@!@\n{body}\n@!@!@ENDMSG {code} @!@!@\n")
}

fn block(stdout: &str, code: i32) -> String {
    let start = stdout
        .find(&format!("@!@!@STARTMSG {code}:"))
        .expect("frame start");
    let suffix = format!("@!@!@ENDMSG {code} @!@!@\n");
    let end = stdout[start..].find(&suffix).expect("frame end") + start + suffix.len();
    stdout[start..end].to_owned()
}

#[test]
fn only_an_unambiguous_owned_assertion_exit_gains_the_deciding_identity() {
    for (kind, code, exit) in [("Assert", 2132, 14)] {
        let text = raw(kind);
        assert!(special(&text, exit));
        let parsed = messages::parse_owned(&text);
        let result = classify_owned(&parsed, &facts(exit));
        assert_eq!(result.verdict_code, Some(code));
        assert_eq!(result.exit_code, Some(exit));
        assert_eq!(result.error_codes, [2113, 2121, 2103]);
        assert_eq!(
            result.counts,
            classify(&messages::parse(&text), &facts(exit)).counts
        );
        assert!(
            !classify(&messages::parse(&text), &facts(exit))
                .verdict
                .is_oracle_fact()
        );
        let mut flat = messages::parse(&text);
        flat.segments.reverse();
        assert!(!classify(&flat, &facts(exit)).verdict.is_oracle_fact());
        assert!(
            special(&text, exit),
            "a mutable copy cannot change the owned carrier"
        );
        for wrong in [0, 13, 75, 76] {
            assert!(!special(&text, wrong));
        }
        for altered in [
            RunFacts {
                simulate: true,
                ..facts(exit)
            },
            RunFacts {
                trace_json: Some("{}"),
                ..facts(exit)
            },
        ] {
            assert!(!classify_owned(&parsed, &altered).verdict.is_oracle_fact());
        }
    }
    for kind in ["DivZero", "Enabled", "Overflow", "Seen"] {
        let text = raw(kind);
        assert!(!special(&text, 255));
        assert_eq!(
            classify_owned(&messages::parse_owned(&text), &facts(255)).verdict,
            Verdict::Unclassified,
            "a primary child and physical255 cannot identify the diagnostic module cause"
        );
    }
}

#[test]
fn altered_parent_child_order_classes_and_injected_markers_supply_no_authority() {
    let text = raw("Assert");
    let child = block(&text, 2132);
    let parent = block(&text, 2113);
    let header = block(&text, 2121);
    let first_state = block(&text, 2217);
    let child_outside = text.replace(&parent, &format!("{}{child}", parent.replace(&child, "")));
    let reversed = text.replace(&format!("{parent}{header}"), &format!("{header}{parent}"));
    let mutants = [
        text.replace("2113", "2102"),
        text.replace("2113:1", "2113:0"),
        text.replace("2132:0", "2132:1"),
        text.replace("2132", "2215"),
        text.replace(
            "Evaluating action property Prop failed.",
            "unproved wrapper",
        ),
        text.replace("2132:0", "2132: 0"),
        text.replace("@!@!@ENDMSG 2132 @!@!@", "@!@!@ENDMSG 2132  @!@!@"),
        text.replace("@!@!@STARTMSG 2132:0", "quoted @!@!@STARTMSG 2132:0"),
        text.replace(&child, &format!("{child}{child}")),
        text.replace(&child, &format!("{child}unproved suffix\n")),
        text.replace(
            &parent,
            &format!("{parent}{}", message(2178, 0, "unrelated module cause")),
        ),
        text.replace(
            &parent,
            &format!("{parent}{}", message(1000, 0, "unrelated general cause")),
        ),
        format!("{text}{}", message(2179, 0, "trailing cause")),
        format!("{text}{}", message(2262, 0, "trailing known frame")),
        text.replace(
            &parent,
            &format!("{}{}", parent, message(1000, 1, "extra error")),
        ),
        text.replace(
            &first_state,
            &first_state.replace("1: <Initial predicate>", "2: <Initial predicate>"),
        ),
        text.replace(&first_state, &format!("{first_state}{first_state}")),
        text.replace(&first_state, ""),
        text.replace(&block(&text, 2103), ""),
        text.replace(&block(&text, 2199), ""),
        text.replace(&child, ""),
        text.replace(&child, &format!("\"{}\"\n", child.replace('\n', "\\n"))),
        child_outside,
        reversed,
    ];
    for (index, mutant) in mutants.into_iter().enumerate() {
        assert!(!special(&mutant, 14), "mutant {index}");
    }
    // An initial context-only stream cannot inherit this post-Init proof.
    let initial = text.replace("2113", "2102");
    assert_eq!(
        classify_owned(&messages::parse_owned(&initial), &facts(75)).verdict,
        Verdict::Unclassified
    );
}

#[test]
fn every_admitted_frame_keeps_its_severity_and_unknown_frames_never_borrow_authority() {
    let text = raw("Assert");
    assert!(special(&text, 14));
    for frame in messages::parse(&text).messages() {
        let noncanonical = text.replacen(
            &format!("@!@!@STARTMSG {}:{} @!@!@", frame.code, frame.class),
            &format!("@!@!@STARTMSG {}: {} @!@!@", frame.code, frame.class),
            1,
        );
        assert!(
            !special(&noncanonical, 14),
            "noncanonical frame {}",
            frame.code
        );
        for wrong in 0..=4 {
            if wrong == frame.class {
                continue;
            }
            let mutant = text.replacen(
                &format!("@!@!@STARTMSG {}:{} @!@!@", frame.code, frame.class),
                &format!("@!@!@STARTMSG {}:{wrong} @!@!@", frame.code),
                1,
            );
            assert!(
                !special(&mutant, 14),
                "changed frame {} to class {wrong}",
                frame.code
            );
        }
    }
    let parent = block(&text, 2113);
    for code in [1000, 2178, 2179] {
        for severity in 0..=4 {
            let unrelated = message(code, severity, "unrelated cause");
            for mutant in [
                format!("{text}{unrelated}"),
                text.replace(&parent, &format!("{parent}{unrelated}")),
            ] {
                assert!(
                    !special(&mutant, 14),
                    "unrelated code {code} class {severity}"
                );
            }
        }
    }
}
fn native_at_workers(file: &Path, engine: Engine, compat: bool, workers: usize) -> OracleResult {
    let report = check_file_engine(
        &CheckRequest {
            file: file.into(),
            config: Some(file.with_extension("cfg")),
            compat_tlc: compat,
            workers: Some(workers),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("native check");
    let outcome = ProcessOutcome {
        exit_code: Some(i32::from(
            report.exit_code_for(dive_api::ExitConvention::Tlc),
        )),
        signal: None,
        timed_out: false,
        stdout: report.to_json().to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    };
    let CheckMapping::Result(result) = check_result(&outcome, compat) else {
        panic!("native classification")
    };
    *result
}

fn model(file: &Path, compat: bool) -> SpecModel {
    SpecModel::load(
        file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: compat,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model")
}

#[test]
fn candidate_words_require_actual_replay_and_ambiguous_oracles_cannot_earn_agreement() {
    let root = repo().join("crates/dive-check/tests/fixtures/errors");
    let mut disagreements = Vec::new();
    for site in ["Constr", "ActConstr", "Prop"] {
        for kind in ["Assert", "DivZero", "Enabled", "Overflow", "Seen"] {
            if kind == "Seen" && site != "Prop" {
                continue;
            }
            let name = format!("Er{site}{kind}");
            let file = root.join(format!("specs/{name}/{name}.tla"));
            let clean =
                std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                    .expect("pinned stream");
            let exit =
                std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                    .expect("pinned exit")
                    .trim()
                    .parse()
                    .expect("integer exit");
            let transcript = messages::parse(&clean);
            assert_eq!(transcript.anomalies, Vec::<String>::new());
            let oracle = classify_owned(
                &messages::parse_owned(&clean),
                &RunFacts {
                    exit_code: Some(exit),
                    ..RunFacts::default()
                },
            );
            for compat in [false, true] {
                if kind == "Overflow" && !compat {
                    continue;
                }
                let spec = model(&file, compat);
                for (engine, workers) in [Engine::Core, Engine::Reference]
                    .into_iter()
                    .flat_map(|engine| [1, 4].map(|workers| (engine, workers)))
                {
                    let actual = native_at_workers(&file, engine, compat, workers);
                    let diffs = diff_results(
                        &oracle,
                        &actual,
                        CompareContext {
                            workers: u32::try_from(workers).expect("worker count"),
                            counts_comparable: true,
                        },
                    );
                    if site == "Prop" && kind != "Assert" {
                        assert_eq!(oracle.verdict, Verdict::Unclassified, "{name}");
                        assert!(
                            !diffs.is_empty(),
                            "an ambiguous oracle cannot earn agreement"
                        );
                    } else if !diffs.is_empty() {
                        disagreements.push(format!(
                            "{name}/{compat}/{engine:?}/{workers}: oracle {oracle:?}; differences {diffs:?}"
                        ));
                    }
                    let proof = validate_counterexample(&spec, &actual, &oracle);
                    assert!(proof.validated, "{name}: {proof:?}");
                    let mut bad = actual.clone();
                    bad.error_class = Some("GENERAL".into());
                    assert!(validate_counterexample(&spec, &bad, &oracle).refuted);
                    let mut bad = actual.clone();
                    bad.trace
                        .as_mut()
                        .and_then(|trace| trace.json.as_mut())
                        .expect("actual word")["ending"]["endpoint"]["source"]["module"] =
                        json!("Forged");
                    assert!(validate_counterexample(&spec, &bad, &oracle).refuted);
                    let mut absent = actual;
                    absent.trace = None;
                    assert!(!validate_counterexample(&spec, &absent, &oracle).validated);
                }
            }
        }
    }
    assert_eq!(disagreements, Vec::<String>::new());
}

#[test]
fn different_actual_diagnostic_causes_sharing_255_are_not_canonical_oracle_facts() {
    let fixture = repo().join("crates/dive-api/tests/fixtures/candidate-diagnostic-mismatch");
    let metadata: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.join("oracle.json")).expect("pinned facts"),
    )
    .expect("JSON facts");
    for name in ["LaterDivDiagnosticOverflow", "LaterOverflowDiagnosticDiv"] {
        let raw = std::fs::read_to_string(repo().join(format!(
            "crates/dive-conformance/tests/fixtures/candidate-diagnostic-mismatch/{name}.stdout"
        )))
        .expect("unchanged normal pinned stream");
        let record = &metadata["cases"][name];
        assert_eq!(
            crate::hash::sha256_bytes(raw.as_bytes()),
            record["normal_stdout_sha256"]
        );
        for ext in ["tla", "cfg"] {
            assert_eq!(
                crate::hash::sha256_bytes(
                    &std::fs::read(fixture.join(format!("{name}.{ext}")))
                        .expect("minimal source/config")
                ),
                record[format!("{ext}_sha256")]
            );
        }
        assert_ne!(
            record["primary_none_code"],
            record["diagnostic_bridge_code"]
        );
        let actual = classify_owned(&messages::parse_owned(&raw), &facts(255));
        assert_eq!(actual.verdict, Verdict::Unclassified);
        assert_eq!(actual.exit_code, Some(255));
        assert!(!special(&raw, 255));
    }
}
