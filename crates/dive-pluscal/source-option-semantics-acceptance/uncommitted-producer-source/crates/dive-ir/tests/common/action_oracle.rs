//! Compares dive's action tree with TLC's action list, as recorded by
//! `acceptance/ActionDump.java` (TLC's own `Tool.getActions()`).
//!
//! Expanding the tree needs values: how many bindings each split `\E` has,
//! whether an inlined operator's arguments evaluate. Without an evaluator,
//! the comparison lets TLC's list decide those choices and checks that some
//! consistent choice reproduces the list *exactly*: every action's name,
//! parameters and label (`<Name(v1,v2) line L, col C to line L2, col C2 of
//! module M>`), in order, and the count. A disjunct, inlining or split dive
//! got wrong cannot be matched, because each choice point only allows TLC's
//! alternatives (`Tool.getActionsAppl`).

#![allow(dead_code, reason = "shared by tests and examples, each using a part")]

use dive_ir::actions::{ActionName, ActionTree, Split, label, name_text, split};
use dive_ir::{DefId, Expr, Ir};
use serde_json::Value;
use std::collections::BTreeSet;

/// One action TLC reports.
#[derive(Debug, Clone)]
pub struct TlcAction {
    pub name: Option<String>,
    pub label: String,
    pub location: String,
    pub params: Vec<String>,
    pub context: Vec<(String, String)>,
}

/// One model's dump.
#[derive(Debug, Clone)]
pub struct TlcModel {
    pub id: String,
    pub module: String,
    pub next: Option<(Option<String>, String, String)>,
    pub overrides: Vec<(String, String)>,
    pub actions: Vec<TlcAction>,
    pub error: Option<String>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

fn action(v: &Value) -> TlcAction {
    TlcAction {
        name: v["name"].as_str().map(str::to_owned),
        label: s(&v["label"]),
        location: s(&v["location"]["text"]),
        params: v["params"]
            .as_array()
            .into_iter()
            .flatten()
            .map(s)
            .collect(),
        context: v["context"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| (s(&p[0]), s(&p[1])))
            .collect(),
    }
}

/// Parses one line of `ActionDump`'s output.
pub fn parse(line: &str) -> Option<TlcModel> {
    let v: Value = serde_json::from_str(line).ok()?;
    let next = if v["next"].is_null() {
        None
    } else {
        Some((
            v["next"]["name"].as_str().map(str::to_owned),
            s(&v["next"]["kind"]),
            s(&v["next"]["location"]["text"]),
        ))
    };
    Some(TlcModel {
        id: s(&v["id"]),
        module: s(&v["module"]),
        next,
        overrides: v["overrides"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), s(v)))
            .collect(),
        actions: v["actions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(action)
            .collect(),
        error: v["error"].as_str().map(str::to_owned),
    })
}

/// The result of a comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// dive's tree reproduces TLC's list: this many actions.
    Match(usize),
    /// TLC's next-state relation is a substitution wrapper it builds for an
    /// instantiated `SPECIFICATION` (`Specs.addSubsts`), which `getActions`
    /// makes one unnamed action. dive's IR has no node for the wrapper (the
    /// model binding, WP-B1, builds it), so the check is that a substitution
    /// with bindings at TLC's location (the `INSTANCE` that made it) reaches
    /// a `[][A]_v`, and that TLC's one label is `<Action location>`.
    SubstitutedSpec(usize),
    /// They differ.
    Mismatch(String),
}

/// Compares the model's action list with dive's tree for the same
/// next-state relation.
pub fn compare(ir: &Ir, model: &TlcModel) -> Outcome {
    if let Some(e) = &model.error {
        return Outcome::Mismatch(format!("TLC failed: {e}"));
    }
    let Some((next_name, kind, location)) = &model.next else {
        return if model.actions.is_empty() {
            Outcome::Match(0)
        } else {
            Outcome::Mismatch("TLC has actions but no next-state relation".to_owned())
        };
    };
    let (root, name, frame) = if let Some(n) = next_name {
        let Some(d) = ir.root_def(n) else {
            return Outcome::Mismatch(format!("no definition {n} in the root module"));
        };
        (ir.def(d).body, ActionName::Def(d), d)
    } else if kind == "SubstInNode" {
        return substituted(ir, model, location);
    } else {
        // The `A` of `[][A]_v`: the node at TLC's location that is the
        // action of a `[]_` step.
        let frames = ir.expr_frames();
        let mut found = None;
        for (i, n) in ir.nodes().iter().enumerate() {
            if let Expr::StepBox { action, .. } = &n.expr
                && ir.location(&ir.node(*action).origin) == *location
                && let Some(Some(f)) = frames.get(i)
            {
                found = Some((*action, *f));
                break;
            }
        }
        let Some((a, f)) = found else {
            return Outcome::Mismatch(format!("no [][A]_v with A at {location}"));
        };
        (a, ActionName::Unnamed, f)
    };
    let mut redirects = std::collections::BTreeMap::new();
    for (from, to) in &model.overrides {
        if let (Some(f), Some(t)) = (ir.root_def(from), ir.root_def(to)) {
            redirects.insert(f, t);
        }
    }
    let sp = split(ir, root, name, frame, &redirects);
    let m = Matcher {
        ir,
        split: &sp,
        tlc: &model.actions,
    };
    let (ok, _) = m.ends(&sp.root, &mut Vec::new(), 0, 0);
    if ok.contains(&model.actions.len()) {
        Outcome::Match(model.actions.len())
    } else {
        Outcome::Mismatch(m.explain(&sp))
    }
}

fn substituted(ir: &Ir, model: &TlcModel, location: &str) -> Outcome {
    let [only] = model.actions.as_slice() else {
        return Outcome::Mismatch(format!(
            "{} actions for a substituted next-state relation",
            model.actions.len()
        ));
    };
    if only.name.is_some() || only.label != format!("<Action {location}>") {
        return Outcome::Mismatch(format!("label {}", only.label));
    }
    // TLC's wrapper (`Specs.addSubsts`) takes the location of the
    // substitution it applies, which is the `INSTANCE` that made it: some
    // substitution with bindings at that location must reach a `[][A]_v`,
    // through uses of definitions.
    let ok = ir.nodes().iter().any(|n| {
        matches!(&n.expr, Expr::Subst { bindings, body }
            if !bindings.is_empty()
                && ir.location(&n.origin) == location
                && reaches_step(ir, *body))
    });
    if ok {
        Outcome::SubstitutedSpec(1)
    } else {
        Outcome::Mismatch(format!("no substitution at {location} reaching a [][A]_v"))
    }
}

fn reaches_step(ir: &Ir, from: dive_ir::ExprId) -> bool {
    let mut work = vec![from];
    let mut seen = BTreeSet::new();
    let mut defs_seen = BTreeSet::new();
    while let Some(e) = work.pop() {
        if !seen.insert(e) {
            continue;
        }
        let expr = ir.expr(e);
        if matches!(expr, Expr::StepBox { .. }) {
            return true;
        }
        work.extend(expr.children());
        let mut defs: Vec<DefId> = Vec::new();
        if let Expr::Def(d) = expr {
            defs.push(*d);
        }
        for r in expr.op_refs() {
            if let dive_ir::OpRef::Def(d) = r {
                defs.push(*d);
            }
        }
        for d in defs {
            if defs_seen.insert(d) {
                work.push(ir.def(d).body);
            }
        }
    }
    false
}

struct Matcher<'a> {
    ir: &'a Ir,
    split: &'a Split,
    tlc: &'a [TlcAction],
}

type Ends = (BTreeSet<usize>, BTreeSet<usize>);

impl Matcher<'_> {
    /// Whether TLC's action `i` is the leaf `expr` named `name`, with the
    /// parameters inlining bound.
    fn leaf(&self, expr: dive_ir::ExprId, name: ActionName, stack: &[DefId], i: usize) -> bool {
        let Some(t) = self.tlc.get(i) else {
            return false;
        };
        if name_text(self.ir, name) != t.name {
            return false;
        }
        let params: Vec<String> = match name {
            ActionName::Def(d) if stack.contains(&d) => self
                .ir
                .def(d)
                .params
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            _ => Vec::new(),
        };
        if params != t.params {
            return false;
        }
        let values: Vec<String> = params
            .iter()
            .map(|p| {
                t.context
                    .iter()
                    .find(|(n, _)| n == p)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            })
            .collect();
        label(self.ir, name, &values, expr) == t.label
    }

    fn ends(&self, t: &ActionTree, stack: &mut Vec<DefId>, i: usize, depth: usize) -> Ends {
        let mut ok = BTreeSet::new();
        let mut crashed = BTreeSet::new();
        if depth > 200 {
            return (ok, crashed);
        }
        match t {
            ActionTree::Leaf { expr, name } => {
                if self.leaf(*expr, *name, stack, i) {
                    ok.insert(i + 1);
                }
            }
            ActionTree::Throws { .. } => {
                crashed.insert(i);
            }
            ActionTree::Configured { .. } => {}
            ActionTree::Scope { body, .. } => return self.ends(body, stack, i, depth + 1),
            ActionTree::Disj { children, .. } => {
                let mut at: BTreeSet<usize> = BTreeSet::from([i]);
                for c in children {
                    let mut next = BTreeSet::new();
                    for &p in &at {
                        let (o, cr) = self.ends(c, stack, p, depth + 1);
                        next.extend(o);
                        crashed.extend(cr);
                    }
                    at = next;
                }
                ok = at;
            }
            ActionTree::Exists { expr, name, body } => {
                if self.leaf(*expr, *name, stack, i) {
                    ok.insert(i + 1);
                }
                let mut frontier = BTreeSet::from([i]);
                let mut seen = BTreeSet::new();
                while !frontier.is_empty() {
                    let mut next = BTreeSet::new();
                    for &p in &frontier {
                        let (o, _) = self.ends(body, stack, p, depth + 1);
                        for e in o {
                            if e > p && seen.insert(e) {
                                next.insert(e);
                            }
                        }
                    }
                    ok.extend(next.iter().copied());
                    frontier = next;
                }
            }
            ActionTree::Inline {
                expr,
                def,
                constant_args,
            } => {
                let name = ActionName::Def(*def);
                if self.leaf(*expr, name, stack, i) {
                    ok.insert(i + 1);
                }
                if *constant_args && let Some(body) = self.split.defs.get(def) {
                    stack.push(*def);
                    let (o, cr) = self.ends(body, stack, i, depth + 1);
                    stack.pop();
                    ok.extend(o);
                    for c in cr {
                        if self.leaf(*expr, name, stack, c) {
                            ok.insert(c + 1);
                        }
                    }
                }
            }
        }
        (ok, crashed)
    }

    /// The leaves dive's tree has, once each, against TLC's distinct
    /// labels, for a mismatch report.
    fn explain(&self, sp: &Split) -> String {
        let mut mine = Vec::new();
        let mut seen = BTreeSet::new();
        self.leaves(&sp.root, &mut mine, &mut seen, 0);
        let mut theirs: Vec<String> = Vec::new();
        for t in self.tlc {
            let k = format!("{:?} {}", t.name, t.location);
            if !theirs.contains(&k) {
                theirs.push(k);
            }
        }
        format!("dive leaves {mine:?}\nTLC actions (distinct) {theirs:?}")
    }

    fn leaves(
        &self,
        t: &ActionTree,
        out: &mut Vec<String>,
        seen: &mut BTreeSet<DefId>,
        depth: usize,
    ) {
        if depth > 50 {
            return;
        }
        match t {
            ActionTree::Leaf { expr, name } => out.push(format!(
                "{:?} {}",
                name_text(self.ir, *name),
                self.ir.location(&self.ir.node(*expr).origin)
            )),
            ActionTree::Throws { expr } => out.push(format!(
                "throws at {}",
                self.ir.location(&self.ir.node(*expr).origin)
            )),
            ActionTree::Configured { expr, .. } => out.push(format!(
                "configured at {}",
                self.ir.location(&self.ir.node(*expr).origin)
            )),
            ActionTree::Scope { body, .. } | ActionTree::Exists { body, .. } => {
                self.leaves(body, out, seen, depth + 1);
            }
            ActionTree::Disj { children, .. } => {
                for c in children {
                    self.leaves(c, out, seen, depth + 1);
                }
            }
            ActionTree::Inline {
                def,
                constant_args,
                expr,
            } => {
                if *constant_args && seen.insert(*def) {
                    if let Some(b) = self.split.defs.get(def) {
                        self.leaves(b, out, seen, depth + 1);
                    }
                } else if !*constant_args {
                    out.push(format!(
                        "{:?} {}",
                        name_text(self.ir, ActionName::Def(*def)),
                        self.ir.location(&self.ir.node(*expr).origin)
                    ));
                }
            }
        }
    }
}
