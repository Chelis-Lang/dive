//! Scoped forwarding with exact exposed identity and numbered tableau fields.
//! The reference expanded-action vector is retained without native equality credit.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_liveness::formula::{AtomKind, Limits, Node};
use dive_liveness::tlc_plan::Projection;
use dive_liveness::tlc_tableau::{Tableau, Term};
use serde_json::Value;
use std::collections::BTreeMap;
fn text(nodes: &[Node], i: usize) -> String {
    let (op, children) = match &nodes[i] {
        Node::Bool(b) => return if *b { "T" } else { "F" }.into(),
        Node::Atom(a) => {
            return format!(
                "{}{}",
                if a.kind == AtomKind::State { "s" } else { "a" },
                a.index + 1
            );
        }
        Node::Not(c) => ("N", vec![*c]),
        Node::Always(c) => ("G", vec![*c]),
        Node::Eventually(c) => ("E", vec![*c]),
        Node::And(cs) => ("C", cs.clone()),
        Node::Or(cs) => ("D", cs.clone()),
    };
    format!(
        "{op}({})",
        children
            .into_iter()
            .map(|c| text(nodes, c))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn indexes(xs: &[usize]) -> String {
    xs.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
fn fields(p: &Projection) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if p.tagged.is_none() {
        out.insert("empty".into(), "true".into());
        return out;
    }
    for (key, root) in [
        ("tagged", p.tagged),
        ("pushed", p.pushed),
        ("simplified", p.simplified),
        ("dnf", p.dnf),
    ] {
        out.insert(key.into(), text(p.nodes(), root.expect("stage")));
    }
    out.insert("groups".into(), p.orders.len().to_string());
    for (i, g) in p.orders.iter().enumerate() {
        let temporal = match g.temporal.as_slice() {
            [] => "null".into(),
            [c] => text(p.nodes(), *c),
            cs => format!(
                "C({})",
                cs.iter()
                    .map(|c| text(p.nodes(), *c))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        out.insert(format!("g{i}.temporal"), temporal);
        out.insert(
            format!("g{i}.state"),
            g.check_state
                .iter()
                .map(|c| text(p.nodes(), *c))
                .collect::<Vec<_>>()
                .join(";"),
        );
        out.insert(
            format!("g{i}.action"),
            g.check_action
                .iter()
                .map(|c| text(p.nodes(), *c))
                .collect::<Vec<_>>()
                .join(";"),
        );
        for (j, pem) in g.pems.iter().enumerate() {
            out.insert(
                format!("g{i}.p{j}"),
                format!(
                    "AEAction={}|AEState={}|EAAction={}",
                    indexes(&pem.ae_action),
                    indexes(&pem.ae_state),
                    indexes(&pem.ea_action)
                ),
            );
        }
    }
    out
}
fn tableau_text(t: &Tableau, i: usize) -> String {
    let (op, children) = match t.terms()[i] {
        Term::Bool(b) => return if b { "T" } else { "F" }.into(),
        Term::Atom(a) => return format!("s{}", a.index + 1),
        Term::Not(c) => ("N", vec![c]),
        Term::Always(c) => ("G", vec![c]),
        Term::Eventually(c) => ("E", vec![c]),
        Term::Next(c) => ("X", vec![c]),
        Term::And(a, b) => ("C", vec![a, b]),
        Term::Or(a, b) => ("D", vec![a, b]),
    };
    format!(
        "{op}({})",
        children
            .into_iter()
            .map(|c| tableau_text(t, c))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete field-level selection retains every identity and failure"
)]
fn scoped_normalization_group_and_tableau_identity_selection() {
    complete_fields("tlc-final-scopes", 16);
}
#[allow(
    clippy::too_many_lines,
    reason = "all selected fields and refusals retained"
)]
fn complete_fields(family: &str, expected_supported: usize) {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes")
        .join(family);
    let capture: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let mut supported = 0;
    let mut refused = 0;
    for row in capture["rows"].as_array().expect("complete selection") {
        let name = row["name"].as_str().expect("name");
        let dir = base.join("models").join(name);
        let analyzed = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("file");
        assert!(analyzed.accepted(), "{name} SANY-compatible input");
        let ir = std::sync::Arc::new(dive_ir::lower(&analyzed).expect("IR"));
        let config = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
        let config = dive_config::parse(&config).into_config().expect("cfg");
        let model =
            dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).expect("binding");
        let projection = dive_liveness::tlc_plan::lower_model(&model, Limits::default());
        if row["exit"] != 0 {
            refused += 1;
            assert!(projection.is_err(), "{name} actual reference failure");
            continue;
        }
        supported += 1;
        let bound = projection.expect("supported field projection");
        let p = &bound.projection;
        let oracle = &row["fields"];
        assert_eq!(
            p.occurrences.len().to_string(),
            oracle["tag_count"],
            "{name} visit count includes shared-tag holes"
        );
        for (key, value) in fields(p) {
            assert_eq!(
                value, oracle[&key],
                "{name} exact normalization/group field {key}"
            );
        }
        assert_eq!(
            bound
                .configured_properties
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(";"),
            oracle["configured_properties"],
            "{name} configured identity"
        );
        assert_eq!(
            p.properties.join(";"),
            oracle["temporal_role_names"],
            "{name} distinct role label"
        );
        let tableaux = dive_liveness::tlc_tableau::project(p, Limits::default()).expect("tableaux");
        assert_eq!(tableaux.len().to_string(), oracle["actual.groups"]);
        for (g, t) in tableaux.iter().enumerate() {
            let k = format!("actual.g{g}");
            assert_eq!(
                t.as_ref().map_or(0, |t| t.particles.len()).to_string(),
                oracle[format!("{k}.tableauNodes")],
                "{name} numbered cardinality"
            );
            if let Some(t) = t {
                assert_eq!(
                    t.initial_count.to_string(),
                    oracle[format!("{k}.initial_count")],
                    "{name} initial identities"
                );
                assert_eq!(
                    t.promises
                        .iter()
                        .map(|i| tableau_text(t, *i))
                        .collect::<Vec<_>>()
                        .join(";"),
                    oracle[format!("{k}.promises")],
                    "{name} promise identity"
                );
                for (n, particle) in t.particles.iter().enumerate() {
                    assert_eq!(
                        particle
                            .terms
                            .iter()
                            .map(|i| tableau_text(t, *i))
                            .collect::<Vec<_>>()
                            .join(";"),
                        oracle[format!("{k}.n{n}.particle")],
                        "{name} complete particle {n}"
                    );
                    assert_eq!(
                        particle
                            .successors
                            .iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        oracle[format!("{k}.n{n}.next")],
                        "{name} exact edge order {n}"
                    );
                }
            }
        }
    }
    assert_eq!((supported, refused), (expected_supported, 2));
}
