//! Exact post-translation expression/conditional/binder serialization controls.
use dive_pluscal::{
    Code, Options, TranslationOptions, algorithm_checksum_at, parse,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;

#[test]
fn complete_nesting_layout_selection_matches_or_refuses() -> Result<(), Box<dyn std::error::Error>>
{
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("checksum-expression-acceptance");
    let mut total = 0;
    let mut exact = 0;
    let mut refused = 0;
    let mut rejected = 0;
    for folder in [
        "primary",
        "c-fixed-controls",
        "layout-controls",
        "separator-controls",
        "empty-else-controls",
        "terminal-empty-controls",
    ] {
        let directory = base.join(folder);
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(directory.join("manifest.json"))?)?;
        for row in manifest["rows"].as_array().ok_or("missing rows")? {
            total += 1;
            let id = row["id"].as_str().ok_or("missing id")?;
            let source = std::fs::read_to_string(directory.join(format!("{id}.tla")))?;
            let selection = select_rewrite(&source, &RewriteOptions::default())?;
            match algorithm_checksum_at(
                &source,
                selection.algorithm.start..selection.algorithm.end,
                &TranslationOptions::default(),
            ) {
                Ok(checksum) => {
                    assert_eq!(row["primary_translated"], true, "{id}");
                    assert_eq!(
                        checksum.serialized_ast,
                        std::fs::read_to_string(directory.join(format!("{id}.translated.ast")))?,
                        "{id}"
                    );
                    assert_eq!(checksum.checksum, row["primary_crc"], "{id}");
                    exact += 1;
                }
                Err(error) if error.diagnostic.code == Code::Unsupported => refused += 1,
                Err(error) => {
                    assert_eq!(row["primary_translated"], false, "{id}: {error}");
                    assert!(matches!(
                        error.diagnostic.code,
                        Code::Syntax | Code::Lexical | Code::Expression | Code::Label
                    ));
                    rejected += 1;
                }
            }
        }
    }
    assert_eq!(total, 320);
    assert_eq!((exact, refused, rejected), (224, 14, 82));
    Ok(())
}

#[test]
fn trailing_with_separator_keeps_empty_binder_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for binding in ["a=1,", "a=1;"] {
        let source =
            format!("--algorithm A {{variable x=0; {{L:with({binding}){{x:=a;}}; E:skip;}}}}");
        parse(&source, &Options::default())?;
    }
    for binding in ["", ",", ";", ",a=1", "a=1,,"] {
        let source =
            format!("--algorithm A {{variable x=0; {{L:with({binding}){{skip;}}; E:skip;}}}}");
        assert!(parse(&source, &Options::default()).is_err(), "{binding}");
    }
    Ok(())
}

#[test]
fn expression_and_nested_output_bounds_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; begin L:with a=1,b=2 do if x=0 then x:=a; elsif x=1 then x:=b; end if; end with; E:skip; end algorithm;";
    let checksum = algorithm_checksum_at(source, 0..source.len(), &TranslationOptions::default())?;
    for max_output_bytes in [0, 1, checksum.serialized_ast.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..Default::default()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|error| error.diagnostic.code == Code::Resource)
        );
    }
    let bare_cr = source.replace(" then ", " then\r");
    assert!(
        algorithm_checksum_at(&bare_cr, 0..bare_cr.len(), &TranslationOptions::default())
            .is_err_and(|error| error.diagnostic.code == Code::Unsupported)
    );
    Ok(())
}

#[test]
fn empty_else_retains_true_before_branch_identity() -> Result<(), Box<dyn std::error::Error>> {
    for otherwise in ["", "else", "else skip;"] {
        let source = format!(
            "--algorithm A variable x=0; begin L:if x=0 then x:=1; {otherwise} end if; E:skip; end algorithm;"
        );
        let artifact = dive_pluscal::translate(&source, &TranslationOptions::default())?;
        assert!(artifact.tla.contains("ELSE /\\ TRUE"), "{otherwise}");
        assert!(artifact.tla.contains("x' = x"), "{otherwise}");
    }
    let source =
        "--algorithm A variable x=0; begin L:if x=0 then else x:=1; end if; E:skip; end algorithm;";
    parse(source, &Options::default())?;
    assert!(
        dive_pluscal::translate(source, &TranslationOptions::default())
            .is_err_and(|error| error.diagnostic.code == Code::Unsupported)
    );
    for source in [
        "--algorithm A variable x=0; begin L:if x=0 then else end if; E:skip; end algorithm;",
        "--algorithm A variable x=0; begin L:if x=0 then x:=1; elsif x=1 then else end if; E:skip; end algorithm;",
        "--algorithm A {variable x=0; {L:if(x=0){x:=1;}else{}; E:skip;}}",
    ] {
        assert!(parse(source, &Options::default()).is_err());
    }
    Ok(())
}
