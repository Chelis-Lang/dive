//! Pure protocol controls for the private Init-replay ownership path.

use super::*;
use crate::tlc::messages::{parse, parse_owned};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_recorded_initial_context_without_a_deciding_primary_stays_unclassified() {
    for raw in [
        include_str!("../../tests/fixtures/initial-context-unclassified/InitialTool.stdout"),
        include_str!("../../tests/fixtures/initial-context-unclassified/InitialOverride.stdout"),
    ] {
        let result = owned(raw, 75);
        assert_eq!(result.verdict, Verdict::Unclassified);
        assert_eq!(result.error_codes, [2102, 2103]);
        assert_eq!(result.exit_code, Some(75));
        assert!(!special(&result));
    }
}

const OUTER: &str =
    include_str!("../../tests/fixtures/init-replay/ExplicitOuterPowersetCardinality.stdout");
const BOOL: &str =
    include_str!("../../tests/fixtures/init-replay/NestedNonBooleanAfterEmpty.stdout");
const COMPONENT: &str =
    include_str!("../../tests/fixtures/init-replay/RequiredInnerComponentOverflow.stdout");

fn owned(stdout: &str, exit: i32) -> OracleResult {
    classify_owned(
        &parse_owned(stdout),
        &RunFacts {
            exit_code: Some(exit),
            stderr: Some(""),
            ..RunFacts::default()
        },
    )
}

fn special(result: &OracleResult) -> bool {
    result
        .note
        .as_deref()
        .is_some_and(|note| note.starts_with("GENERAL owns NONE cause "))
}

fn refused(stdout: &str) {
    let result = owned(stdout, 75);
    assert!(
        !special(&result),
        "unexpected ownership authority: {result:?}"
    );
    assert!(
        !result.verdict.is_oracle_fact(),
        "unexpected oracle fact: {result:?}"
    );
}

fn msg(code: i32, severity: i32, body: &str) -> String {
    format!("@!@!@STARTMSG {code}:{severity} @!@!@\n{body}\n@!@!@ENDMSG {code} @!@!@\n")
}

fn span(stdout: &str, code: i32) -> Option<std::ops::Range<usize>> {
    let start = stdout.find(&format!("@!@!@STARTMSG {code}:"))?;
    let end = format!("@!@!@ENDMSG {code} @!@!@\n");
    let finish = stdout[start..].find(&end)? + start + end.len();
    Some(start..finish)
}

fn without(stdout: &str, code: i32) -> Option<String> {
    let range = span(stdout, code)?;
    Some(format!(
        "{}{}",
        &stdout[..range.start],
        &stdout[range.end..]
    ))
}

#[test]
fn original_streams_keep_general_class_physical_exit_counts_and_output() -> TestResult {
    let metadata: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/init-replay/facts.json"))?;
    assert_eq!(
        metadata["source"],
        "4a9b1d3b8cefed628b94a4d83d813b1a3e66b402"
    );
    assert_eq!(
        metadata["oracle_source"],
        "94d0c5070852b37b801bb8998c436383ab7ee37b"
    );
    for (name, raw) in [
        ("ExplicitOuterPowersetCardinality", OUTER),
        ("NestedNonBooleanAfterEmpty", BOOL),
        ("RequiredInnerComponentOverflow", COMPONENT),
    ] {
        let facts = &metadata["cases"][name];
        assert_eq!(
            sha256_bytes(raw.as_bytes()),
            facts["stdout_sha256"].as_str().ok_or("stdout hash")?
        );
        assert_eq!(
            u64::try_from(raw.len())?,
            facts["stdout_bytes"].as_u64().ok_or("stdout bytes")?
        );
        assert_eq!(facts["exit_code"], 75);
        assert!(facts["signal"].is_null());
        assert_eq!(facts["timed_out"], false);
        assert_eq!(facts["stderr_bytes"], 0);
        let parsed = parse_owned(raw);
        assert_eq!(parsed.transcript(), &parse(raw));
        assert_eq!(parsed.transcript().anomalies, Vec::<String>::new());
        let result = owned(raw, 75);
        assert_eq!(result.exit_code, Some(75));
        assert_eq!(serde_json::to_value(&result.counts)?, facts["counts"]);
        assert!(result.trace.is_none());
        if name == "RequiredInnerComponentOverflow" {
            assert_eq!(result.verdict, Verdict::EvaluationError);
            assert_eq!(result.verdict_code, Some(2147));
            assert_eq!(result.error_codes, [1000, 2147]);
            assert!(!special(&result));
        } else {
            assert_eq!(result.verdict, Verdict::OtherError);
            assert_eq!(result.verdict_code, Some(1000));
            assert_eq!(result.error_class.as_deref(), Some("GENERAL"));
            assert_eq!(result.error_codes, [1000, 2103]);
            assert!(special(&result));
        }
    }
    assert_eq!(BOOL.matches("<<\"accepted\", {}>>").count(), 2);
    Ok(())
}

#[test]
fn public_flat_values_never_acquire_private_or_stale_ownership() {
    for raw in [OUTER, BOOL] {
        let parsed = parse_owned(raw);
        let mut flat = parsed.transcript().clone();
        let facts = RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        };
        assert_eq!(classify(&flat, &facts).verdict, Verdict::Unclassified);
        flat.segments.reverse();
        assert!(!special(&classify(&flat, &facts)));
        flat = parse(raw);
        for segment in &mut flat.segments {
            if let super::super::messages::Segment::Message(message) = segment {
                message.body.push_str("modified");
            }
        }
        flat.anomalies.push("caller anomaly".to_owned());
        flat.anomalies.clear();
        flat.segments
            .extend(parse(&msg(2215, 0, "unowned")).segments);
        assert!(!special(&classify(&flat, &facts)));
        assert!(special(&classify_owned(&parsed, &facts)));
    }
    let empty = crate::tlc::messages::Transcript::default();
    assert!(!special(&classify(
        &empty,
        &RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        }
    )));
}

#[test]
fn only_the_exact_direct_or_wrapped_owned_cause_is_authoritative() {
    let mutants = [
        BOOL.replace("1000", "5000"),
        BOOL.replace("@!@!@STARTMSG 1000:1 @!@!@\n", "")
            .replace("@!@!@ENDMSG 1000 @!@!@\n", ""),
        BOOL.replace("2215", "2179"),
        BOOL.replace("2215:0", "2215:1"),
        BOOL.replace("1000:1", "1000:0"),
        BOOL.replace(
            "@!@!@STARTMSG 2215:0",
            "unexplained exception\n@!@!@STARTMSG 2215:0",
        ),
        BOOL.replace(
            "@!@!@ENDMSG 2215 @!@!@\n",
            &format!("@!@!@ENDMSG 2215 @!@!@\n{}", msg(2215, 0, "second cause")),
        ),
        OUTER.replace("2178", "2215"),
        OUTER.replace("2154:0", "2154:1"),
        OUTER.replace(
            ",\nbut it produced the following error:",
            ",\nunknown wrapper:",
        ),
        BOOL.replace(
            "TLC expected a boolean value, but did not find one. ",
            "unknown error: ",
        ),
        OUTER.replace("Overflow when computing ", "unknown overflow: "),
    ];
    for mutant in mutants {
        refused(&mutant);
    }
}

#[test]
fn sibling_and_unrelated_nested_frames_do_not_supply_a_cause() -> TestResult {
    let leaf = span(BOOL, 2215).ok_or("leaf")?;
    let general = span(BOOL, 1000).ok_or("GENERAL")?;
    let sibling = format!(
        "{}{}{}{}",
        &BOOL[..general.start],
        msg(1000, 1, ""),
        &BOOL[leaf],
        &BOOL[general.end..]
    );
    refused(&sibling);
    let extra = BOOL.replace(
        "@!@!@ENDMSG 1000 @!@!@",
        &format!("{}@!@!@ENDMSG 1000 @!@!@", msg(4000, 0, "unknown child")),
    );
    refused(&extra);
    refused(&(BOOL.to_owned() + &msg(4000, 1, "conflicting error")));
    refused(&(BOOL.to_owned() + &msg(2215, 0, "unowned extra cause")));
    refused(&(BOOL.to_owned() + &msg(49999, 0, "unknown tool message")));
    let direct_overflow = OUTER
        .replace("@!@!@STARTMSG 2154:0 @!@!@\n", "")
        .replace("@!@!@ENDMSG 2154 @!@!@\n", "");
    refused(&direct_overflow);
    Ok(())
}

#[test]
fn new_route_rejects_midline_quoted_indented_and_legacy_colon_placement() {
    for code in [2185, 2189, 1000, 2215, 2103, 2199, 2194, 2186] {
        for prefix in ["arbitrary ", "\"", " ", ": "] {
            let marker = format!("@!@!@STARTMSG {code}:");
            refused(&BOOL.replacen(&marker, &format!("{prefix}{marker}"), 1));
        }
    }
    for code in [2154, 2178] {
        refused(&OUTER.replacen(
            &format!("@!@!@STARTMSG {code}:"),
            &format!("prefix @!@!@STARTMSG {code}:"),
            1,
        ));
    }
    refused(&BOOL.replace("@!@!@STARTMSG 2215:0 @!@!@", "@!@!STARTMSG 2215:0 @!@!@"));
    let printed = msg(2178, 0, "not an actual message").replace('\n', "\\n");
    let with_print = format!("\"{printed}\"\n{BOOL}");
    assert!(special(&owned(&with_print, 75)));
    assert_eq!(
        parse(&with_print).messages().count(),
        parse(BOOL).messages().count()
    );
}

#[test]
fn malformed_crossed_duplicate_or_unclosed_frames_are_not_repaired() {
    for mutant in [
        BOOL.replace("@!@!@ENDMSG 2215 @!@!@\n", ""),
        BOOL.replace("@!@!@ENDMSG 2215 @!@!@", "@!@!@ENDMSG 1000 @!@!@"),
        BOOL.replace(
            "@!@!@ENDMSG 1000 @!@!@\n",
            "@!@!@ENDMSG 1000 @!@!@\n@!@!@ENDMSG 1000 @!@!@\n",
        ),
        BOOL.replace("@!@!@STARTMSG 2215:0 @!@!@", "@!@!@STARTMSG bad:0 @!@!@"),
        BOOL.replace("@!@!@STARTMSG 2215:0 @!@!@", "@!@!@STARTMSG 2215:0"),
    ] {
        refused(&mutant);
    }
}

#[test]
fn wrapper_child_must_occupy_the_original_error_slot() -> TestResult {
    let leaf = span(OUTER, 2178).ok_or("overflow cause")?;
    let frame = &OUTER[leaf.clone()];
    let rest = without(OUTER, 2178).ok_or("overflow cause")?;
    // All three retain the same flattened wrapper body and ancestry. Only
    // scanner-owned parent-prefix positions distinguish their placements.
    for needle in [
        "Attempted to apply the operator overridden by the Java method",
        "public static tlc2.value.impl.IntValue",
        ",\nbut it produced the following error:",
    ] {
        let mutant = rest.replacen(needle, &format!("{frame}{needle}"), 1);
        assert_ne!(mutant, rest);
        refused(&mutant);
    }
    let wrapper = span(&rest, 2154).ok_or("wrapper")?;
    let after_wrapper = format!("{}{}{}", &rest[..wrapper.end], frame, &rest[wrapper.end..]);
    refused(&after_wrapper);
    let trailing = OUTER.replace("@!@!@ENDMSG 2154 @!@!@", "\n@!@!@ENDMSG 2154 @!@!@");
    refused(&trailing);
    Ok(())
}

#[test]
fn canonical_headers_and_exhaustive_statistics_are_required() {
    for code in [2185, 2189, 1000, 2154, 2178, 2103, 2199, 2194, 2186] {
        for raw in [OUTER, BOOL] {
            let marker = format!("STARTMSG {code}:");
            if raw.contains(marker.as_str()) {
                refused(&raw.replacen(marker.as_str(), &format!("STARTMSG +{code}:"), 1));
                let end = format!("ENDMSG {code} @!@!@");
                refused(&raw.replacen(end.as_str(), &format!("ENDMSG  {code} @!@!@"), 1));
            }
        }
    }
    for code in [2188, 2210] {
        let mutant = BOOL.to_owned() + &msg(code, 0, "123 states generated, 0 traces generated.");
        refused(&mutant);
    }
}

#[test]
fn lifecycle_is_unique_complete_top_level_and_has_not_completed_init() -> TestResult {
    for code in [2185, 2189, 1000, 2103, 2199, 2194, 2186] {
        refused(&without(BOOL, code).ok_or("required phase")?);
        let phase = span(BOOL, code).ok_or("required phase")?;
        refused(&(BOOL.to_owned() + &BOOL[phase]));
    }
    for code in [2190, 2191] {
        let completed = BOOL.replace(
            "@!@!@STARTMSG 1000:1",
            &format!(
                "{}@!@!@STARTMSG 1000:1",
                msg(code, 0, "Finished computing initial states")
            ),
        );
        refused(&completed);
    }
    refused(&BOOL.replace("2189:0", "2189:3"));
    refused(&BOOL.replace("Computing initial states...", "another phase"));
    refused(&BOOL.replace("Starting... (", "unknown start ("));
    refused(&BOOL.replace("Finished in ", "unknown finish "));
    refused(&BOOL.replace(
        "1 states generated, 1 distinct states found, 1 states left on queue.",
        "unknown states generated, 1 distinct states found, 1 states left on queue.",
    ));
    Ok(())
}

#[test]
fn replay_context_must_follow_general_and_have_real_stack_structure() -> TestResult {
    let context = span(BOOL, 2103).ok_or("context")?;
    let frame = &BOOL[context];
    let rest = without(BOOL, 2103).ok_or("context")?;
    refused(&(frame.to_owned() + &rest));
    refused(&rest.replace(
        "@!@!@STARTMSG 2215:0",
        &format!("{frame}@!@!@STARTMSG 2215:0"),
    ));
    refused(&BOOL.replace("2103:1", "2103:0"));
    refused(&BOOL.replace("0. Line ", "1. Line "));
    refused(&BOOL.replace(
        "The error occurred when TLC was evaluating the nested",
        "unknown stack",
    ));
    refused(&BOOL.replace("0. Line 4, column 9", "0. Line 0, column 9"));
    Ok(())
}

#[test]
fn exit_resource_stderr_state_and_trace_guards_keep_original_authority() {
    for exit in [0, 12, 255, 3] {
        let result = owned(BOOL, exit);
        assert!(!special(&result));
        if exit == 255 {
            assert_eq!(result.verdict, Verdict::OtherError);
            assert_eq!(result.verdict_code, Some(1000));
        }
    }
    for facts in [
        RunFacts {
            exit_code: None,
            ..RunFacts::default()
        },
        RunFacts {
            exit_code: Some(75),
            timed_out: true,
            ..RunFacts::default()
        },
        RunFacts {
            exit_code: Some(75),
            simulate: true,
            ..RunFacts::default()
        },
        RunFacts {
            exit_code: Some(75),
            trace_json: Some("{}"),
            ..RunFacts::default()
        },
        RunFacts {
            exit_code: Some(75),
            stderr: Some("unexpected output"),
            ..RunFacts::default()
        },
    ] {
        let result = classify_owned(&parse_owned(BOOL), &facts);
        assert!(!result.verdict.is_oracle_fact());
    }
    refused(&(BOOL.to_owned() + &msg(2217, 4, "1: <Initial predicate>\nx = 0")));
    refused(&(BOOL.to_owned() + &msg(2193, 0, "No error has been found.")));
    let oom = BOOL.to_owned() + &msg(1002, 1, "Out Of Memory.");
    assert_eq!(owned(&oom, 75).verdict, Verdict::ResourceLimit);
    let stack = BOOL.replace(
        "@!@!@STARTMSG 2215:0",
        "StackOverflowError\n@!@!@STARTMSG 2215:0",
    );
    assert_eq!(owned(&stack, 75).verdict, Verdict::ResourceLimit);
}

#[test]
fn general_255_and_opaque_descriptor_and_location_identity_are_preserved() {
    for code in [2179, 5006] {
        let stream = msg(1000, 1, &msg(code, 0, "nested original cause"));
        let result = owned(&stream, 255);
        assert_eq!(result.verdict, Verdict::OtherError);
        assert_eq!(result.verdict_code, Some(1000));
        assert_eq!(result.error_codes, [1000]);
        assert_eq!(result.note, None);
    }
    let other_method = OUTER.replace("public static tlc2.value.impl.IntValue tlc2.module.FiniteSets.Cardinality(tlc2.value.impl.Value)", "another opaque method descriptor");
    assert_ne!(other_method, OUTER);
    assert!(special(&owned(&other_method, 75)));
    let other_module = BOOL.replace("NestedNonBooleanAfterEmpty", "AnotherModule");
    assert!(special(&owned(&other_module, 75)));
}
