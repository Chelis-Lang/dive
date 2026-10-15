//! Reached bounds for the separately measured flat procedural header positive.
use dive_pluscal::{Code, TranslationOptions, algorithm_checksum_at, original_ast_at};
const FLAT: &str = "--algorithm A variables x=0; procedure P(a=1) begin P1:skip; R:return; end procedure; begin L:call P(2); E:skip; end algorithm";

#[test]
fn procedural_original_artifacts_and_generation_do_not_require_exact_crc()
-> Result<(), Box<dyn std::error::Error>> {
    let options = TranslationOptions {
        debug: true,
        ..TranslationOptions::default()
    };
    // Different source names/labels and source-finite self calls reach the same
    // copied-entry rule; no new primary/module/execution credit is inferred.
    for source in [
        FLAT.to_owned(),
        FLAT.replace("P(a=1)", "Q(a=1)")
            .replace("P1:", "Entry:")
            .replace("call P(2)", "call Q(2)"),
        FLAT.replace("P1:skip;", "P1:call P(a); return;"),
        FLAT.replace("x=0;", "x=0;\n"),
    ] {
        let original = original_ast_at(&source, 0..source.len(), &options)?;
        assert_ne!(original.serialized_ast, "");
        assert!(
            algorithm_checksum_at(&source, 0..source.len(), &options).is_err_and(|error| error
                .diagnostic
                .message
                .contains("procedure AST checksum is unavailable"))
        );
        assert!(dive_pluscal::translate(&source, &options).is_ok());
    }
    Ok(())
}

#[test]
fn optional_crc_failures_preserve_real_generation_diagnostics()
-> Result<(), Box<dyn std::error::Error>> {
    let options = TranslationOptions {
        debug: true,
        ..TranslationOptions::default()
    };
    for (source, label, feature) in [
        (
            "--algorithm A variables x=0,y=0; begin L:x:=1 || y:=2; end algorithm".to_owned(),
            false,
            "simultaneous-assignment",
        ),
        (
            FLAT.replace("P1:skip;", "P1:if TRUE then skip; end if;"),
            false,
            "compound procedural",
        ),
        (
            FLAT.replace(
                "begin L:call P(2); E:skip; end algorithm",
                "process W=1 begin L:call P(2); E:skip; end process; end algorithm",
            ),
            false,
            "process-owned procedure",
        ),
        (
            FLAT.replace(
                "procedure P",
                "macro M() begin skip; end macro; procedure P",
            ),
            false,
            "procedural macro",
        ),
        (
            FLAT.replace("procedure P", "define H==1 end define; procedure P"),
            false,
            "procedure DEFINE",
        ),
        (FLAT.replace("P1:", ""), true, "automatic procedure label"),
        (FLAT.replace("P1:", "P1:-"), false, "fairness-label"),
        (
            FLAT.replace("x=0;", "x=0,y=0;")
                .replace("P1:skip;", "P1:x:=1 || y:=2;"),
            false,
            "simultaneous-assignment",
        ),
        (
            FLAT.replace("call P(2)", "call Missing(2)"),
            false,
            "unresolved procedure",
        ),
        (
            FLAT.replace("call P(2)", "call P()"),
            false,
            "argument-count",
        ),
        (
            FLAT.replace("E:skip;", "E:return;"),
            false,
            "return outside",
        ),
        (FLAT.replace("P1:", "x:"), false, "identifier renaming"),
    ] {
        let options = TranslationOptions {
            insert_missing_labels: label,
            ..options.clone()
        };
        assert!(
            original_ast_at(&source, 0..source.len(), &options).is_ok(),
            "original availability: {feature}"
        );
        let failure = algorithm_checksum_at(&source, 0..source.len(), &options)
            .err()
            .ok_or(feature)?;
        assert_eq!(
            failure.diagnostic.code,
            Code::Unsupported,
            "{feature}: {failure}"
        );
        assert!(
            failure.diagnostic.message.contains(feature)
                || failure
                    .diagnostic
                    .message
                    .contains("procedure AST checksum is unavailable"),
            "{feature}: {failure}"
        );
        let generated = dive_pluscal::translate(&source, &options);
        if ["unresolved procedure", "argument-count", "return outside"].contains(&feature) {
            assert!(
                generated.is_err(),
                "{feature}: preserve real generation failure"
            );
        } else {
            assert!(
                generated.is_ok(),
                "{feature}: optional CRC must not block generation"
            );
        }
    }
    Ok(())
}

#[test]
fn existing_shared_frame_controls_keep_exact_body_lines() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/procedure");
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("fixtures/manifest.json"),
    )?)?;
    for id in [
        "Procedure_C_simple",
        "Procedure_P_simple",
        "Procedure_P_two_params",
        "Procedure_P_return_same_label",
    ] {
        let row = manifest["rows"]
            .as_array()
            .ok_or("rows")?
            .iter()
            .find(|row| row["id"] == id)
            .ok_or("control")?;
        assert_eq!(row["primary"]["status"], "translated");
        assert_eq!(row["sany"]["status"], "accepted");
        assert_eq!(
            row["options"].as_array().ok_or("flags")?.as_slice(),
            &[] as &[serde_json::Value]
        );
        let source = std::fs::read_to_string(
            root.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        let reference = std::fs::read_to_string(
            root.join("fixtures")
                .join(row["translated"]["path"].as_str().ok_or("output")?),
        )?;
        let body = reference
            .lines()
            .skip_while(|line| !line.contains("BEGIN TRANSLATION"))
            .skip(1)
            .take_while(|line| !line.contains("END TRANSLATION"))
            .collect::<Vec<_>>()
            .join("\n");
        let selected = dive_pluscal::rewrite::select_rewrite(
            &source,
            &dive_pluscal::rewrite::RewriteOptions::default(),
        )?;
        let native = dive_pluscal::translate_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &TranslationOptions::default(),
        )?;
        assert_eq!(
            native.tla.trim_end_matches('\n'),
            body.trim_end_matches('\n'),
            "{id}"
        );
        assert_eq!(
            dive_pluscal::rewrite::translation_checksum(&native.tla),
            dive_pluscal::rewrite::translation_checksum(&body),
            "{id}"
        );
    }
    Ok(())
}
