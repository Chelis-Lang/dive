//! Genuine and forged words for primary-backed lazy query contexts.
#![allow(clippy::expect_used, reason = "pinned source fixtures must bind")]

use std::{path::Path, sync::Arc};

use dive_model::BindOptions;
use dive_trace::{Check, Ending, Trace, TraceDoc, Validation};
use dive_value::Value;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one primary/forged/transport matrix keeps all expected outcomes together"
)]
fn lazy_primed_query_words_match_primary_in_both_transport_paths() {
    let cases = [
        ("lazy-enabled-capture", "primed_disabled", true),
        ("lazy-enabled-capture", "primed_enabled", false),
        ("lazy-enabled-capture", "constant_disabled", true),
        ("lazy-enabled-capture", "state_enabled", false),
        ("lazy-enabled-cache", "hidden_remove_two", true),
        ("lazy-enabled-cache", "hidden_unchanged", true),
        ("lazy-enabled-cache", "hidden_remove_one", false),
        ("lazy-enabled-cache", "hidden_singleton_two", false),
        ("lazy-enabled-cache", "hidden_inline_remove_two", false),
        ("lazy-enabled-cache", "root_remove_two", false),
        ("context", "outside_let_unchanged", false),
        ("context", "outside_let_remove_one", true),
        ("context", "inside_argument_unchanged", true),
        ("context", "inside_argument_remove_one", false),
        ("context", "outside_let_alias_unchanged", false),
        ("context", "outside_let_alias_remove_one", true),
        ("context", "membership_argument_unchanged", true),
        ("context", "membership_argument_remove_two", true),
    ];
    check_words(&cases);
}

#[test]
fn reached_lexical_lifetimes_match_primary_in_both_transport_paths() {
    check_words(&[
        (
            "lazy-query-lifetimes",
            "after_membership_helper_remove_one",
            false,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_helper_remove_two",
            false,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_helper_unchanged",
            true,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_let_remove_one",
            false,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_let_remove_two",
            false,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_let_remove_two_inverse",
            true,
        ),
        (
            "lazy-query-lifetimes",
            "after_membership_let_unchanged",
            true,
        ),
        (
            "lazy-query-lifetimes",
            "delayed_false_guard_remove_one",
            true,
        ),
        (
            "lazy-query-lifetimes",
            "delayed_false_guard_remove_two",
            true,
        ),
        (
            "lazy-query-lifetimes",
            "delayed_false_guard_unchanged",
            true,
        ),
        ("lazy-query-lifetimes", "delayed_helper_remove_one", false),
        ("lazy-query-lifetimes", "delayed_helper_remove_two", true),
        ("lazy-query-lifetimes", "delayed_helper_unchanged", true),
        ("lazy-query-lifetimes", "delayed_let_nested", true),
        ("lazy-query-lifetimes", "delayed_let_remove_one", false),
        ("lazy-query-lifetimes", "delayed_let_remove_two", true),
        (
            "lazy-query-lifetimes",
            "delayed_let_remove_two_inverse",
            false,
        ),
        ("lazy-query-lifetimes", "delayed_let_sibling_eq", false),
        ("lazy-query-lifetimes", "delayed_let_sibling_or", true),
        ("lazy-query-lifetimes", "delayed_let_unchanged", true),
        ("lazy-query-lifetimes", "leading_helper_remove_one", false),
        ("lazy-query-lifetimes", "leading_helper_remove_two", true),
        (
            "lazy-query-lifetimes",
            "leading_helper_remove_two_inverse",
            false,
        ),
        ("lazy-query-lifetimes", "leading_helper_unchanged", true),
    ]);
}

#[allow(
    clippy::too_many_lines,
    reason = "one primary and forged word matrix keeps every transport outcome together"
)]
fn check_words(cases: &[(&str, &str, bool)]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut failures = Vec::new();
    for &(corpus, name, violation) in cases {
        let fixture = if corpus == "context" {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lazy-query-context")
                .join(name)
        } else {
            root.join("conformance/corpus")
                .join(corpus)
                .join("specifications")
                .join(name)
        };
        for options in [BindOptions::tlc(), BindOptions::dive()] {
            let analysis = dive_sema::analyze_file(
                &fixture.join("CallbackEnabled.tla"),
                &dive_sema::Options::default(),
            )
            .expect("primary source");
            assert!(analysis.accepted(), "{name}: {:?}", analysis.diagnostics());
            let config = dive_config::parse(
                &std::fs::read_to_string(fixture.join("CallbackEnabled.cfg")).expect("config"),
            )
            .into_config()
            .expect("config parses");
            let model = dive_model::bind(
                Arc::new(dive_ir::lower(&analysis).expect("IR")),
                &config,
                &options,
            )
            .expect("model");
            let state = model
                .ir
                .vars()
                .iter()
                .map(|decl| {
                    Some(match model.ir.decl(*decl).name.as_str() {
                        "s" | "c" => Value::tuple(vec![1.into(), 2.into(), 3.into()]),
                        _ => 0.into(),
                    })
                })
                .collect::<Vec<_>>();
            let trace = Trace {
                states: vec![state.clone().into()],
                labels: vec![None],
                ending: Ending::Invariant("Inv".into()),
                length: Some(1),
                properties: Vec::new(),
                concrete_closure: false,
                witnesses: Vec::new(),
            };
            let mut bad_init = trace.clone();
            let mut forged = state;
            let first = model
                .ir
                .vars()
                .iter()
                .position(|d| matches!(model.ir.decl(*d).name.as_str(), "a" | "b"))
                .expect("scalar root");
            forged[first] = Some(99.into());
            bad_init.states[0] = forged.into();
            for (kind, word) in [
                ("primary invariant ending", trace),
                ("forged Init", bad_init),
            ] {
                let typed = dive_trace::validate(&model, &word).validation;
                let doc = word.to_doc(&model).expect("document");
                let parsed = TraceDoc::parse(&doc.to_json().to_string()).expect("transport");
                let serial = dive_trace::validate_doc(&model, &parsed, &config.model_values)
                    .0
                    .validation;
                for (route, result) in [("typed", typed), ("document", serial)] {
                    let expected = if kind == "forged Init" {
                        matches!(&result, Validation::Invalid(f) if f.state == 0 && f.check == Check::Init)
                    } else if violation {
                        result == Validation::Valid
                    } else {
                        matches!(&result, Validation::Invalid(f) if f.state == 0 && f.check == Check::Ending)
                    };
                    if !expected {
                        failures.push(format!(
                            "{corpus}/{name}/{options:?}/{kind}/{route}: {result:?}"
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
