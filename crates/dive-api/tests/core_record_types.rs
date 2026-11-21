//! Importing unions of record families preserves membership and real enumeration.
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/core_final.rs"]
mod core_final;

use dive_runtime::CheckOptions;

const SPEC: &str = r#"---- MODULE Family ----
EXTENDS Naturals, FiniteSets
VARIABLE x
U == [tag: {"left"}, a: 1..2] \cup [tag: {"right"}, a: {"s"}]
Envelope == [body: U]
Init == x = {[body |-> [tag |-> "right", a |-> "s"]]}
Next == x' = {[body |-> [tag |-> "unknown", a |-> "s"]]}
Inv == x \in SUBSET Envelope
Enumeration == Cardinality(Envelope) = 3
V == IF TRUE THEN x ELSE Envelope
====
"#;

#[test]
fn later_payload_and_invalid_tag_preserve_the_shortest_trace() {
    let cfg = "INIT Init\nNEXT Next\nINVARIANTS Inv Enumeration\nVIEW V\n";
    // A homogeneous payload is supported in both profiles. The heterogeneous
    // case exercises canonical replay; compat refuses its mixed enumeration.
    let single_payload = SPEC.replace("a: 1..2", r#"a: {"t", "u"}"#);
    for (source, opts, compat) in [
        (SPEC, CheckOptions::dive(0), false),
        (single_payload.as_str(), CheckOptions::dive(0), false),
        (single_payload.as_str(), CheckOptions::tlc(0, 0), true),
    ] {
        let files = core_final::Files::new(source, cfg);
        let row = dive_api::check_file(&files.request(&opts))
            .expect("check")
            .to_json();
        assert_eq!(row["verdict"], "invariant-violation", "{row}");
        assert_eq!(row["statistics"]["depth"], 2);
        let model = dive_api::trace::SpecModel::load(
            &files.file,
            &files.file.with_extension("cfg"),
            &dive_api::trace::TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..Default::default()
            },
        )
        .expect("independent model");
        let trace = dive_api::trace::TraceDoc::parse(&row["counterexample"]["trace"].to_string())
            .expect("trace");
        assert_eq!(
            model.validate(&trace).validation,
            dive_api::trace::Validation::Valid
        );
    }
}

#[test]
fn an_invalid_function_domain_cannot_prove_membership() {
    let source = SPEC
        .replace(
            r#"[tag: {"left"}, a: 1..2] \cup [tag: {"right"}, a: {"s"}]"#,
            r#"[a: [7 -> {0}], b: {0}] \cup [a: {[k |-> 0]}, b: {0}]"#,
        )
        .replace(
            r#"[tag |-> "right", a |-> "s"]"#,
            "[a |-> [k |-> 0], b |-> 0]",
        )
        .replace(
            r#"Next == x' = {[body |-> [tag |-> "unknown", a |-> "s"]]}"#,
            "Next == x' = x",
        );
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let verdict = core_final::run(
            &source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW V\n",
            &opts,
        )
        .to_json();
        assert_eq!(verdict["verdict"], "unsupported", "{verdict}");
    }
}
