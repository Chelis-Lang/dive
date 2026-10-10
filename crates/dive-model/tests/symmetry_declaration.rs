//! Early declaration stages against normal pinned TLC runs.
#![allow(clippy::unwrap_used, reason = "test assertions")]

use std::path::Path;
use std::sync::Arc;

use dive_model::{BindOptions, ErrorKind, Model};

fn model(name: &str, source: &str, config: &str) -> Model {
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    let config = dive_config::parse(config).into_config().unwrap();
    dive_model::bind(ir, &config, &BindOptions::tlc()).unwrap()
}

fn fixture(name: &str) -> Model {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/symmetry-group/specs")
        .join(name);
    model(
        name,
        &std::fs::read_to_string(root.join(format!("{name}.tla"))).unwrap(),
        &std::fs::read_to_string(root.join(format!("{name}.cfg"))).unwrap(),
    )
}

#[test]
fn early_errors_match_clean_pinned_tlc_messages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/symmetry-group/tlc-2026.10.06.014338");
    for name in [
        "SyMissing",
        "SyConstant",
        "SyNotSet",
        "SyTuple",
        "SyAssignment",
    ] {
        let model = fixture(name);
        let error = model.symmetry_declaration().unwrap_err();
        let transcript = std::fs::read_to_string(root.join(format!("{name}.stdout"))).unwrap();
        assert!(transcript.contains(&error.tool_text()), "{name}: {error}");
        assert_eq!(
            error.code(),
            Some(if name == "SyMissing" { 2229 } else { 1000 })
        );
    }
}

#[test]
fn original_definition_survives_constant_evaluation_and_keeps_identity_functions() {
    for (name, count) in [
        ("SySmall", 1),
        ("SyUnused", 3),
        ("SyNonBiject", 2),
        ("SyEmpty", 0),
        ("SyNonModel", 2),
        ("SySubstitution", 2),
    ] {
        let model = fixture(name);
        let declaration = model.symmetry_declaration().unwrap().unwrap();
        assert_eq!(declaration.name, "Sym");
        assert_eq!(declaration.functions.len(), count, "{name}");
        assert!(
            model
                .unsupported
                .iter()
                .any(|f| f.feature == "SYMMETRY Sym")
        );
        assert_eq!(
            model
                .eval(&declaration.predicate, &dive_eval::St::constant())
                .unwrap()
                .as_set()
                .unwrap()
                .elements(model.evaluator.options().ctx)
                .unwrap(),
            declaration.functions
        );
    }
}

#[test]
fn snapshot_preserves_cfg_substitution_and_assignment_distinction() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT S\nVARIABLE x\nInit == x = TRUE\nNext == UNCHANGED x\nSym == TRUE\nOther == Permutations(S)\n====\n";
    let base = "CONSTANT S={a,b}\nINIT Init\nNEXT Next\nSYMMETRY Sym\n";
    let replaced = model("M", source, &format!("{base}CONSTANT Sym <- Other\n"));
    assert_eq!(
        replaced
            .symmetry_declaration()
            .unwrap()
            .unwrap()
            .functions
            .len(),
        2
    );
    let assigned = model("M", source, &format!("{base}CONSTANT Sym = TRUE\n"));
    let error = assigned.symmetry_declaration().unwrap_err();
    assert_eq!(error.code(), Some(1000));
    assert!(error.message.contains("symmetry function Sym"));
}

#[test]
fn effectful_or_state_dependent_declarations_are_named_refusals() {
    for declaration in [
        "Sym == Print(\"sym\", {<<>>})",
        "Sym == IF x THEN {} ELSE {}",
        "Sym(p) == {}",
    ] {
        let source = format!(
            "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == UNCHANGED x\n{declaration}\n====\n"
        );
        let model = model("M", &source, "INIT Init\nNEXT Next\nSYMMETRY Sym\n");
        let _ = model.take_output();
        let error = model.symmetry_declaration().unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Unsupported { .. }));
        assert!(error.message.contains("SYMMETRY Sym"));
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
    let no_symmetry = model(
        "M",
        "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == UNCHANGED x\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    assert!(no_symmetry.symmetry_declaration().unwrap().is_none());
}

#[test]
fn unverified_startup_errors_are_refusals_not_tlc_claims() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/symmetry-group/tlc-2026.10.06.014338");
    for (name, detail) in [("SyDivZero", "2179"), ("SyMixedElements", "normalization")] {
        let model = fixture(name);
        let error = model.symmetry_declaration().unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Unsupported { .. }));
        assert_eq!(error.code(), None);
        assert!(error.message.contains("SYMMETRY Sym"));
        assert!(error.message.contains(detail));
        let raw = std::fs::read_to_string(root.join(format!("{name}.stdout"))).unwrap();
        assert!(raw.contains("STARTMSG 1000:1"));
        assert_eq!(
            std::fs::read_to_string(root.join(format!("{name}.exit")))
                .unwrap()
                .trim(),
            "255"
        );
    }
}
