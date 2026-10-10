//! Field-level pinned normalization projections and independent word controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_liveness::formula::{Atom, AtomKind, Formula, FormulaError, Limits, Node};
use dive_liveness::tlc_plan::{Projection, ProjectionError, Property, process};
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
fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-planning/cases.json"
    ))
    .expect("cases")
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
fn project(fs: &[Formula], ps: &[Formula], limits: Limits) -> Result<Projection, ProjectionError> {
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
#[test]
fn pinned_stages_and_ordered_bins() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-planning/capture.json"
    ))
    .expect("capture");
    let rows = capture["rows"].as_array().expect("rows");
    let cases = cases();
    assert_eq!(cases.len(), rows.len());
    for (case, row) in cases.iter().zip(rows) {
        assert_eq!(row["name"], case["name"]);
        assert_eq!(row["timeout"], false);
        let (fs, ps) = inputs(case);
        let native = project(&fs, &ps, Limits::default());
        if case["name"] == "dnf-integer-overflow" {
            assert_eq!(row["exit"], 1);
            assert_eq!(native.as_ref().expect_err("bound").tlc_code(), Some(2213));
            assert_eq!(row["fields"]["failure_code"], "2213");
            assert!(matches!(
                native,
                Err(ProjectionError::Formula(FormulaError::Limit(
                    "TLC DNF cardinality"
                )))
            ));
            continue;
        }
        assert_eq!(row["exit"], 0);
        if row["fields"]["refusal"].is_string() {
            assert_eq!(native.as_ref().expect_err("shape").tlc_code(), Some(2214));
            assert_eq!(row["fields"]["refusal_code"], "2214");
            assert!(
                matches!(native, Err(ProjectionError::ActionInTemporal)),
                "{}: {native:?}",
                case["name"]
            );
        } else {
            let native = native.expect("supported pinned projection");
            let expected: BTreeMap<String, String> = row["fields"]
                .as_object()
                .expect("fields")
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().expect("field").into()))
                .collect();
            assert_eq!(fields(&native), expected, "{}", case["name"]);
            assert_eq!(
                native.properties,
                (0..ps.len())
                    .map(|i| format!("OriginalProperty{i}"))
                    .collect::<Vec<_>>()
            );
        }
    }
}

fn grouped(p: &Projection) -> Formula {
    let mut nodes = p.nodes().to_vec();
    let mut groups = Vec::new();
    let push = |nodes: &mut Vec<Node>, n| {
        let i = nodes.len();
        nodes.push(n);
        i
    };
    for group in &p.orders {
        for pem in &group.pems {
            let mut conjuncts = group.temporal.clone();
            for &c in &pem.ae_state {
                let e = push(&mut nodes, Node::Eventually(group.check_state[c]));
                conjuncts.push(push(&mut nodes, Node::Always(e)));
            }
            for &c in &pem.ae_action {
                let e = push(&mut nodes, Node::Eventually(group.check_action[c]));
                conjuncts.push(push(&mut nodes, Node::Always(e)));
            }
            for &c in &pem.ea_action {
                let a = push(&mut nodes, Node::Always(group.check_action[c]));
                conjuncts.push(push(&mut nodes, Node::Eventually(a)));
            }
            groups.push(push(&mut nodes, Node::And(conjuncts)));
        }
    }
    let root = push(&mut nodes, Node::Or(groups));
    Formula::new(nodes, root).expect("grouped formula")
}
#[test]
fn all_stages_and_grouping_preserve_periodic_word_truth() {
    let mut checked = 0;
    for case in cases() {
        let (fs, ps) = inputs(&case);
        let Ok(p) = project(&fs, &ps, Limits::default()) else {
            continue;
        };
        if p.tagged.is_none() {
            continue;
        }
        let inputs: Vec<_> = fs.iter().chain(&ps).collect();
        for len in 1..=4 {
            for seed in 0..128usize {
                let mut input_states = Vec::new();
                let mut input_actions = Vec::new();
                for (input, f) in inputs.iter().enumerate() {
                    let width = f
                        .nodes()
                        .iter()
                        .filter_map(|n| {
                            if let Node::Atom(a) = n {
                                Some(a.index + 1)
                            } else {
                                None
                            }
                        })
                        .max()
                        .unwrap_or_default();
                    let row = |pos: usize, kind: usize| {
                        (0..width)
                            .map(|i| {
                                // Exhaustive one/two-predicate words are supplemented
                                // by a fixed deterministic mixture for wider controls.
                                let bit = (input * 7 + i * 3 + kind * 5 + pos) % 7;
                                (seed >> bit) & 1 != 0
                            })
                            .collect::<Vec<_>>()
                    };
                    input_states.push((0..len).map(|pos| row(pos, 0)).collect::<Vec<_>>());
                    input_actions.push((0..len).map(|pos| row(pos, 1)).collect::<Vec<_>>());
                }
                let tagged_rows = |action| {
                    (0..len)
                        .map(|pos| {
                            p.occurrences
                                .iter()
                                .map(|o| {
                                    let rows = if o.atom.kind == AtomKind::Action {
                                        &input_actions
                                    } else {
                                        &input_states
                                    };
                                    if (o.atom.kind == AtomKind::Action) == action {
                                        rows[o.input][pos][o.atom.index]
                                    } else {
                                        false
                                    }
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                };
                let states = tagged_rows(false);
                let actions = tagged_rows(true);
                let rewritten = [p.tagged, p.pushed, p.simplified, p.dnf]
                    .into_iter()
                    .map(|root| p.formula(root.expect("root")).expect("stage formula"))
                    .chain([grouped(&p)])
                    .collect::<Vec<_>>();
                for back in 0..len {
                    let values: Vec<_> = inputs
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            f.evaluate_word(&input_states[i], &input_actions[i], back, 100_000)
                                .expect("input truth")
                        })
                        .collect();
                    let expected: Vec<_> = (0..len)
                        .map(|pos| {
                            values[..fs.len()].iter().all(|v| v[pos])
                                && (ps.is_empty() || values[fs.len()..].iter().any(|v| !v[pos]))
                        })
                        .collect();
                    for stage in &rewritten {
                        assert_eq!(
                            stage
                                .evaluate_word(&states, &actions, back, 100_000)
                                .expect("stage truth"),
                            expected,
                            "{} len={len} back={back} seed={seed}",
                            case["name"]
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    println!("independent synthetic stage/group word comparisons: {checked}");
    assert!(checked > 400_000);
}

#[test]
fn resource_failures_are_explicit_not_empty_plans() {
    let p = formula("C(D(s0,s1),D(s2,s3),D(s4,s5),D(s6,s7),D(s8,s9),D(s10,s11))");
    for terms in [0, 1, 20, 100, 200] {
        let limits = Limits {
            formula_nodes: terms,
            ..Limits::default()
        };
        assert!(matches!(
            project(std::slice::from_ref(&p), &[], limits),
            Err(ProjectionError::Formula(FormulaError::Limit(_)))
        ));
    }
    let huge = formula(&format!("C({})", vec!["D(s0,s1)"; 32].join(",")));
    assert!(matches!(
        project(&[huge], &[], Limits::default()),
        Err(ProjectionError::Formula(FormulaError::Limit(
            "TLC DNF cardinality"
        )))
    ));
    let limits = Limits {
        expansion_steps: 1,
        ..Limits::default()
    };
    assert!(matches!(
        project(&[], &[formula("G(s0)")], limits),
        Err(ProjectionError::Formula(FormulaError::Limit(
            "TLC planning work"
        )))
    ));
    let mut nodes = vec![Node::Atom(Atom {
        kind: AtomKind::State,
        index: 0,
    })];
    for _ in 0..258 {
        nodes.push(Node::Always(nodes.len() - 1));
    }
    let root = nodes.len() - 1;
    let deep = Formula::new(nodes, root).expect("deep checked");
    assert!(matches!(
        project(&[], &[deep], Limits::default()),
        Err(ProjectionError::Formula(FormulaError::Limit(
            "TLC planning depth"
        )))
    ));
    assert!(Formula::new(vec![Node::Not(usize::MAX)], 0).is_err());
    assert!(Formula::new(vec![Node::Bool(true)], usize::MAX).is_err());
}

fn bound_model(name: &str) -> Option<dive_model::Model> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-planning/models")
        .join(name);
    let analysis = dive_sema::analyze_file(
        &root.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("analyze file");
    if !analysis.accepted() {
        return None;
    }
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(
        &std::fs::read_to_string(root.join(format!("{name}.cfg"))).expect("config"),
    )
    .into_config()
    .expect("parsed config");
    Some(dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model"))
}
#[test]
fn actual_process_liveness_model_fields_and_retained_property_identity() {
    let record: Value = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-planning/model-capture.json"
    ))
    .expect("models");
    let rows = record["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 12);
    for row in rows {
        let name = row["name"].as_str().expect("name");
        let Some(model) = bound_model(name) else {
            assert_eq!(name, "TpActionBad");
            assert_eq!(row["exit"], 1);
            assert_eq!(row["timeout"], false);
            continue;
        };
        assert_eq!(row["exit"], 0, "{name}");
        assert_eq!(row["timeout"], false);
        let bound = dive_liveness::tlc_plan::lower_model(&model, Limits::default())
            .expect("pure model planning");
        let native = fields(&bound.projection);
        // Action.getName retains configured attribution; the derived temporal
        // vector can instead name a forwarding helper. Compare the role table
        // to that distinct pinned field, never substitute one for the other.
        assert_eq!(
            bound.projection.properties.join(";"),
            row["fields"]["temporal_role_names"]
                .as_str()
                .expect("derived role identities"),
            "{name}"
        );
        assert_eq!(
            row["fields"]["properties"], row["fields"]["configured_properties"],
            "{name}: pinned action/config identity"
        );
        if name == "TpLocal" {
            assert_eq!(row["fields"]["properties"], "Live");
            assert_eq!(row["fields"]["temporal_role_names"], "Identity");
        }
        let configured = bound
            .configured_properties
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>()
            .join(";");
        assert_eq!(
            configured,
            row["fields"]["configured_properties"]
                .as_str()
                .expect("pinned configured identities"),
            "{name}"
        );
        for (key, value) in &native {
            assert_eq!(row["fields"][key], *value, "{name}: {key}");
        }
        // The full pinned processLiveness result independently checks the
        // development projection's group counts, bin ordering and PEM arrays.
        for (key, value) in row["fields"].as_object().expect("fields") {
            if let Some(field) = key.strip_prefix("actual.")
                && !field.ends_with(".promises")
                && !field.ends_with(".tableauNodes")
            {
                assert_eq!(
                    native[field],
                    value.as_str().expect("field"),
                    "{name}: {field}"
                );
            }
        }
        assert_eq!(
            bound.projection.properties,
            model
                .implied_temporals
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn configured_projection_matches_independent_scoped_word_interpreter() {
    let models: Vec<String> = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-planning/models.json"
    ))
    .expect("model names");
    let mut checks = 0;
    for name in models {
        let Some(model) = bound_model(&name) else {
            continue;
        };
        let bound =
            dive_liveness::tlc_plan::lower_model(&model, Limits::default()).expect("projection");
        let p = &bound.projection;
        let formulas = [p.tagged, p.pushed, p.simplified, p.dnf]
            .into_iter()
            .map(|r| p.formula(r.expect("stage")).expect("formula"))
            .chain([grouped(p)])
            .collect::<Vec<_>>();
        for len in 1..=4 {
            for mask in 0usize..(1 << len) {
                let states: Vec<dive_trace::State> = (0..len)
                    .map(|pos| std::sync::Arc::from([Some((mask & (1 << pos) != 0).into())]))
                    .collect();
                for back in 0..len {
                    let truths = model
                        .temporals
                        .iter()
                        .chain(&model.implied_temporals)
                        .map(|n| {
                            dive_trace::temporal::evaluate_lasso(&model, &n.pred, &states, back)
                                .expect("independent temporal truth")
                        })
                        .collect::<Vec<_>>();
                    let expected = (0..len)
                        .map(|pos| {
                            truths[..model.temporals.len()].iter().all(|v| v[pos])
                                && truths[model.temporals.len()..].iter().any(|v| !v[pos])
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
                                .expect("bound atom truth");
                            if o.atom.kind == AtomKind::State {
                                sr[tag] = value;
                            } else {
                                ar[tag] = value;
                            }
                        }
                        state_rows.push(sr);
                        action_rows.push(ar);
                    }
                    for f in &formulas {
                        assert_eq!(
                            f.evaluate_word(&state_rows, &action_rows, back, 100_000)
                                .expect("projection word"),
                            expected,
                            "{name} len={len} mask={mask} back={back}"
                        );
                        checks += 1;
                    }
                }
            }
        }
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
    assert_eq!(checks, 5390);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the negative context/effect matrix keeps all admission boundaries together"
)]
fn model_projection_refuses_effects_opaque_captures_and_unchecked_ids_before_output() {
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
                model.evaluator.scope(p).expect("valid scope"),
            );
            match case {
                "PrintProperty" | "LiveProperty" => {
                    let d = ir.root_def(case).expect("property");
                    pred = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
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
                    let error = model
                        .evaluator
                        .scope_apply(
                            &pred.scope,
                            d,
                            &[dive_ir::Arg::Expr {
                                expr: dive_ir::ExprId(u32::MAX),
                                pass: dive_ir::Pass::Lazy,
                            }],
                        )
                        .expect_err("malformed capture refused before projection");
                    assert!(error.message().contains("DIVE-EVAL-C001"));
                    assert_eq!(model.take_output(), Vec::<String>::new());
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
fn complete_selection_raw_hashes_and_source_matrix_are_verified() {
    let status = std::process::Command::new("python3")
        .arg("-I")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/check-tlc-planning.py"),
        )
        .status()
        .expect("integrity verifier");
    assert!(status.success());
}
