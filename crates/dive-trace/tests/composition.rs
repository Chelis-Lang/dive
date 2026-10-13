//! Standalone replay remembers refused composition even in an optional label.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_eval::ActionComposition;
use dive_trace::{COMPOSITION_UNSUPPORTED, Label, TraceDoc, Validation};
use serde_json::json;

#[test]
fn optional_label_composition_cannot_be_swallowed_into_a_valid_word() {
    let source = "---- MODULE Composition ----\nVARIABLE x\nInit == x=0\nNext == x'=2\nAlias == (x'=1) \\cdot (x'=2)\n====\n";
    let analysis =
        dive_sema::analyze_source("Composition.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let mut opts = dive_model::BindOptions::tlc();
        opts.eval.action_composition = mode;
        let m = dive_model::bind(ir.clone(), &cfg, &opts).expect("model");
        let mut doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})]).expect("document");
        assert_eq!(
            dive_trace::validate_doc(&m, &doc, &[]).0.validation,
            Validation::Valid
        );
        doc.labels[1] = Some(Label {
            name: "Alias".to_owned(),
            location: None,
        });
        assert_eq!(
            dive_trace::validate_doc(&m, &doc, &[]).0.validation,
            Validation::Unsupported(COMPOSITION_UNSUPPORTED.to_owned())
        );
    }
}
