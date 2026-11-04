//! Source maps follow emitted actions, including the program-counter optimization.
use dive_pluscal::{Translation, TranslationOptions, translate};
use dive_syntax::ast::AstNode;

fn definition_regions(artifact: &Translation) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = "---- MODULE MapTest ----\nEXTENDS Naturals,TLC\n";
    let parsed = dive_syntax::parse_module(&format!("{prefix}{}\n====\n", artifact.tla));
    assert!(parsed.is_ok(), "{}", artifact.tla);
    let module = parsed.module().ok_or("module")?;
    let definitions = module
        .operator_definitions()
        .map(|definition| {
            let tokens = definition
                .syntax()
                .descendants_with_tokens()
                .filter_map(dive_syntax::SyntaxElement::into_token)
                .filter(|token| !token.kind().is_trivia())
                .collect::<Vec<_>>();
            let first = tokens.first().ok_or("definition start")?;
            let last = tokens.last().ok_or("definition end")?;
            Ok((
                usize::from(first.text_range().start()) - prefix.len(),
                usize::from(last.text_range().end()) - prefix.len(),
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    for mapping in &artifact.mappings {
        assert!(
            definitions.contains(&(mapping.generated.start, mapping.generated.end)),
            "mapping must cover one complete parsed definition: {mapping:?}\n{}",
            artifact.tla
        );
    }
    for pair in artifact.mappings.windows(2) {
        assert!(pair[0].generated.end < pair[1].generated.start);
    }
    Ok(())
}

#[test]
fn optimized_while_true_action_maps_to_its_original_labeled_loop()
-> Result<(), Box<dyn std::error::Error>> {
    let source =
        "--algorithm A variable x=0; begin Loop: while TRUE do x:=x+1; end while; end algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(!artifact.program_counter);
    assert_eq!(artifact.actions, ["Loop"]);
    assert_eq!(artifact.mappings.len(), 1, "{}", artifact.tla);
    definition_regions(&artifact)?;
    let map = &artifact.mappings[0];
    let generated = artifact
        .tla
        .get(map.generated.start..map.generated.end)
        .ok_or("generated region")?;
    assert!(generated.starts_with("Next =="), "{generated}");
    assert!(generated.contains("x' = x+1"), "{generated}");
    assert_eq!(
        source.get(map.original.start..map.original.end),
        Some("Loop: while TRUE do x:=x+1; end while;")
    );
    Ok(())
}

#[test]
fn optimized_process_names_keep_their_renamed_label_source_owners()
-> Result<(), Box<dyn std::error::Error>> {
    for (first, second, first_name, second_name) in [
        ("=1", "=2", "P ==", "Q =="),
        ("\\in {1}", "\\in {2}", "P(self) ==", "Q(self) =="),
    ] {
        let source = format!(
            "--algorithm A variable x=0;\nprocess P{first} begin Loop: while TRUE do x:=1; end while; end process;\nprocess Q{second} begin Loop: while TRUE do x:=2; end while; end process;\nend algorithm"
        );
        let artifact = translate(&source, &TranslationOptions::default())?;
        assert!(!artifact.program_counter, "{}", artifact.tla);
        assert_eq!(artifact.mappings.len(), 2, "{}", artifact.tla);
        assert_ne!(artifact.actions[0], artifact.actions[1]);
        assert!(
            artifact
                .warnings
                .iter()
                .any(|warning| warning.code == "symbols-renamed")
        );
        definition_regions(&artifact)?;
        for (mapping, name, original) in [
            (
                &artifact.mappings[0],
                first_name,
                "Loop: while TRUE do x:=1; end while;",
            ),
            (
                &artifact.mappings[1],
                second_name,
                "Loop: while TRUE do x:=2; end while;",
            ),
        ] {
            assert!(artifact.tla[mapping.generated.start..mapping.generated.end].starts_with(name));
            assert_eq!(
                source.get(mapping.original.start..mapping.original.end),
                Some(original)
            );
        }
    }
    Ok(())
}

#[test]
fn scoped_label_renaming_maps_each_emitted_definition_to_the_original_spelling()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A variable x=0;\nprocess P=1 begin Loop: x:=1; End: skip; end process;\nprocess Q=2 begin Loop: x:=2; End: skip; end process;\nend algorithm";
    let artifact = translate(source, &TranslationOptions::default())?;
    assert!(artifact.program_counter);
    assert_eq!(artifact.mappings.len(), 4, "{}", artifact.tla);
    assert_eq!(artifact.actions.len(), 4);
    assert_ne!(artifact.actions[0], artifact.actions[2]);
    definition_regions(&artifact)?;
    for ((mapping, action), original) in artifact.mappings.iter().zip(&artifact.actions).zip([
        "Loop: x:=1;",
        "End: skip;",
        "Loop: x:=2;",
        "End: skip;",
    ]) {
        let generated = &artifact.tla[mapping.generated.start..mapping.generated.end];
        assert!(
            generated.starts_with(&format!("{action} ==")),
            "{generated}"
        );
        assert_eq!(
            source.get(mapping.original.start..mapping.original.end),
            Some(original)
        );
    }
    Ok(())
}

#[test]
fn multiline_action_ranges_include_complete_branches_and_wrapped_updates()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "--algorithm A\nvariables x=0, message=\"L ==\", seen={1} ∪ {2}, another_variable=0;\nbegin\nL: if x=0 then\n     x:=x+1;\n   else\n     another_variable:=x;\n   end if;\nE: skip;\nend algorithm";
    for line_width in [60, 78, 100] {
        let artifact = translate(
            source,
            &TranslationOptions {
                line_width,
                ..TranslationOptions::default()
            },
        )?;
        assert!(artifact.program_counter);
        assert_eq!(artifact.mappings.len(), 2, "{}", artifact.tla);
        definition_regions(&artifact)?;
        let first = &artifact.mappings[0];
        let generated = &artifact.tla[first.generated.start..first.generated.end];
        assert!(generated.starts_with("L =="));
        assert!(generated.contains("ELSE"), "{generated}");
        assert!(generated.contains("another_variable' = x"));
        assert!(generated.lines().count() > 4);
        assert_eq!(
            source.get(first.original.start..first.original.end),
            Some("L: if x=0 then\n     x:=x+1;\n   else\n     another_variable:=x;\n   end if;")
        );
        let second = &artifact.mappings[1];
        assert_eq!(
            source.get(second.original.start..second.original.end),
            Some("E: skip;")
        );
        assert!(artifact.tla[second.generated.start..second.generated.end].starts_with("E =="));
    }
    Ok(())
}
