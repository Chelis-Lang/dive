//! Actual pinned particle identities and independent periodic acceptance controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_liveness::formula::{Atom, AtomKind, Formula, Limits, Node};
use dive_liveness::tlc_plan::{Projection, ProjectionError, Property, process};
use dive_liveness::tlc_tableau::{self, Error, Tableau, Term};
use serde_json::Value;
use std::collections::BTreeMap;
struct Parser<'a> {
    source: &'a [u8],
    at: usize,
    nodes: Vec<Node>,
}
impl Parser<'_> {
    fn parse(&mut self) -> usize {
        let c = self.source[self.at];
        self.at += 1;
        let node = match c {
            b'T' => Node::Bool(true),
            b'F' => Node::Bool(false),
            b's' | b'a' => {
                let start = self.at;
                while self.source.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
                let i = std::str::from_utf8(&self.source[start..self.at])
                    .expect("digits")
                    .parse()
                    .expect("index");
                Node::Atom(Atom {
                    kind: if c == b's' {
                        AtomKind::State
                    } else {
                        AtomKind::Action
                    },
                    index: i,
                })
            }
            _ => {
                assert_eq!(self.source[self.at], b'(');
                self.at += 1;
                let mut cs = Vec::new();
                while self.source[self.at] != b')' {
                    cs.push(self.parse());
                    if self.source[self.at] == b',' {
                        self.at += 1;
                    } else {
                        break;
                    }
                }
                assert_eq!(self.source[self.at], b')');
                self.at += 1;
                match c {
                    b'N' => Node::Not(cs[0]),
                    b'G' => Node::Always(cs[0]),
                    b'E' => Node::Eventually(cs[0]),
                    b'C' => Node::And(cs),
                    b'D' => Node::Or(cs),
                    _ => panic!("unknown input opcode"),
                }
            }
        };
        let i = self.nodes.len();
        self.nodes.push(node);
        i
    }
}
fn formula(s: &str) -> Formula {
    let mut p = Parser {
        source: s.as_bytes(),
        at: 0,
        nodes: Vec::new(),
    };
    let root = p.parse();
    assert_eq!(p.at, s.len());
    Formula::new(p.nodes, root).expect("checked formula")
}
fn inputs(case: &Value) -> (Vec<Formula>, Vec<Formula>) {
    let parse = |key| {
        case[key]
            .as_array()
            .expect("selection")
            .iter()
            .map(|s| formula(s.as_str().expect("formula")))
            .collect()
    };
    (parse("fairness"), parse("properties"))
}
fn planning(fs: &[Formula], ps: &[Formula], limits: Limits) -> Result<Projection, ProjectionError> {
    let names: Vec<_> = (0..ps.len())
        .map(|i| format!("OriginalProperty{i}"))
        .collect();
    let properties: Vec<_> = ps
        .iter()
        .zip(&names)
        .map(|(f, n)| Property {
            name: n,
            formula: f,
        })
        .collect();
    process(&fs.iter().collect::<Vec<_>>(), &properties, limits)
}

fn selection() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-tableau/selection.json"
    ))
    .expect("selection")
}
fn capture() -> Value {
    serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-tableau/capture.json"
    ))
    .expect("capture")
}
fn text(t: &Tableau, i: usize) -> String {
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
            .map(|c| text(t, c))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn fields(t: &[Option<Tableau>]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::from([("groups".into(), t.len().to_string())]);
    for (g, t) in t.iter().enumerate() {
        let key = format!("g{g}");
        out.insert(
            format!("{key}.binary"),
            t.as_ref()
                .map_or_else(|| "null".into(), |t| text(t, t.root)),
        );
        out.insert(
            format!("{key}.initial"),
            t.as_ref().map_or(0, |t| t.initial_count).to_string(),
        );
        out.insert(
            format!("{key}.nodes"),
            t.as_ref().map_or(0, |t| t.particles.len()).to_string(),
        );
        out.insert(
            format!("{key}.promises"),
            t.as_ref().map_or_else(String::new, |t| {
                t.promises
                    .iter()
                    .map(|i| text(t, *i))
                    .collect::<Vec<_>>()
                    .join(";")
            }),
        );
        if let Some(t) = t {
            for (i, p) in t.particles.iter().enumerate() {
                let key = format!("{key}.n{i}");
                out.insert(format!("{key}.index"), i.to_string());
                for (label, terms) in [("particle", &p.terms), ("state", &p.state_predicates)] {
                    out.insert(
                        format!("{key}.{label}"),
                        terms
                            .iter()
                            .map(|i| text(t, *i))
                            .collect::<Vec<_>>()
                            .join(";"),
                    );
                }
                out.insert(
                    format!("{key}.next"),
                    p.successors
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.insert(
                    format!("{key}.fulfills"),
                    p.fulfilling
                        .iter()
                        .map(bool::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.insert(format!("{key}.accepting"), p.accepting.to_string());
            }
        }
    }
    out
}
fn model(case: &Value) -> Option<dive_model::Model> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-tableau");
    let source = root.join(case["source"].as_str().expect("source"));
    let a = dive_sema::analyze_file(&source, &dive_sema::Options::default()).expect("read source");
    if !a.accepted() {
        return None;
    }
    let ir = std::sync::Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg =
        std::fs::read_to_string(root.join(case["config"].as_str().expect("config"))).expect("cfg");
    let cfg = dive_config::parse(&cfg).into_config().expect("config");
    Some(dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model"))
}
fn oracle_fields(row: &Value) -> BTreeMap<String, String> {
    row["fields"]
        .as_object()
        .expect("projected fields")
        .iter()
        .filter(|(key, _)| {
            key.as_str() == "groups"
                || key.starts_with('g') && key.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        })
        .map(|(key, value)| (key.clone(), value.as_str().expect("field text").to_owned()))
        .collect::<BTreeMap<_, _>>()
}
#[test]
fn all_actual_pin_particles_edges_promises_and_initial_ids_match() {
    let record = capture();
    let rows = record["rows"].as_array().expect("rows");
    let selected = selection();
    assert_eq!(rows.len(), 137);
    assert_eq!(selected.len(), rows.len());
    let (mut completed, mut planning_refused, mut direct_refused, mut sema_refused) = (0, 0, 0, 0);
    for (case, row) in selected.iter().zip(rows) {
        let name = case["name"].as_str().expect("name");
        let kind = case["kind"].as_str().expect("kind");
        assert_eq!(row["name"], format!("{kind}.{name}"));
        assert_eq!(row["timeout"], false);
        let projected = match kind {
            "planning" => {
                let (fs, ps) = inputs(case);
                match planning(&fs, &ps, Limits::default()) {
                    Ok(p) => tlc_tableau::project(&p, Limits::default())
                        .expect("bounded planning tableaux"),
                    Err(e) => {
                        planning_refused += 1;
                        assert_eq!(row["exit"], 1);
                        assert_eq!(
                            row["fields"]["failure_code"],
                            e.tlc_code().expect("reference code").to_string()
                        );
                        continue;
                    }
                }
            }
            "direct" => match tlc_tableau::project_formula(
                &formula(case["formula"].as_str().expect("formula")),
                Limits::default(),
            ) {
                Ok(t) => vec![Some(t)],
                Err(e) => {
                    direct_refused += 1;
                    match name {
                        "nonpositive-negation" => {
                            assert_eq!(row["exit"], 0);
                            assert!(matches!(e, Error::Reference(_)));
                        }
                        "empty-conjunction" | "empty-disjunction" => {
                            assert_eq!(row["exit"], 1);
                            assert!(matches!(e, Error::Reference(_)));
                        }
                        "action" => {
                            assert_eq!(row["exit"], 1);
                            assert_eq!(e, Error::Action);
                        }
                        _ => panic!("unexpected refusal {name}: {e}"),
                    }
                    continue;
                }
            },
            "model" => {
                let Some(m) = model(case) else {
                    sema_refused += 1;
                    assert_eq!(row["exit"], 1);
                    assert!(matches!(name, "TpActionBad" | "TtInstanceNested"));
                    continue;
                };
                let p = dive_liveness::tlc_plan::lower_model(&m, Limits::default())
                    .expect("pure bound planning");
                let configured = p
                    .configured_properties
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(";");
                assert_eq!(row["fields"]["configured_names"], configured);
                assert_eq!(
                    row["fields"]["action_names"], configured,
                    "original action/config identity {name}"
                );
                assert_eq!(
                    row["fields"]["role_names"],
                    p.projection.properties.join(";")
                );
                tlc_tableau::project(&p.projection, Limits::default())
                    .expect("bounded model tableaux")
            }
            _ => panic!("unknown kind"),
        };
        assert_eq!(row["exit"], 0, "{kind}.{name}");
        let actual = fields(&projected);
        let oracle = oracle_fields(row);
        assert_eq!(actual, oracle, "complete field set {kind}.{name}");
        completed += 1;
    }
    assert_eq!(
        (completed, planning_refused, direct_refused, sema_refused),
        (126, 5, 4, 2)
    );
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
fn group_formula(p: &Projection, g: usize) -> Formula {
    let group = &p.orders[g];
    let mut nodes = p.nodes().to_vec();
    if group.temporal.is_empty() {
        nodes.push(Node::Bool(true));
    } else {
        nodes.push(Node::And(group.temporal.clone()));
    }
    let root = nodes.len() - 1;
    Formula::new(nodes, root).expect("group formula")
}
fn width(f: &Formula) -> usize {
    f.nodes()
        .iter()
        .filter_map(|n| {
            if let Node::Atom(a) = n {
                Some(a.index + 1)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0)
}
fn words(width: usize, len: usize) -> Vec<Vec<Vec<bool>>> {
    let bits = width * len;
    let count = if bits <= 8 { 1usize << bits } else { 64 };
    (0..count)
        .map(|sample| {
            let mut random = u64::try_from(sample).expect("seed").wrapping_add(1);
            (0..len)
                .map(|pos| {
                    (0..width)
                        .map(|atom| {
                            if bits <= 8 {
                                sample & (1 << (pos * width + atom)) != 0
                            } else {
                                random = random
                                    .wrapping_mul(6_364_136_223_846_793_005)
                                    .wrapping_add(1);
                                random >> 63 != 0
                            }
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}
#[test]
fn periodic_language_and_closing_edges_match_direct_word_truth() {
    let mut cases = Vec::new();
    for case in selection() {
        if case["kind"] == "direct" {
            let f = formula(case["formula"].as_str().expect("formula"));
            if let Ok(t) = tlc_tableau::project_formula(&f, Limits::default()) {
                cases.push((case["name"].as_str().expect("name").to_owned(), f, t));
            }
        } else if case["kind"] == "planning" {
            let (fs, ps) = inputs(&case);
            if let Ok(p) = planning(&fs, &ps, Limits::default()) {
                let tables = tlc_tableau::project(&p, Limits::default()).expect("tableaux");
                for (g, t) in tables.into_iter().enumerate() {
                    if let Some(t) = t {
                        cases.push((format!("{} g{g}", case["name"]), group_formula(&p, g), t));
                    }
                }
            }
        }
    }
    let mut comparisons = 0;
    for (name, f, t) in cases {
        for len in 1..=4 {
            for word in words(width(&f), len) {
                for back in 0..len {
                    let truth = f
                        .evaluate_word(&word, &word, back, 100_000)
                        .expect("word truth");
                    for (pos, expected) in truth.iter().enumerate() {
                        let (suffix, closing) = rebase_word(&word, back, pos);
                        assert_eq!(
                            accepted(&t, &suffix, closing),
                            Some(*expected),
                            "{name} word={word:?} back={back} pos={pos}"
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    println!("independent tableau/word comparisons: {comparisons}");
    assert!(comparisons > 10_000);
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
fn scoped_models_match_independent_periodic_validator_at_every_word_position() {
    let mut comparisons = 0;
    for case in selection().into_iter().filter(|c| c["kind"] == "model") {
        let Some(m) = model(&case) else {
            continue;
        };
        let bound =
            dive_liveness::tlc_plan::lower_model(&m, Limits::default()).expect("bound planning");
        let p = &bound.projection;
        let tables = tlc_tableau::project(p, Limits::default()).expect("model tableaux");
        assert_eq!(m.ir.vars().len(), 1);
        for len in 1..=4 {
            for mask in 0..(1usize << len) {
                let states = (0..len)
                    .map(|pos| {
                        std::sync::Arc::from(vec![Some(dive_value::Value::Bool(
                            mask & (1 << pos) != 0,
                        ))])
                    })
                    .collect::<Vec<_>>();
                for back in 0..len {
                    let original = m
                        .temporals
                        .iter()
                        .chain(&m.implied_temporals)
                        .map(|n| {
                            dive_trace::temporal::evaluate_lasso(&m, &n.pred, &states, back)
                                .expect("independent periodic truth")
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
                                .evaluate(&m, &st)
                                .expect("pure atom");
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
                        let expected = original[..m.temporals.len()].iter().all(|v| v[pos])
                            && original[m.temporals.len()..].iter().any(|v| !v[pos]);
                        let (sr, closing) = rebase_word(&state_rows, back, pos);
                        let (ar, aclosing) = rebase_word(&action_rows, back, pos);
                        assert_eq!(closing, aclosing);
                        assert_eq!(
                            projected_holds(p, &tables, &sr, &ar, closing),
                            expected,
                            "{} len={len} mask={mask} back={back} pos={pos}",
                            case["name"]
                        );
                        comparisons += 1;
                    }
                }
            }
        }
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
    println!("independent scoped tableau/group comparisons: {comparisons}");
    assert_eq!(comparisons, 5536);
}
#[test]
fn limits_malformed_groups_and_particle_mutants_refuse_or_reject() {
    for limits in [
        Limits {
            formula_nodes: 0,
            ..Limits::default()
        },
        Limits {
            tableau_nodes: 0,
            ..Limits::default()
        },
        Limits {
            expansion_steps: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            tlc_tableau::project_formula(&formula("E(s0)"), limits),
            Err(Error::Limit(_))
        ));
    }
    let f = formula("G(s0)");
    let mut p = planning(&[f], &[], Limits::default()).expect("planning");
    p.orders[0].temporal.push(usize::MAX);
    assert!(matches!(
        tlc_tableau::project(&p, Limits::default()),
        Err(Error::Reference(_))
    ));
    let mut p = planning(&[formula("E(G(s0))")], &[], Limits::default()).expect("planning");
    p.orders[0].pems[0].ea_action[0] = usize::MAX;
    assert!(matches!(
        tlc_tableau::project(&p, Limits::default()),
        Err(Error::Reference(_))
    ));
    let mut p = planning(&[formula("G(s0)")], &[], Limits::default()).expect("planning");
    p.occurrences.clear();
    assert!(matches!(
        tlc_tableau::project(&p, Limits::default()),
        Err(Error::Reference(_))
    ));
    let mut nodes = vec![Node::Bool(true)];
    for _ in 0..258 {
        nodes.push(Node::Always(nodes.len() - 1));
    }
    let root = nodes.len() - 1;
    let deep = Formula::new(nodes, root).expect("checked deep");
    assert!(matches!(
        tlc_tableau::project_formula(&deep, Limits::default()),
        Err(Error::Limit("TLC tableau closure depth"))
    ));
    let t = tlc_tableau::project_formula(&formula("E(s0)"), Limits::default()).expect("eventual");
    assert_eq!(accepted(&t, &[vec![false]], 0), Some(false));
    assert_eq!(accepted(&t, &[vec![false], vec![true]], 1), Some(true));
    let mut corrupt = t.clone();
    corrupt.particles[0].successors = vec![usize::MAX];
    assert_eq!(accepted(&corrupt, &[vec![true]], 0), None);
    let mut corrupt = t.clone();
    for p in &mut corrupt.particles {
        p.fulfilling.fill(true);
    }
    assert_eq!(accepted(&corrupt, &[vec![false]], 0), Some(true));
    // A forged acceptance flag cannot change original periodic semantics.
    assert_eq!(
        formula("E(s0)")
            .evaluate_word(&[vec![false]], &[vec![]], 0, 20)
            .expect("direct truth"),
        [false]
    );
    let mut corrupt = t;
    corrupt.root = usize::MAX;
    assert_eq!(accepted(&corrupt, &[vec![true]], 0), None);
}
#[test]
fn complete_raw_source_selection_hashes_are_verified() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python3")
        .args(["-I", "scripts/check-tlc-tableau.py"])
        .current_dir(root)
        .output()
        .expect("verifier");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn tableau_pipeline_refuses_effects_opaque_captures_and_unchecked_ids_before_output() {
    let source = r#"---- MODULE PlanningHost ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == []<>x
Identity(p) == p
Capture(Op(_)) == Identity([]<>Op(x))
PrintProperty == []<>(PrintT(x))
LiveProperty == []<>(TLCGet("level") = 0)
====
"#;
    let analysis =
        dive_sema::analyze_source("PlanningHost.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        for case in [
            "PrintProperty",
            "LiveProperty",
            "opaque",
            "badroot",
            "badcapture",
            "baddef",
        ] {
            let mut model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
            let p = ir.root_def("P").expect("P");
            let mut pred = dive_model::Pred::new(
                ir.def(p).body,
                model.evaluator.scope(p).expect("valid P scope"),
            );
            match case {
                "PrintProperty" | "LiveProperty" => {
                    let d = ir.root_def(case).expect("property");
                    pred = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid property scope"),
                    );
                }
                "opaque" => {
                    let d = ir.root_def("Capture").expect("capture");
                    let scope = model
                        .evaluator
                        .scope_apply(
                            &pred.scope,
                            d,
                            &[dive_ir::Arg::Op {
                                op: dive_ir::OpRef::Std(dive_ir::StdOp::TlcPrintT),
                                level: dive_ir::Level::Constant,
                            }],
                        )
                        .expect("opaque capture");
                    pred = dive_model::Pred::new(ir.def(d).body, scope);
                }
                "badroot" => pred.expr = dive_ir::ExprId(u32::MAX),
                "badcapture" => {
                    let d = ir.root_def("Identity").expect("Identity");
                    let failure = model
                        .evaluator
                        .scope_apply(
                            &pred.scope,
                            d,
                            &[dive_ir::Arg::Expr {
                                expr: dive_ir::ExprId(u32::MAX),
                                pass: dive_ir::Pass::Lazy,
                            }],
                        )
                        .expect_err("missing lazy expression must fail before capture");
                    assert_eq!(failure.class(), dive_eval::ErrorClass::Unsupported);
                    assert!(failure.to_string().contains("DIVE-EVAL-C001"));
                    assert!(model.take_output().is_empty(), "{case} ran a callback");
                    continue;
                }
                "baddef" => {
                    let mut json = serde_json::to_value(ir.as_ref()).expect("IR serialization");
                    json["exprs"][pred.expr.index()]["expr"] = serde_json::json!({"Def":u32::MAX});
                    model.ir =
                        std::sync::Arc::new(serde_json::from_value(json).expect("unchecked IR"));
                }
                _ => panic!("unknown negative"),
            }
            model.implied_temporals.push(dive_model::Named {
                name: case.into(),
                pred,
            });
            let failure = dive_liveness::tlc_plan::lower_model(&model, Limits::default())
                .expect_err("unproved captured context");
            assert!(
                matches!(
                    failure,
                    dive_liveness::tlc_plan::ModelPlanningError::Bound(_)
                ),
                "{case}: {failure:?}"
            );
            assert!(model.take_output().is_empty(), "{case} ran a callback");
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one complete scoped witness and mutant census"
)]
fn independent_full_witness_validation_rejects_transition_closing_and_claim_mutants() {
    let names = [
        "TtLocalNested",
        "TtForwardNested",
        "TtEnabledNested",
        "TtFairNested",
        "TtVariableInstanceNested",
    ];
    let mut validations = 0;
    for case in selection()
        .into_iter()
        .filter(|c| names.contains(&c["name"].as_str().expect("name")))
    {
        // Fresh model per witness, independent of any production tableau/SCC.
        let m = model(&case).expect("pure scoped model");
        let bound = dive_liveness::tlc_plan::lower_model(&m, Limits::default()).expect("planning");
        assert!(tlc_tableau::project(&bound.projection, Limits::default()).is_ok());
        let claims = bound
            .configured_properties
            .iter()
            .map(|n| n.name.clone())
            .collect::<Vec<_>>();
        let mut valid = dive_trace::TraceDoc::behavior(vec![
            serde_json::json!({"x":false}),
            serde_json::json!({"x":true}),
        ])
        .expect("witness")
        .with_properties(claims);
        valid.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
        let check =
            |doc: &dive_trace::TraceDoc| dive_trace::validate_doc(&m, doc, &[], None).0.validation;
        assert_eq!(
            check(&valid),
            dive_trace::Validation::Valid,
            "{}",
            case["name"]
        );
        validations += 1;
        let mut closing = valid.clone();
        closing.ending = Some(dive_trace::Ending::Lasso { back_to: 1 });
        let result = check(&closing);
        if case["name"] == "TtEnabledNested" {
            // Stuttering remains a legitimate violation here; no invalid credit.
            assert_eq!(result, dive_trace::Validation::Valid);
        } else {
            assert!(
                matches!(result, dive_trace::Validation::Invalid(_)),
                "closing {} {result:?}",
                case["name"]
            );
        }
        let mut transition = valid.clone();
        transition.states[1].insert("x".into(), serde_json::Value::Bool(false));
        if case["name"] == "TtEnabledNested" {
            assert_eq!(check(&transition), dive_trace::Validation::Valid);
        } else {
            assert!(
                matches!(check(&transition), dive_trace::Validation::Invalid(_)),
                "Next {}",
                case["name"]
            );
        }
        let mut initial = valid.clone();
        initial.states[0].insert("x".into(), serde_json::Value::Bool(true));
        assert!(
            matches!(check(&initial), dive_trace::Validation::Invalid(_)),
            "Init {}",
            case["name"]
        );
        let mut claim = valid.clone();
        claim.properties = vec!["ForgedProperty".into()];
        assert_ne!(
            check(&claim),
            dive_trace::Validation::Valid,
            "claim {}",
            case["name"]
        );
        let mut range = valid;
        range.ending = Some(dive_trace::Ending::Lasso { back_to: 2 });
        assert!(
            matches!(check(&range), dive_trace::Validation::Invalid(_)),
            "range {}",
            case["name"]
        );
        validations += 5;
        let mut derived = range.clone();
        derived.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
        derived.properties = m.implied_temporals.iter().map(|n| n.name.clone()).collect();
        assert_ne!(
            derived.properties,
            bound
                .configured_properties
                .iter()
                .map(|n| n.name.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            matches!(check(&derived), dive_trace::Validation::Invalid(_)),
            "derived role used as configured claim {}",
            case["name"]
        );
        validations += 1;
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
    assert_eq!(validations, 35);
}

#[test]
fn scoped_three_state_witness_rejects_nonstuttering_next_and_closing_edges() {
    // Independent native admission/validation control, not extra oracle credit.
    let src = "---- MODULE ScopedClosing ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=(x+1)%3\nLive == LET Identity(p)==p IN Identity([](x=1 => [](x=1)))\n====\n";
    let a = dive_sema::analyze_source("ScopedClosing.tla", src, &dive_sema::Options::default());
    assert!(a.accepted());
    let ir = std::sync::Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .expect("cfg");
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        let m = dive_model::bind(ir.clone(), &cfg, &options).expect("fresh independent model");
        let bound = dive_liveness::tlc_plan::lower_model(&m, Limits::default()).expect("planning");
        assert!(tlc_tableau::project(&bound.projection, Limits::default()).is_ok());
        let mut doc =
            dive_trace::TraceDoc::behavior((0..3).map(|x| serde_json::json!({"x":x})).collect())
                .expect("word")
                .with_properties(vec!["Live".into()]);
        doc.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
        let check =
            |d: &dive_trace::TraceDoc| dive_trace::validate_doc(&m, d, &[], None).0.validation;
        assert_eq!(check(&doc), dive_trace::Validation::Valid);
        let mut closing = doc.clone();
        closing.ending = Some(dive_trace::Ending::Lasso { back_to: 1 });
        let dive_trace::Validation::Invalid(f) = check(&closing) else {
            panic!("nonstuttering closing edge accepted")
        };
        assert_eq!(f.check, dive_trace::Check::Next);
        let mut next = doc;
        next.states[1].insert("x".into(), serde_json::json!(2));
        let dive_trace::Validation::Invalid(f) = check(&next) else {
            panic!("nonstuttering Next edge accepted")
        };
        assert_eq!(f.check, dive_trace::Check::Next);
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
}

#[test]
fn every_fixed_budget_outcome_is_complete_or_typed_refusal() {
    let f = formula("D(C(E(s0),G(s1)),C(G(s0),E(s1)))");
    let complete = tlc_tableau::project_formula(&f, Limits::default()).expect("bounded reference");
    let mut counts = [0usize; 2];
    for terms in [0, 1, 4, 8, 16, 32, 64, 4096] {
        for nodes in [0, 1, 2, 4, 8, 16, 4096] {
            for work in [0, 1, 4, 32, 256, 4096, 1_000_000] {
                match tlc_tableau::project_formula(
                    &f,
                    Limits {
                        formula_nodes: terms,
                        tableau_nodes: nodes,
                        expansion_steps: work,
                    },
                ) {
                    Ok(actual) => {
                        assert_eq!(actual, complete);
                        counts[0] += 1;
                    }
                    Err(Error::Limit(_)) => counts[1] += 1,
                    Err(e) => panic!("unexpected bounded failure {e}"),
                }
            }
        }
    }
    assert_eq!(counts.iter().sum::<usize>(), 392);
    assert!(counts.into_iter().all(|c| c > 0));
}
