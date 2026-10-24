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
    assert!(dive_api::diagnostics::explain("DIVE-E0100").is_some());
    assert!(dive_api::diagnostics::explain("DIVE-E0102").is_none());
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

#[test]
fn unresolved_names_keep_utf8_and_qualified_source_evidence() -> Result {
    for (source, spelling, repaired) in [
        (
            "---- MODULE MissingName ----\n\\* π precedes the diagnostic byte range\nA == missing\n====\n",
            "missing",
            "---- MODULE MissingName ----\n\\* π precedes the diagnostic byte range\nmissing == 1\nA == missing\n====\n",
        ),
        (
            "---- MODULE MissingName ----\n\\* π precedes the diagnostic byte range\n---- MODULE Inner ----\nz == 1\n====\nI == INSTANCE Inner\nA == I!missing\n====\n",
            "I!missing",
            "---- MODULE MissingName ----\n\\* π precedes the diagnostic byte range\n---- MODULE Inner ----\nz == 1\n====\nI == INSTANCE Inner\nA == I!z\n====\n",
        ),
    ] {
        let report =
            dive_api::analyze_source("MissingName.tla", source, &AnalyzeOptions::default());
        assert!(!report.accepted());
        assert!(
            report.analysis().syntax_diagnostics().is_empty(),
            "{}",
            report.render_human()
        );
        let raw = report
            .analysis()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Code::UndefinedSymbol)
            .ok_or("unresolved name diagnostic")?;
        assert_eq!(raw.sany, Some(4200));
        assert_eq!(raw.severity, Severity::Error);
        let span = raw.span.ok_or("actual unresolved name span")?;
        let start = usize::from(span.range.start());
        let end = usize::from(span.range.end());
        assert_eq!(&source[start..end], spelling);
        assert!(source[..start].chars().count() < start);
        assert_eq!(
            raw.help,
            Some(format!(
                "define `{spelling}`, declare it, or extend the module that defines it"
            ))
        );
        let document = report.to_json();
        let row = document["diagnostics"]
            .as_array()
            .ok_or("diagnostic rows")?
            .iter()
            .find(|row| row["code"] == "DIVE-E0100")
            .ok_or("name JSON row")?;
        let line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        assert_eq!(row["span"]["line"], line);
        assert_eq!(row["span"]["column"], 6);
        assert!(
            report
                .render_human()
                .contains(&format!("MissingName.tla:{line}:6:"))
        );
        check_projection(&report)?;
        let repaired =
            dive_api::analyze_source("MissingName.tla", repaired, &AnalyzeOptions::default());
        assert!(repaired.accepted(), "{}", repaired.render_human());
        assert_eq!(repaired.analysis().diagnostics(), []);
    }
    Ok(())
}

#[test]
fn unresolved_standard_references_keep_specific_import_suggestions() -> Result {
    for (body, code, help, module) in [
        (
            "Len(<<1>>)",
            Code::UndefinedSymbol,
            "`Len` is defined in the standard module Sequences; add it to EXTENDS",
            "Sequences",
        ),
        (
            "1 + 2",
            Code::UndefinedOperatorSymbol,
            "add `EXTENDS Naturals` (or `Integers`, `Reals`)",
            "Naturals",
        ),
        (
            "-1",
            Code::UndefinedOperatorSymbol,
            "add `EXTENDS Integers` (or `Reals`) for unary minus",
            "Integers",
        ),
    ] {
        let source = format!("---- MODULE MissingImport ----\nA == {body}\n====\n");
        let report =
            dive_api::analyze_source("MissingImport.tla", &source, &AnalyzeOptions::default());
        assert!(!report.accepted());
        let raw = report
            .analysis()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .ok_or("import diagnostic")?;
        assert_eq!(raw.help.as_deref(), Some(help));
        let explanation =
            dive_api::diagnostics::explain(code.as_str()).ok_or("reference explanation")?;
        assert_ne!(explanation.remediation, help);
        check_projection(&report)?;
        let source = source.replace("A ==", &format!("EXTENDS {module}\nA =="));
        let repaired =
            dive_api::analyze_source("MissingImport.tla", &source, &AnalyzeOptions::default());
        assert!(repaired.accepted(), "{}", repaired.render_human());
        assert_eq!(repaired.analysis().diagnostics(), []);
    }
    Ok(())
}

#[test]
fn bare_operator_arguments_proofs_and_substitutions_receive_fallback_help() -> Result {
    for (body, spelling) in [
        ("F(G(_,_)) == G(1, 2)\nA == F(+)\n", "+"),
        (
            "THEOREM 1 = 1\n<1>1. 1 = 1\n  OBVIOUS\n<1>2. @ + 1\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
            "@ + 1",
        ),
        (
            "---- MODULE Inner ----\nCONSTANT Op(_,_)\nz == Op(1, 2)\n====\nI == INSTANCE Inner WITH Op <- +\n",
            "+",
        ),
    ] {
        let source = format!("---- MODULE MissingOperator ----\n{body}====\n");
        let report =
            dive_api::analyze_source("MissingOperator.tla", &source, &AnalyzeOptions::default());
        assert!(!report.accepted());
        assert!(
            report.analysis().syntax_diagnostics().is_empty(),
            "{}",
            report.render_human()
        );
        let raw = report
            .analysis()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Code::UndefinedOperatorSymbol)
            .ok_or("bare operator diagnostic")?;
        assert_eq!(raw.help, None);
        assert_eq!(raw.sany, Some(4004));
        let span = raw.span.ok_or("actual operator span")?;
        assert_eq!(
            &source[usize::from(span.range.start())..usize::from(span.range.end())],
            spelling
        );
        let explanation =
            dive_api::diagnostics::explain("DIVE-E0101").ok_or("operator explanation")?;
        let document = report.to_json();
        let row = document["diagnostics"]
            .as_array()
            .ok_or("diagnostic rows")?
            .iter()
            .find(|row| row["code"] == "DIVE-E0101")
            .ok_or("operator JSON row")?;
        assert_eq!(row["help"], explanation.remediation);
        assert!(
            report
                .render_human()
                .contains(&format!("  help: {}\n", explanation.remediation))
        );
        check_projection(&report)?;
        let repaired = format!("---- MODULE MissingOperator ----\nEXTENDS Naturals\n{body}====\n");
        let repaired =
            dive_api::analyze_source("MissingOperator.tla", &repaired, &AnalyzeOptions::default());
        assert!(repaired.accepted(), "{}", repaired.render_human());
        assert_eq!(repaired.analysis().diagnostics(), []);
    }
    Ok(())
}

#[test]
fn reference_explanations_are_informational_and_neighbor_codes_stay_unknown() -> Result {
    for code in ["DIVE-E0100", "DIVE-E0101"] {
        let report = dive_api::diagnostics::explanation_json(code);
        assert_eq!(report["schema_version"], 3);
        assert_eq!(report["command"], "explain");
        assert_eq!(report["code"], code);
        assert_eq!(report["known"], true);
        let explanation = dive_api::diagnostics::explain(code).ok_or("reference explanation")?;
        assert_eq!(report["explanation"]["cause"], explanation.cause);
        assert_eq!(
            report["explanation"]["remediation"],
            explanation.remediation
        );
        assert!(report.get("verdict").is_none());
    }
    assert!(dive_api::diagnostics::explain(Code::Redefinition.as_str()).is_none());
    assert_eq!(
        dive_api::diagnostics::explanation_json("DIVE-E0102")["known"],
        false
    );
    Ok(())
}
