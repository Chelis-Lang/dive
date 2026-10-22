//! Report-only semantic help retains the original pinned diagnostic evidence.

use std::path::PathBuf;

use dive_api::{AnalyzeOptions, AnalyzeReport};
use dive_sema::{Code, Severity};
use serde_json::json;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn cases(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sema/tests")
        .join(path)
}

fn check_projection(report: &AnalyzeReport) -> Result {
    let producer = report.analysis().diagnostics().to_vec();
    let model_before = format!("{:?}", report.analysis().model());
    let document = report.to_json();
    let human = report.render_human();
    let rows = document["diagnostics"]
        .as_array()
        .ok_or("diagnostics array")?;
    let offset = report.analysis().syntax_diagnostics().len();
    assert_eq!(rows.len(), offset + producer.len());
    assert_eq!(document["accepted"], report.analysis().accepted());
    for (raw, row) in producer.iter().zip(&rows[offset..]) {
        assert_eq!(row["code"], raw.code.as_str());
        assert_eq!(row["message"], raw.message);
        assert_eq!(
            row["severity"],
            match raw.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            }
        );
        assert_eq!(row["sany_code"], json!(raw.sany));
        assert_eq!(row["file"], json!(raw.span.map(|span| span.file.index())));
        if let Some(span) = raw.span {
            assert_eq!(row["span"]["start"], u32::from(span.range.start()));
            assert_eq!(row["span"]["end"], u32::from(span.range.end()));
        } else {
            assert!(row["span"].is_null());
        }
        let related = row["related"].as_array().ok_or("related array")?;
        assert_eq!(related.len(), raw.related.len());
        for (original, shown) in raw.related.iter().zip(related) {
            assert_eq!(shown["file"], original.span.file.index());
            assert_eq!(
                shown["span"]["start"],
                u32::from(original.span.range.start())
            );
            assert_eq!(shown["span"]["end"], u32::from(original.span.range.end()));
            assert_eq!(shown["message"], original.message);
        }
        let expected = raw.help.as_deref().or_else(|| {
            dive_api::diagnostics::explain(raw.code.as_str()).map(|entry| entry.remediation)
        });
        assert_eq!(row["help"], json!(expected));
        if let Some(help) = expected {
            assert!(human.contains(&format!("  help: {help}\n")));
        }
    }
    assert_eq!(report.analysis().diagnostics(), producer);
    assert_eq!(format!("{:?}", report.analysis().model()), model_before);
    Ok(())
}

const ARGUMENT_CASES: &[(&str, Code, u16)] = &[
    (
        "error-corpus/E4204_2_Required_1_Provided_Test.tla",
        Code::WrongArgumentCount,
        4204,
    ),
    (
        "error-corpus/E4204_0_Required_1_Provided_Test.tla",
        Code::WrongArgumentCount,
        4204,
    ),
    (
        "error-corpus/E4204_Instance_Test.tla",
        Code::WrongArgumentCount,
        4204,
    ),
    (
        "sany-cases/FunctionArity.tla",
        Code::FunctionArgumentCount,
        4260,
    ),
    (
        "sany-cases/FunctionTupleArity.tla",
        Code::FunctionArgumentCount,
        4260,
    ),
    (
        "error-corpus/E4260_1_Expected_2_Provided_Test.tla",
        Code::FunctionArgumentCount,
        4260,
    ),
    (
        "error-corpus/E4260_2_Expected_3_Provided_Test.tla",
        Code::FunctionArgumentCount,
        4260,
    ),
    (
        "sany-cases/ExprToOpParam.tla",
        Code::OperatorArgumentExpected,
        4270,
    ),
    (
        "error-corpus/E4270_Test.tla",
        Code::OperatorArgumentExpected,
        4270,
    ),
    (
        "sany-cases/HigherOrderBad.tla",
        Code::OperatorArgumentArity,
        4271,
    ),
    (
        "sany-cases/OpArgBuiltinArity.tla",
        Code::OperatorArgumentArity,
        4271,
    ),
    (
        "error-corpus/E4271_2_Required_1_Provided_Test.tla",
        Code::OperatorArgumentArity,
        4271,
    ),
    (
        "error-corpus/E4271_1_Required_2_Provided_Test.tla",
        Code::OperatorArgumentArity,
        4271,
    ),
    (
        "sany-cases/LambdaArity.tla",
        Code::LambdaArgumentArity,
        4274,
    ),
    (
        "error-corpus/E4274_1_Required_2_Provided_Test.tla",
        Code::LambdaArgumentArity,
        4274,
    ),
    (
        "error-corpus/E4274_2_Required_1_Provided_Test.tla",
        Code::LambdaArgumentArity,
        4274,
    ),
    (
        "error-corpus/E4274_2_Required_3_Provided_Test.tla",
        Code::LambdaArgumentArity,
        4274,
    ),
];

#[test]
fn existing_sany_argument_cases_keep_their_exact_diagnostic_evidence() -> Result {
    for &(file, code, sany) in ARGUMENT_CASES {
        let report = dive_api::analyze_file(&cases(file), &AnalyzeOptions::default())?;
        assert!(!report.accepted(), "{file}");
        let diagnostic = report
            .analysis()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .ok_or_else(|| format!("missing {} in {file}", code.as_str()))?;
        assert_eq!(diagnostic.sany, Some(sany));
        assert!(diagnostic.span.is_some());
        check_projection(&report)?;
    }
    Ok(())
}

#[test]
fn higher_order_positive_controls_and_unlisted_errors_are_unchanged() -> Result {
    for file in ["HigherOrderOk.tla", "OpArgBuiltin.tla"] {
        let report = dive_api::analyze_file(
            &cases(&format!("sany-cases/{file}")),
            &AnalyzeOptions::default(),
        )?;
        assert!(report.accepted(), "{file}");
        assert_eq!(report.analysis().diagnostics(), []);
        check_projection(&report)?;
    }
    let report = dive_api::analyze_source(
        "Unknown.tla",
        "---- MODULE Unknown ----\nA == missing\n====\n",
        &AnalyzeOptions::default(),
    );
    assert!(!report.accepted());
    assert_eq!(report.to_json()["diagnostics"][0]["code"], "DIVE-E0100");
    assert_eq!(
        report.to_json()["diagnostics"][0]["help"],
        "define `missing`, declare it, or extend the module that defines it"
    );
    assert!(dive_api::diagnostics::explain("DIVE-E0100").is_none());
    check_projection(&report)
}

#[test]
fn unrelated_related_locations_and_accepted_warnings_stay_in_order() -> Result {
    for (file, accepted, severity) in [
        ("DupDefinition.tla", false, Severity::Error),
        ("DupConstant.tla", true, Severity::Warning),
    ] {
        let report = dive_api::analyze_file(
            &cases(&format!("sany-cases/{file}")),
            &AnalyzeOptions::default(),
        )?;
        assert_eq!(report.accepted(), accepted, "{file}");
        let diagnostic = report
            .analysis()
            .diagnostics()
            .first()
            .ok_or("diagnostic")?;
        assert_eq!(diagnostic.severity, severity);
        assert!(!diagnostic.related.is_empty(), "{file}");
        assert!(dive_api::diagnostics::explain(diagnostic.code.as_str()).is_none());
        check_projection(&report)?;
    }
    Ok(())
}

#[test]
fn unicode_offsets_and_specific_producer_help_survive_projection() -> Result {
    let source = "---- MODULE UnicodeCall ----\n\\* π precedes the diagnostic\nF(a) == a\nP == F(0, 0)\n====\n";
    let report = dive_api::analyze_source("UnicodeCall.tla", source, &AnalyzeOptions::default());
    assert!(!report.accepted());
    let diagnostic = report
        .analysis()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == Code::WrongArgumentCount)
        .ok_or("call diagnostic")?;
    let span = diagnostic.span.ok_or("actual call span")?;
    let start = usize::from(span.range.start());
    assert!(source[..start].chars().count() < start);
    assert!(source.get(start..usize::from(span.range.end())).is_some());
    assert_eq!(report.to_json()["diagnostics"][0]["span"]["line"], 4);
    check_projection(&report)?;
    let report = dive_api::analyze_file(
        &cases("sany-cases/ExprToOpParam.tla"),
        &AnalyzeOptions::default(),
    )?;
    let original = report
        .analysis()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == Code::OperatorArgumentExpected)
        .ok_or("operator argument diagnostic")?;
    assert_eq!(
        original.help.as_deref(),
        Some("pass an operator name or a LAMBDA")
    );
    check_projection(&report)
}
