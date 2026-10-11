//! Classification of real TLC and SANY transcripts captured from both pinned
//! oracles (`tests/fixtures/tlc/<oracle>/`), produced by
//! `dive-conformance dev capture` from the specs in `tests/fixtures/specs/`.
//! No network or Java is needed.

use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::golden::{
    GOLDEN_SCHEMA, GoldenModel, ModelGolden, Provenance, payload, to_pretty,
};
use dive_conformance::oracle::{Capabilities, Java, JavaSource, OracleProvenance};
use dive_conformance::sany::{self, SanyVerdict};
use dive_conformance::tlc::messages;
use dive_conformance::tlc::result::{OracleResult, RunFacts, TraceEnding, Verdict, classify};
use std::path::{Path, PathBuf};
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NEW: &str = "tlc-2026.10.06.014338";
const OLD: &str = "tlc-1.7.4";

fn dir(oracle: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tlc")
        .join(oracle)
}

fn read(oracle: &str, file: &str) -> std::io::Result<String> {
    std::fs::read_to_string(dir(oracle).join(file))
}

fn exit_of(text: &str) -> Option<i32> {
    text.trim().parse().ok()
}

fn tlc(oracle: &str, name: &str) -> Result<OracleResult, Box<dyn std::error::Error>> {
    let stdout = read(oracle, &format!("{name}.stdout"))?;
    let exit = exit_of(&read(oracle, &format!("{name}.exit"))?);
    let json = read(oracle, &format!("{name}.trace.json")).ok();
    let stderr = read(oracle, &format!("{name}.stderr"))?;
    Ok(classify(
        &messages::parse(&stdout),
        &RunFacts {
            exit_code: exit,
            timed_out: false,
            trace_json: json.as_deref(),
            simulate: false,
            stderr: Some(&stderr),
        },
    ))
}

/// What a fixture must classify as. `depth_old` differs because v1.7.4 does
/// not report the search depth after assertion, evaluation, overflow and
/// liveness errors.
struct Expect {
    name: &'static str,
    verdict: Verdict,
    class: &'static str,
    exit: i32,
    property: Option<&'static str>,
    distinct: Option<u64>,
    generated: Option<u64>,
    depth_new: Option<u64>,
    depth_old: Option<u64>,
    trace: Option<(u64, TraceEnding)>,
    json_new: bool,
}

#[allow(clippy::too_many_lines, reason = "a table of expectations")]
fn expectations() -> Vec<Expect> {
    let e = |name, verdict, class, exit| Expect {
        name,
        verdict,
        class,
        exit,
        property: None,
        distinct: None,
        generated: None,
        depth_new: None,
        depth_old: None,
        trace: None,
        json_new: false,
    };
    vec![
        Expect {
            distinct: Some(4),
            generated: Some(5),
            depth_new: Some(4),
            depth_old: Some(4),
            ..e("Success", Verdict::Success, "TLC_SUCCESS", 0)
        },
        Expect {
            distinct: Some(4),
            generated: Some(4),
            depth_new: Some(4),
            depth_old: Some(4),
            ..e("PrintOutput", Verdict::Success, "TLC_SUCCESS", 0)
        },
        Expect {
            property: Some("Small"),
            distinct: Some(4),
            generated: Some(4),
            depth_new: Some(4),
            depth_old: Some(4),
            trace: Some((4, TraceEnding::Finite)),
            json_new: true,
            ..e(
                "InvariantViolation",
                Verdict::SafetyViolation,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                12,
            )
        },
        Expect {
            property: Some("Small"),
            trace: Some((1, TraceEnding::Finite)),
            json_new: true,
            ..e(
                "InitInvariant",
                Verdict::SafetyViolation,
                "TLC_INVARIANT_VIOLATED_INITIAL",
                12,
            )
        },
        Expect {
            distinct: Some(3),
            generated: Some(3),
            depth_new: Some(3),
            depth_old: Some(3),
            trace: Some((3, TraceEnding::Finite)),
            json_new: true,
            ..e("Deadlock", Verdict::Deadlock, "TLC_DEADLOCK_REACHED", 11)
        },
        Expect {
            property: Some("Reaches5"),
            distinct: Some(3),
            generated: Some(4),
            depth_new: Some(3),
            trace: Some((3, TraceEnding::Lasso { back_to: Some(1) })),
            json_new: true,
            ..e(
                "Liveness",
                Verdict::LivenessViolation,
                "TLC_TEMPORAL_PROPERTY_VIOLATED",
                13,
            )
        },
        Expect {
            property: Some("Reaches2"),
            distinct: Some(3),
            generated: Some(3),
            depth_new: Some(3),
            trace: Some((2, TraceEnding::Stuttering)),
            json_new: true,
            ..e(
                "LivenessStutter",
                Verdict::LivenessViolation,
                "TLC_TEMPORAL_PROPERTY_VIOLATED",
                13,
            )
        },
        Expect {
            property: Some("Monotone"),
            distinct: Some(3),
            generated: Some(4),
            depth_new: Some(3),
            depth_old: Some(3),
            trace: Some((4, TraceEnding::Finite)),
            json_new: true,
            ..e(
                "ActionProperty",
                Verdict::ActionPropertyViolation,
                "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR",
                13,
            )
        },
        Expect {
            property: Some("line 4, col 8 to line 4, col 13 of module Assumption"),
            ..e(
                "Assumption",
                Verdict::AssumptionFailure,
                "TLC_ASSUMPTION_FALSE",
                10,
            )
        },
        Expect {
            distinct: Some(3),
            generated: Some(3),
            depth_new: Some(3),
            trace: Some((3, TraceEnding::Finite)),
            ..e(
                "AssertFailure",
                Verdict::AssertionFailure,
                "TLC_VALUE_ASSERT_FAILED",
                14,
            )
        },
        Expect {
            distinct: Some(2),
            generated: Some(2),
            depth_new: Some(2),
            trace: Some((2, TraceEnding::Finite)),
            ..e(
                "EvalError",
                Verdict::EvaluationError,
                "TLC_FINGERPRINT_EXCEPTION",
                75,
            )
        },
        Expect {
            distinct: Some(4),
            generated: Some(4),
            depth_new: Some(4),
            trace: Some((4, TraceEnding::Finite)),
            ..e("Overflow", Verdict::Overflow, "TLC_MODULE_OVERFLOW", 255)
        },
        Expect {
            property: Some("StartsAtZero"),
            trace: Some((1, TraceEnding::Finite)),
            json_new: true,
            ..e(
                "InitProperty",
                Verdict::SafetyViolation,
                "TLC_PROPERTY_VIOLATED_INITIAL",
                13,
            )
        },
        Expect {
            distinct: Some(3),
            generated: Some(3),
            depth_new: Some(3),
            depth_old: Some(3),
            trace: Some((3, TraceEnding::Finite)),
            json_new: true,
            ..e("PrintMarker", Verdict::Deadlock, "TLC_DEADLOCK_REACHED", 11)
        },
        e("ParseError", Verdict::ParseError, "TLC_PARSING_FAILED", 150),
        e(
            "SemanticError",
            Verdict::SemanticError,
            "TLC_PARSING_FAILED",
            150,
        ),
        e(
            "ConfigError",
            Verdict::ConfigError,
            "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
            151,
        ),
        e(
            "MissingModule",
            Verdict::ParseError,
            "TLC_PARSING_FAILED",
            150,
        ),
    ]
}

#[test]
fn both_oracles_classify_every_fixture() -> TestResult {
    for oracle in [NEW, OLD] {
        for x in expectations() {
            let r = tlc(oracle, x.name)?;
            let ctx = format!("{oracle} {}", x.name);
            assert_eq!(r.verdict, x.verdict, "{ctx}: {:?}", r.note);
            assert_eq!(r.error_class.as_deref(), Some(x.class), "{ctx}");
            assert_eq!(r.exit_code, Some(x.exit), "{ctx}");
            // v1.7.4 prints "Temporal properties were violated." without a name.
            let property = if oracle == OLD && x.verdict == Verdict::LivenessViolation {
                None
            } else {
                x.property
            };
            assert_eq!(r.violated_property.as_deref(), property, "{ctx}");
            assert_eq!(r.counts.distinct_states, x.distinct, "{ctx}");
            assert_eq!(r.counts.states_generated, x.generated, "{ctx}");
            let depth = if oracle == NEW {
                x.depth_new
            } else {
                x.depth_old
            };
            assert_eq!(r.counts.depth, depth, "{ctx}");
            let trace = r.trace.as_ref().map(|t| (t.length, t.ending.clone()));
            assert_eq!(trace, x.trace, "{ctx}");
            let has_json = r.trace.as_ref().is_some_and(|t| t.json.is_some());
            assert_eq!(has_json, oracle == NEW && x.json_new, "{ctx}");
            if let Some(t) = &r.trace {
                // Without JSON the message-stream states are kept instead.
                assert_eq!(t.steps.is_some(), !has_json, "{ctx}");
            }
            assert_eq!(r.note, None, "{ctx}");
        }
    }
    Ok(())
}

#[test]
fn nested_config_error_in_1_7_4() -> TestResult {
    // v1.7.4 does not know the ALIAS keyword; it reports CFG_EXPECTED_SYMBOL
    // (class 0) *inside* GENERAL (1000) and exits 255. The nesting is parsed,
    // and the verdict is what TLC's own exit status and error message say:
    // other-error / GENERAL. 94d0c50 accepts ALIAS.
    let stdout = read(OLD, "CfgAlias.stdout")?;
    let t = messages::parse(&stdout);
    assert_eq!(t.anomalies, Vec::<String>::new());
    let codes: Vec<i32> = t.messages().map(|m| m.code).collect();
    let general = codes.iter().position(|c| *c == 1000).ok_or("no 1000")?;
    assert_eq!(codes.get(general + 1), Some(&5006));
    let old = tlc(OLD, "CfgAlias")?;
    assert_eq!(
        (old.verdict, old.error_class.as_deref(), old.exit_code),
        (Verdict::OtherError, Some("GENERAL"), Some(255))
    );
    assert_eq!(tlc(NEW, "CfgAlias")?.verdict, Verdict::Success);
    Ok(())
}

#[test]
fn success_reports_collision_probability() -> TestResult {
    for oracle in [NEW, OLD] {
        let r = tlc(oracle, "Success")?;
        let p = r.fingerprint_collision.ok_or("no probability")?;
        assert_eq!(p.optimistic.as_deref(), Some("val = 2.2E-19"), "{oracle}");
    }
    Ok(())
}

#[test]
fn missing_modules_are_named() -> TestResult {
    for oracle in [NEW, OLD] {
        assert_eq!(
            tlc(oracle, "MissingModule")?.unresolved_modules,
            vec!["NoSuchModule".to_owned()]
        );
    }
    // v1.7.4 runs without CommunityModules (see conformance/oracles.toml).
    let new = tlc(NEW, "UsesCommunityModules")?;
    assert_eq!(new.verdict, Verdict::Success);
    assert_eq!(new.counts.distinct_states, Some(7));
    let old = tlc(OLD, "UsesCommunityModules")?;
    assert_eq!(old.verdict, Verdict::ParseError);
    assert_ne!(old.unresolved_modules, [] as [std::string::String; 0]);
    Ok(())
}

#[test]
fn cross_oracle_diffs_show_version_differences() -> TestResult {
    let one = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    // Same facts.
    let a = tlc(NEW, "InvariantViolation")?;
    let b = tlc(OLD, "InvariantViolation")?;
    assert_eq!(diff_results(&a, &b, one), vec![]);
    // v1.7.4 omits the depth after an assertion failure.
    let a = tlc(NEW, "AssertFailure")?;
    let b = tlc(OLD, "AssertFailure")?;
    let fields: Vec<String> = diff_results(&a, &b, one)
        .into_iter()
        .map(|d| d.field)
        .collect();
    assert_eq!(fields, ["counts.depth"]);
    Ok(())
}

fn sany_fixture(oracle: &str, name: &str) -> Result<sany::SanyResult, Box<dyn std::error::Error>> {
    let outcome = dive_conformance::process::ProcessOutcome {
        exit_code: exit_of(&read(oracle, &format!("{name}.sany.exit"))?),
        signal: None,
        timed_out: false,
        stdout: read(oracle, &format!("{name}.sany.stdout"))?,
        stderr: read(oracle, &format!("{name}.sany.stderr"))?,
        elapsed: Duration::ZERO,
    };
    Ok(sany::classify(&outcome))
}

#[test]
fn sany_exit_codes_per_version() -> TestResult {
    // 94d0c50 with -error-codes: semantic errors exit 4.
    let r = sany_fixture(NEW, "SemanticError")?;
    assert_eq!(
        (r.verdict, r.exit_code, r.exit_code_reflects_errors),
        (SanyVerdict::SemanticError, Some(4), true)
    );
    // v1.7.4 (no -error-codes): semantic errors exit 0 but are rejections.
    let r = sany_fixture(OLD, "SemanticError")?;
    assert_eq!(
        (r.verdict, r.exit_code, r.exit_code_reflects_errors),
        (SanyVerdict::SemanticError, Some(0), false)
    );
    assert!(!r.accepted);
    for oracle in [NEW, OLD] {
        for name in ["ParseError", "MissingModule"] {
            let r = sany_fixture(oracle, name)?;
            assert_eq!(
                (r.verdict, r.exit_code),
                (SanyVerdict::ParseError, Some(255)),
                "{oracle} {name}"
            );
        }
        for name in [
            "Success",
            "Deadlock",
            "Overflow",
            "ConfigError",
            "Assumption",
        ] {
            let r = sany_fixture(oracle, name)?;
            assert!(r.accepted, "{oracle} {name}: {:?}", r.error_text);
        }
    }
    assert_eq!(
        sany_fixture(OLD, "UsesCommunityModules")?.verdict,
        SanyVerdict::ParseError
    );
    assert!(sany_fixture(NEW, "UsesCommunityModules")?.accepted);
    // v1.7.4 prints a "Semantic errors:" header above a warnings-only list
    // ("Multiple declarations or definitions for symbol Foo") and accepts.
    for oracle in [NEW, OLD] {
        let r = sany_fixture(oracle, "SanyWarning")?;
        assert_eq!(
            r.verdict,
            SanyVerdict::Accept,
            "{oracle}: {:?}",
            r.error_text
        );
    }
    assert!(read(OLD, "SanyWarning.sany.stdout")?.contains("Semantic errors:"));
    assert_eq!(tlc(OLD, "SanyWarning")?.verdict, Verdict::Success);
    Ok(())
}

fn golden_for(result: OracleResult, generated_at: &str) -> ModelGolden {
    ModelGolden {
        schema: GOLDEN_SCHEMA,
        kind: "tlc-model".to_owned(),
        oracle: NEW.to_owned(),
        corpus: "fixtures".to_owned(),
        model: GoldenModel {
            id: "Liveness/Liveness.cfg".to_owned(),
            module: "Liveness/Liveness.tla".to_owned(),
            mode: dive_conformance::corpus::ModelMode::Exhaustive,
            features: vec![],
            counts_comparable: true,
        },
        workers: 1,
        result,
        provenance: Provenance {
            oracle: OracleProvenance {
                id: NEW.to_owned(),
                sha256: "0".repeat(64),
                upstream_commit: "94d0c50".to_owned(),
                classpath: vec![],
                module_libraries: vec![],
                capabilities: Capabilities::default(),
                java: Java {
                    path: PathBuf::new(),
                    source: JavaSource::Pinned {
                        version: "17".to_owned(),
                        sha256: "1".repeat(64),
                        platform: "test".to_owned(),
                    },
                    version_line: "openjdk".to_owned(),
                },
            },
            corpus_commit: "c".repeat(40),
            command: vec!["java".to_owned()],
            trace_command: None,
            timeout_seconds: 60,
            reason: None,
            generator: "test".to_owned(),
            generated_at: generated_at.to_owned(),
            elapsed_ms: 1,
            uncompared_counts: None,
        },
    }
}

#[test]
fn golden_round_trip_and_determinism() -> TestResult {
    let first = to_pretty(&golden_for(tlc(NEW, "Liveness")?, "2026-01-01T00:00:00Z"))?;
    let second = to_pretty(&golden_for(tlc(NEW, "Liveness")?, "2027-01-01T00:00:00Z"))?;
    // Only provenance differs, so the payloads are identical.
    assert_ne!(first, second);
    assert_eq!(payload(&first)?, payload(&second)?);
    // Round trip: parse and re-serialise is byte-identical.
    let parsed: ModelGolden = serde_json::from_str(&first)?;
    assert_eq!(to_pretty(&parsed)?, first);
    assert_eq!(parsed.result, tlc(NEW, "Liveness")?);
    // Key order is fixed (struct order): provenance comes after the payload.
    let pos = |key: &str| first.find(&format!("\n  \"{key}\":"));
    assert!(pos("schema") < pos("result"));
    assert!(pos("result") < pos("provenance"));
    assert!(first.ends_with("}\n"));
    Ok(())
}

#[test]
fn resource_exhaustion_is_not_an_oracle_fact() -> TestResult {
    // 94d0c50 at -Xmx64m: SYSTEM_OUT_OF_MEMORY_TOO_MANY_INIT (1002), exit 255.
    let r = tlc(NEW, "Oom")?;
    assert_eq!(r.verdict, Verdict::ResourceLimit);
    assert_eq!(
        r.error_class.as_deref(),
        Some("SYSTEM_OUT_OF_MEMORY_TOO_MANY_INIT")
    );
    // v1.7.4 never runs out of memory here: it refuses the set (>1000000
    // elements) as TLC_INITIAL_STATE (2102), and exit 75 is explained only
    // by the nested-expression context (2103): unclassified.
    let r = tlc(OLD, "Oom")?;
    assert_eq!((r.verdict, r.exit_code), (Verdict::Unclassified, Some(75)));
    // Deep recursion: GENERAL (1000) "StackOverflowError" then 2103, exit 75,
    // with -Xss pinned to 8m, in both versions.
    for oracle in [NEW, OLD] {
        let r = tlc(oracle, "Stack")?;
        assert_eq!(r.verdict, Verdict::ResourceLimit, "{oracle}");
        assert!(!r.verdict.is_oracle_fact());
    }
    Ok(())
}

#[test]
fn printed_markers_do_not_forge_messages() -> TestResult {
    // PrintT("@!@!@ENDMSG 2185 @!@!@") appears quoted, mid-line: it is text.
    for oracle in [NEW, OLD] {
        let stdout = read(oracle, "PrintMarker.stdout")?;
        let t = messages::parse(&stdout);
        assert_eq!(t.anomalies, Vec::<String>::new(), "{oracle}");
        assert!(t.untagged_text().contains("\"@!@!@ENDMSG 2185 @!@!@\""));
    }
    Ok(())
}

#[test]
fn unexpected_stderr_is_an_oracle_error() -> TestResult {
    let stdout = read(NEW, "Success.stdout")?;
    let r = classify(
        &messages::parse(&stdout),
        &RunFacts {
            exit_code: Some(0),
            stderr: Some("Exception in thread \"main\" java.lang.InternalError\n"),
            ..RunFacts::default()
        },
    );
    assert_eq!(r.verdict, Verdict::OracleError);
    assert!(r.note.is_some_and(|n| n.contains("InternalError")));
    // v1.7.4's resolver diagnostics on stderr are known and benign.
    assert_eq!(tlc(OLD, "MissingModule")?.verdict, Verdict::ParseError);
    Ok(())
}
