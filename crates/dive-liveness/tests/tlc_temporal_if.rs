//! Actual shared-guard tags and numbered tableaux before any verdict credit.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_liveness::formula::{Atom, AtomKind, Formula, Limits, Node};
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
fn complete_model_normalization_group_and_tableau_identity_selection() {
    complete_fields("tlc-temporal-if", 20);
    complete_fields("tlc-temporal-if-final", 25);
}
#[allow(
    clippy::too_many_lines,
    reason = "all selected fields and refusals retained"
)]
fn complete_fields(family: &str, expected_supported: usize) {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes")
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
            if matches!(name, "TiLet" | "TiForward") {
                // Existing scoped lowering expands this, whereas pinned TLC
                // rejects2213. Retain that diagnostic gap without field credit;
                // actual Session admission must still refuse before graph work.
                assert!(projection.is_ok(), "retained diagnostic observation {name}");
                let refused = dive_liveness::phase_l::Session::new(
                    &model,
                    0,
                    1,
                    dive_liveness::phase_l::Limits::default(),
                )
                .err()
                .expect("upstream-refused forwarding remains staged");
                assert!(matches!(
                    refused.error,
                    dive_liveness::phase_l::Error::Unsupported(_)
                ));
                assert_eq!(refused.progress.sources, 0);
                assert_eq!(refused.progress.actual_workers, 0);
            } else {
                assert!(projection.is_err(), "{name} actual reference failure");
            }
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
    assert_eq!((supported, refused), (expected_supported, 5));
}
// A separate finite-word product checker: state consistency, positive-path
// reachability and mutually reachable cyclic components. It never uses the
// production tableau/product/search algorithms to determine acceptance.
fn state_value(t: &Tableau, i: usize, row: &[bool]) -> Option<bool> {
    Some(match t.terms().get(i)? {
        Term::Bool(b) => *b,
        Term::Atom(a) => *row.get(a.index)?,
        Term::Not(c) => !state_value(t, *c, row)?,
        Term::And(a, b) => state_value(t, *a, row)? && state_value(t, *b, row)?,
        Term::Or(a, b) => state_value(t, *a, row)? || state_value(t, *b, row)?,
        _ => return None,
    })
}
fn accepted(t: &Tableau, word: &[Vec<bool>], back: usize) -> Option<bool> {
    if word.is_empty()
        || back >= word.len()
        || t.initial_count > t.particles.len()
        || t.root >= t.terms().len()
    {
        return None;
    }
    let n = t.particles.len();
    let size = n.checked_mul(word.len())?;
    if size > 4096 {
        return None;
    }
    for node in &t.particles {
        if node.successors.iter().any(|i| *i >= n)
            || node.fulfilling.len() != t.promises.len()
            || node.terms.iter().any(|i| *i >= t.terms().len())
        {
            return None;
        }
    }
    let mut enabled = vec![false; size];
    let mut edges = vec![Vec::new(); size];
    for (pos, row) in word.iter().enumerate() {
        for (i, node) in t.particles.iter().enumerate() {
            let mut holds = true;
            for &p in &node.state_predicates {
                holds &= state_value(t, p, row)?;
            }
            enabled[pos * n + i] = holds;
        }
    }
    for (pos, _) in word.iter().enumerate() {
        let next = if pos + 1 == word.len() { back } else { pos + 1 };
        for (i, node) in t.particles.iter().enumerate() {
            if enabled[pos * n + i] {
                for &j in &node.successors {
                    if enabled[next * n + j] {
                        edges[pos * n + i].push(next * n + j);
                    }
                }
            }
        }
    }
    let mut reach = vec![vec![false; size]; size];
    for start in 0..size {
        let mut pending = edges[start].clone();
        while let Some(i) = pending.pop() {
            if !reach[start][i] {
                reach[start][i] = true;
                pending.extend(&edges[i]);
            }
        }
    }
    for (c, reachable) in reach.iter().enumerate() {
        if !reachable[c] || !(0..t.initial_count).any(|i| enabled[i] && (i == c || reach[i][c])) {
            continue;
        }
        if (0..t.promises.len()).all(|p| {
            (0..size).any(|d| reachable[d] && reach[d][c] && t.particles[d % n].fulfilling[p])
        }) {
            return Some(true);
        }
    }
    Some(false)
}
fn rebase_word(word: &[Vec<bool>], back: usize, pos: usize) -> (Vec<Vec<bool>>, usize) {
    if pos < back {
        (word[pos..].to_vec(), back - pos)
    } else {
        (
            word[pos..]
                .iter()
                .chain(&word[back..pos])
                .cloned()
                .collect(),
            0,
        )
    }
}
fn projected_holds(
    p: &Projection,
    tables: &[Option<Tableau>],
    states: &[Vec<bool>],
    actions: &[Vec<bool>],
    back: usize,
) -> bool {
    p.orders.iter().zip(tables).any(|(g, t)| {
        let temporal = t
            .as_ref()
            .is_none_or(|t| accepted(t, states, back).expect("valid tableau word"));
        if !temporal {
            return false;
        }
        let value = |i: usize| {
            p.formula(i)
                .expect("bin")
                .evaluate_word(states, actions, back, 100_000)
                .expect("bin word")
        };
        g.pems.iter().any(|pem| {
            pem.ae_state
                .iter()
                .all(|i| value(g.check_state[*i])[back..].iter().any(|b| *b))
                && pem
                    .ae_action
                    .iter()
                    .all(|i| value(g.check_action[*i])[back..].iter().any(|b| *b))
                && pem
                    .ea_action
                    .iter()
                    .all(|i| value(g.check_action[*i])[back..].iter().all(|b| *b))
        })
    })
}

#[test]
fn sharing_preserves_independent_words_and_every_closing_position() {
    independent_words("tlc-temporal-if", 18, 5076);
    independent_words("tlc-temporal-if-final", 23, 6486);
}
fn independent_words(family: &str, expected_models: usize, expected_comparisons: usize) {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes")
        .join(family);
    let capture: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let mut comparisons = 0;
    let mut models = 0;
    for row in capture["rows"].as_array().expect("selection") {
        let name = row["name"].as_str().expect("name");
        if row["exit"] != 0 || matches!(name, "IfCallError" | "IfNonBool") {
            continue;
        }
        let dir = base.join("models").join(name);
        let analyzed = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("file");
        let ir = std::sync::Arc::new(dive_ir::lower(&analyzed).expect("IR"));
        let config = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
        let model = dive_model::bind(
            ir,
            &dive_config::parse(&config).into_config().expect("cfg"),
            &dive_model::BindOptions::tlc(),
        )
        .expect("binding");
        let bound =
            dive_liveness::tlc_plan::lower_model(&model, Limits::default()).expect("pure planning");
        let p = &bound.projection;
        let tables = dive_liveness::tlc_tableau::project(p, Limits::default()).expect("tableaux");
        models += 1;
        for len in 1..=3usize {
            for mask in 0..3usize.pow(u32::try_from(len).expect("length")) {
                let states = (0..len)
                    .map(|pos| {
                        std::sync::Arc::from(vec![Some(dive_value::Value::int(
                            i32::try_from(
                                (mask / 3usize.pow(u32::try_from(pos).expect("pos"))) % 3,
                            )
                            .expect("small")
                            .into(),
                        ))])
                    })
                    .collect::<Vec<_>>();
                for back in 0..len {
                    let original = model
                        .temporals
                        .iter()
                        .chain(&model.implied_temporals)
                        .map(|n| {
                            dive_trace::temporal::evaluate_lasso(&model, &n.pred, &states, back)
                                .expect("fresh independent word truth")
                        })
                        .collect::<Vec<_>>();
                    let mut state_rows = Vec::new();
                    let mut action_rows = Vec::new();
                    for pos in 0..len {
                        let next = if pos + 1 == len { back } else { pos + 1 };
                        let st = dive_eval::St::action(states[pos].clone(), states[next].clone());
                        let mut sr = vec![false; p.occurrences.len()];
                        let mut ar = sr.clone();
                        for (tag, o) in p.occurrences.iter().enumerate() {
                            let value = bound.inputs[o.input].atoms[o.atom.index]
                                .evaluate(&model, &st)
                                .expect("pure predicate");
                            if o.atom.kind == AtomKind::State {
                                sr[tag] = value;
                            } else {
                                ar[tag] = value;
                            }
                        }
                        state_rows.push(sr);
                        action_rows.push(ar);
                    }
                    for pos in 0..len {
                        let expected = original[..model.temporals.len()].iter().all(|v| v[pos])
                            && original[model.temporals.len()..].iter().any(|v| !v[pos]);
                        let (sr, closing) = rebase_word(&state_rows, back, pos);
                        let (ar, aclosing) = rebase_word(&action_rows, back, pos);
                        assert_eq!(closing, aclosing);
                        assert_eq!(
                            projected_holds(p, &tables, &sr, &ar, closing),
                            expected,
                            "{name} len={len} mask={mask} back={back} pos={pos}"
                        );
                        comparisons += 1;
                    }
                }
            }
        }
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
    assert_eq!(models, expected_models);
    assert_eq!(comparisons, expected_comparisons);
}
#[test]
fn shared_objects_keep_last_tags_holes_and_bounded_rewrite_work() {
    let atom = Node::Atom(Atom {
        kind: AtomKind::State,
        index: 0,
    });
    let formula = Formula::new(vec![atom, Node::And(vec![0, 0, 0]), Node::Always(1)], 2)
        .expect("shared arena");
    let p = dive_liveness::tlc_plan::process(
        &[],
        &[dive_liveness::tlc_plan::Property {
            name: "Original",
            formula: &formula,
        }],
        Limits::default(),
    )
    .expect("bounded");
    assert_eq!(p.occurrences.len(), 3);
    assert_eq!(
        text(p.nodes(), p.tagged.expect("root")),
        "N(G(C(s3,s3,s3)))"
    );
    let formula = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::And(vec![0, 1]),
        ],
        2,
    )
    .expect("separate arenas");
    let p = dive_liveness::tlc_plan::process(
        &[],
        &[dive_liveness::tlc_plan::Property {
            name: "Original",
            formula: &formula,
        }],
        Limits::default(),
    )
    .expect("bounded");
    assert_eq!(
        text(p.nodes(), p.tagged.expect("root")),
        "N(C(s1,s2))",
        "truth equality never merges separately constructed objects"
    );
    let wide = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::And(vec![0; 256]),
        ],
        1,
    )
    .expect("bounded input");
    assert!(matches!(
        dive_liveness::tlc_plan::process(
            &[],
            &[dive_liveness::tlc_plan::Property {
                name: "Original",
                formula: &wide
            }],
            Limits {
                expansion_steps: 1000,
                ..Limits::default()
            }
        ),
        Err(dive_liveness::tlc_plan::ProjectionError::Formula(
            dive_liveness::formula::FormulaError::Limit("TLC planning work")
        ))
    ));
}
