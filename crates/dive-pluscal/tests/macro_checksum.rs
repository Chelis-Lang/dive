//! Every retained primary row keeps its stage-specific disposition.
use dive_pluscal::{
    Code, FairnessOption, TranslationOptions, algorithm_checksum_at,
    rewrite::{RewriteOptions, select_rewrite},
};
use std::path::Path;

#[test]
fn exact_macro_ast_bytes_and_uncredited_reference_failures()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/macro-checksum");
    let mut outcomes = [0_usize; 4];
    for folder in [
        "",
        "boundary",
        "layout",
        "regression",
        "api-regression",
        "ownership",
        "insertion",
        "insertion-old",
        "closing-line",
        "inline",
        "marker",
        "marker-extra",
        "marker-single",
    ] {
        let base = root.join(folder);
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            base.join("primary/manifest.json"),
        )?)?;
        for row in manifest["rows"].as_array().ok_or("missing primary rows")? {
            let id = row["id"].as_str().ok_or("missing identifier")?;
            let source = std::fs::read_to_string(base.join("cases").join(format!("{id}.tla")))?;
            let flags = row["flags"].as_array().ok_or("missing flags")?;
            let has = |flag: &str| flags.iter().any(|actual| actual == flag);
            let options = TranslationOptions {
                insert_missing_labels: has("-label"),
                fairness: if has("-nof") {
                    FairnessOption::None
                } else if has("-wf") {
                    FairnessOption::Weak
                } else if has("-sf") {
                    FairnessOption::Strong
                } else if has("-wfNext") {
                    FairnessOption::Next
                } else {
                    FairnessOption::Declared
                },
                ..TranslationOptions::default()
            };
            let result =
                select_rewrite(&source, &RewriteOptions::default()).and_then(|selection| {
                    algorithm_checksum_at(
                        &source,
                        selection.algorithm.start..selection.algorithm.end,
                        &options,
                    )
                    .map_err(|error| error.diagnostic)
                });
            match result {
                Ok(actual) => {
                    assert_eq!(row["translated"], true, "{folder}/{id}");
                    assert_eq!(row["reference_errors"], false, "{folder}/{id}");
                    assert_eq!(
                        actual.serialized_ast,
                        std::fs::read_to_string(
                            base.join("primary").join(format!("{id}.translated.ast"))
                        )?,
                        "{folder}/{id}"
                    );
                    assert_eq!(
                        actual.checksum, row["stages"]["translated"]["crc"],
                        "{folder}/{id}"
                    );
                    assert_eq!(actual.fair_header_crc_only, row["header"][0] == "7c28162a");
                    let index = usize::from(row["sany"]["accepted"] != true);
                    outcomes[index] += 1;
                }
                Err(error) => {
                    assert!(source.is_char_boundary(error.span.start));
                    assert!(source.is_char_boundary(error.span.end));
                    assert!(error.span.start <= error.span.end && error.span.end <= source.len());
                    if error.code == Code::Unsupported {
                        assert_ne!(error.message, "");
                        outcomes[3] += 1;
                    } else {
                        assert!(
                            row["translated"] != true || row["reference_errors"] == true,
                            "{folder}/{id}: {error}"
                        );
                        assert!(
                            matches!(
                                error.code,
                                Code::Syntax | Code::Lexical | Code::Expression | Code::Label
                            ),
                            "{folder}/{id}: {error}"
                        );
                        outcomes[2] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(outcomes, [237, 24, 66, 21]);
    Ok(())
}

#[test]
fn macro_checksum_never_returns_partial_output_under_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; macro M(a) begin x:=a+1; end macro; begin L:M(1); end algorithm;";
    let exact = algorithm_checksum_at(source, 0..source.len(), &TranslationOptions::default())?;
    for max_output_bytes in [0, 1, exact.serialized_ast.len() - 1] {
        let options = TranslationOptions {
            max_output_bytes,
            ..TranslationOptions::default()
        };
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|error| error.diagnostic.code == Code::Resource)
        );
    }
    for budget in 0..4 {
        let mut options = TranslationOptions::default();
        match budget {
            0 => options.parser.max_bytes = 1,
            1 => options.parser.max_tokens = 1,
            2 => options.parser.max_nodes = 1,
            _ => options.max_actions = 0,
        }
        assert!(
            algorithm_checksum_at(source, 0..source.len(), &options)
                .is_err_and(|error| error.diagnostic.code == Code::Resource)
        );
    }
    Ok(())
}

#[test]
fn decorated_embedded_comments_keep_original_bytes_and_standalone_eof()
-> Result<(), Box<dyn std::error::Error>> {
    let algorithm = "--algorithm A variable x=0; begin L:x:=1; end algorithm;";
    for (open, close) in [("(*", "*)"), ("(**", "**)"), ("(*", "****)")] {
        let source = format!("---- MODULE R ----\n{open} {algorithm} {close}\n====\n");
        let before = source.clone();
        let selected = select_rewrite(&source, &RewriteOptions::default())?;
        let checksum = algorithm_checksum_at(
            &source,
            selected.algorithm.start..selected.algorithm.end,
            &TranslationOptions::default(),
        )?;
        assert_ne!(checksum.serialized_ast, "");
        assert_eq!(source, before);
        assert!(source[selected.comment.start..selected.comment.end].ends_with(close));
    }
    assert!(
        dive_pluscal::parse(&format!("{algorithm} *"), &dive_pluscal::Options::default()).is_err()
    );
    assert!(
        dive_pluscal::parse(
            &format!("{algorithm} notes"),
            &dive_pluscal::Options::default()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn owned_region_failure_and_global_token_budget_never_guess_end()
-> Result<(), Box<dyn std::error::Error>> {
    use dive_pluscal::{Options, ast::Span, rewrite::owned_algorithm_comment_region};
    let source = "(* --algorithm A begin L:skip; end algorithm; notes *)";
    let comment = Span {
        start: 0,
        end: source.len(),
    };
    let limits = Options::default();
    let owned = owned_algorithm_comment_region(source, comment, &limits)?;
    assert_eq!(
        &source[owned.start..owned.end],
        "--algorithm A begin L:skip; end algorithm;"
    );
    for max_tokens in [0, 1, 9] {
        assert!(
            owned_algorithm_comment_region(
                source,
                comment,
                &Options {
                    max_tokens,
                    ..Options::default()
                }
            )
            .is_err_and(|error| error.code == Code::Resource)
        );
    }
    for (text, invalid) in [
        (
            source,
            Span {
                start: usize::MAX,
                end: usize::MAX,
            },
        ),
        (source, Span { start: 7, end: 2 }),
        ("λ(* missing marker *)", Span { start: 1, end: 3 }),
        ("λ(* missing marker *)", Span { start: 0, end: 1 }),
    ] {
        let error = owned_algorithm_comment_region(text, invalid, &limits)
            .err()
            .ok_or("invalid caller range unexpectedly accepted")?;
        assert_eq!(error.code, Code::Syntax);
        assert!(error.span.start <= error.span.end && error.span.end <= text.len());
        assert!(text.is_char_boundary(error.span.start));
        assert!(text.is_char_boundary(error.span.end));
    }
    let missing = "(* --algorithm A begin L:skip; notes *)";
    assert!(
        owned_algorithm_comment_region(
            missing,
            Span {
                start: 0,
                end: missing.len()
            },
            &limits
        )
        .is_err()
    );
    let quote = "(* --algorithm A begin L:skip; end algorithm; \"open *)";
    assert!(
        owned_algorithm_comment_region(
            quote,
            Span {
                start: 0,
                end: quote.len()
            },
            &limits
        )
        .is_err_and(|error| error.code == Code::Lexical)
    );
    Ok(())
}
