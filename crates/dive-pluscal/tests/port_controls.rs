//! Pinned semantic controls for former translation-admission limitations.
use dive_pluscal::{Code, TranslationOptions, translate, translate_at};
use std::{fmt::Write, path::Path};

fn words(source: &str) -> Vec<String> {
    dive_syntax::tokens::tokenize_spec(source)
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .map(|token| {
            source[usize::from(token.range.start())..usize::from(token.range.end())].into()
        })
        .collect()
}

#[test]
fn former_limits_follow_the_pinned_translator() -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/port-controls");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let rows = manifest["rows"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), 21);
    for row in rows {
        let id = row["id"].as_str().ok_or("id")?;
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        let start = source.find("--algorithm").ok_or("algorithm")?;
        let end = source.rfind("*)").ok_or("comment")?;
        let artifact = translate_at(&source, start..end, &TranslationOptions::default())?;
        let primary = std::fs::read_to_string(
            base.join("fixtures")
                .join(row["translated"]["path"].as_str().ok_or("translation")?),
        )?;
        let body = primary
            .lines()
            .skip_while(|line| !line.contains("BEGIN TRANSLATION"))
            .skip(1)
            .take_while(|line| !line.contains("END TRANSLATION"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(words(&artifact.tla), words(&body), "{id}");
    }
    Ok(())
}

#[test]
fn native_mappings_and_macro_locations_retain_original_source()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; macro M() begin x:=1; x:=2; end macro; begin M(); end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert_eq!(artifact.mappings.len(), artifact.actions.len());
    assert!(artifact.mappings.iter().all(
        |map| map.original.start < map.original.end && map.generated.start < map.generated.end
    ));
    let quoted = translate(
        "--algorithm A variable x=\"L ==\"; begin L:skip; end algorithm",
        &TranslationOptions::default(),
    )?;
    let mapping = quoted.mappings.first().ok_or("L mapping")?;
    assert!(quoted.tla[mapping.generated.start..mapping.generated.end].starts_with("L =="));
    assert!(quoted.tla[..mapping.generated.start].contains("x = \"L ==\""));
    assert_eq!(artifact.inserted_labels.len(), 2);
    for label in &artifact.inserted_labels {
        assert!(source[label.source.start..label.source.end].contains("M()"));
        let definition = label.macro_definition.ok_or("macro definition")?;
        assert!(source[definition.start..definition.end].contains("x:="));
        assert!(label.macro_definition_position.is_some());
    }
    Ok(())
}

#[test]
fn macro_budget_is_shared_across_definitions_and_call_sites()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0; macro M(a) begin x:=<<a,a>>; end macro; macro N(a) begin M(<<a,a>>); end macro; begin L:N(<<1,2>>); E:N(<<3,4>>); end algorithm";
    let options = TranslationOptions {
        max_output_bytes: 40,
        ..TranslationOptions::default()
    };
    let error = translate(source, &options)
        .err()
        .ok_or("macro expansion must stop before allocating amplified output")?;
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("before token allocation"));
    assert!(
        source
            .get(error.diagnostic.span.start..error.diagnostic.span.end)
            .is_some()
    );
    Ok(())
}

#[test]
fn expanded_macro_depth_respects_the_selected_boundary() -> Result<(), Box<dyn std::error::Error>> {
    fn source(depth: usize) -> String {
        let mut result =
            "--algorithm A variable x=0; macro M0() begin x:=1; end macro; ".to_owned();
        for i in 1..=depth {
            let _ = write!(
                result,
                "macro M{i}() begin with a{i}=1 do M{}(); end with; end macro; ",
                i - 1
            );
        }
        let _ = write!(result, "begin L:M{depth}(); end algorithm");
        result
    }
    for (depth, limit) in [(15, 8), (140, 128)] {
        let mut options = TranslationOptions::default();
        options.parser.max_depth = limit;
        let input = source(depth);
        let error = translate(&input, &options)
            .err()
            .ok_or("expanded depth must be bounded")?;
        assert_eq!(error.diagnostic.code, Code::Resource);
        assert_eq!(
            error.diagnostic.message,
            "PlusCal normalized control-flow depth limit reached"
        );
        assert!(
            input
                .get(error.diagnostic.span.start..error.diagnostic.span.end)
                .is_some()
        );
    }
    let mut options = TranslationOptions::default();
    options.parser.max_depth = 8;
    assert!(translate(&source(7), &options).is_ok());
    Ok(())
}

#[test]
fn generated_output_is_bounded_during_construction() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = "--algorithm A variables ".to_owned();
    for i in 0..500 {
        if i != 0 {
            source.push(',');
        }
        write!(source, "long_variable_name_{i:04}=0")?;
    }
    source.push_str("; begin ");
    for i in 0..1000 {
        write!(source, "L{i}:skip; ")?;
    }
    source.push_str("end algorithm");
    for max_output_bytes in [1024, 4_194_304] {
        let options = TranslationOptions {
            max_output_bytes,
            ..TranslationOptions::default()
        };
        let error = translate(&source, &options)
            .err()
            .ok_or("amplified UNCHANGED output must stop")?;
        assert_eq!(error.diagnostic.code, Code::Resource);
        assert_eq!(error.stage, dive_pluscal::TranslationStage::Generation);
        assert_eq!(
            error.diagnostic.message,
            "PlusCal generated-output byte limit reached"
        );
        assert!(error.diagnostic.span.end > error.diagnostic.span.start);
    }
    Ok(())
}

#[test]
fn generated_byte_budget_admits_its_exact_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multiline");
    for relative in [
        "primary/C_junction.tla",
        "primary/P_empty-then.tla",
        "conditional-controls/C_if-else-lines.tla",
        "conditional-controls/P_fair_if-else-lines.tla",
        "primary/C_unicode-junction.tla",
        "supplemental/P_process-lines.tla",
    ] {
        let module = std::fs::read_to_string(base.join(relative))?;
        let start = module
            .find("--fair")
            .or_else(|| module.find("--algorithm"))
            .ok_or("algorithm")?;
        let end = module.rfind("*)").ok_or("algorithm comment")?;
        let source = &module[start..end];
        let artifact = translate(source, &TranslationOptions::default())?;
        let options = TranslationOptions {
            max_output_bytes: artifact.tla.len(),
            ..TranslationOptions::default()
        };
        assert_eq!(translate(source, &options)?.tla, artifact.tla, "{relative}");
        let options = TranslationOptions {
            max_output_bytes: artifact.tla.len() - 1,
            ..options
        };
        assert_eq!(
            translate(source, &options)
                .err()
                .ok_or("one byte too small")?
                .diagnostic
                .code,
            Code::Resource,
            "{relative}"
        );
    }
    Ok(())
}

#[test]
fn macro_layout_amplification_is_charged_before_substitution()
-> Result<(), Box<dyn std::error::Error>> {
    let actual = format!("1 + {}2", " ".repeat(100_000));
    let copies = std::iter::repeat_n("a", 100).collect::<Vec<_>>().join(",");
    let source = format!(
        "--algorithm A variable x=0; macro M(a) begin x:=<<{copies}>>; end macro; begin L:M({actual}); end algorithm"
    );
    let error = translate(&source, &TranslationOptions::default())
        .err()
        .ok_or("physical layout must be bounded before cloning")?;
    assert_eq!(error.diagnostic.code, Code::Resource);
    assert!(error.diagnostic.message.contains("before token allocation"));
    Ok(())
}

#[test]
fn physical_newline_mappings_keep_literal_global_utf8_statement_spans()
-> Result<(), Box<dyn std::error::Error>> {
    // Supplementary characters occupy four UTF-8 bytes/two UTF-16 units; tab
    // columns and global byte spans must not be confused with either encoding.
    let lf = "---- MODULE Case ----\nEXTENDS Naturals, TLC\n\\* 😀\n(* --algorithm A variables x=0;\nbegin (* 𝄞 *)\tA: x := 1;\n\tB: skip;\nend algorithm; *)\n====\n";
    let controls = [
        lf.to_owned(),
        lf.replace('\n', "\r\n"),
        lf.replace('\n', "\r"),
        lf.replacen('\n', "\r", 1),
        lf.replacen('\n', "\r\n", 1).replace(";\n", ";\r"),
    ];
    let mut body = None;
    for source in controls {
        let start = source.find("--algorithm").ok_or("algorithm")?;
        let end = source.rfind("*)").ok_or("algorithm end")?;
        let artifact = translate_at(&source, start..end, &TranslationOptions::default())?;
        assert_eq!(artifact.mappings.len(), 2);
        for (mapping, statement) in artifact.mappings.iter().zip(["A: x := 1;", "B: skip;"]) {
            let expected_start = source.find(statement).ok_or("literal statement")?;
            assert_eq!(mapping.original.start, expected_start);
            assert_eq!(mapping.original.end, expected_start + statement.len());
            assert_eq!(
                source.get(mapping.original.start..mapping.original.end),
                Some(statement)
            );
            assert!(mapping.generated.start < mapping.generated.end);
            assert!(
                artifact
                    .tla
                    .get(mapping.generated.start..mapping.generated.end)
                    .is_some()
            );
        }
        if let Some(expected) = &body {
            assert_eq!(
                &artifact.tla, expected,
                "metadata correction must not change generation"
            );
        } else {
            body = Some(artifact.tla);
        }
    }
    Ok(())
}

#[test]
fn physical_newline_macro_mappings_keep_literal_call_and_definition_spans()
-> Result<(), Box<dyn std::error::Error>> {
    let lf = "---- MODULE Case ----\nEXTENDS Naturals, TLC\n\\* 😀\n(* --algorithm A variable x=0;\nmacro M() begin x:=1; x:=2; end macro;\nbegin (* 𝄞 *)\tM();\nend algorithm; *)\n====\n";
    for source in [
        lf.to_owned(),
        lf.replace('\n', "\r\n"),
        lf.replace('\n', "\r"),
        lf.replacen('\n', "\r", 1),
    ] {
        let start = source.find("--algorithm").ok_or("algorithm")?;
        let end = source.rfind("*)").ok_or("algorithm end")?;
        let artifact = translate_at(&source, start..end, &TranslationOptions::default())?;
        let call = source.find("M();").ok_or("literal call")?;
        let expected = dive_pluscal::ast::Span {
            start: call,
            end: call + "M();".len(),
        };
        assert_eq!(artifact.mappings.len(), 2);
        assert!(
            artifact
                .mappings
                .iter()
                .all(|mapping| mapping.original == expected)
        );
        assert_eq!(artifact.inserted_labels.len(), 2);
        for (label, statement) in artifact.inserted_labels.iter().zip(["x:=1;", "x:=2;"]) {
            assert_eq!(label.source, expected);
            let at = source.find(statement).ok_or("literal definition")?;
            assert_eq!(
                label.macro_definition,
                Some(dive_pluscal::ast::Span {
                    start: at,
                    end: at + statement.len()
                })
            );
        }
    }
    Ok(())
}
