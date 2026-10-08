//! Splitting the next-state relation into actions (ADR 0013 §4).
//!
//! [`split`] builds the tree that TLC's `Tool.getActions` walks
//! (`tlc2/tool/impl/Tool.java:269-419` at tlaplus `94d0c50`). Expanding it
//! into TLC's flat, ordered action list needs values (the domains of
//! existential quantifiers and the arguments of inlined operators), so that
//! happens when the model is bound (`dive-model`, ADR 0013 §8). The tree
//! records every point where TLC's walk branches on a value or an exception,
//! so the expansion is mechanical:
//!
//! | Node | TLC (`getActions`, `getActionsAppl`) | Expansion |
//! |---|---|---|
//! | [`ActionTree::Disj`] | `\/` and bulleted `\/` lists (`OPCODE_lor`, `OPCODE_dl`) | each child's actions, in order |
//! | [`ActionTree::Exists`] | `OPCODE_be`: a bounded `\E`, its domains enumerated in the empty state (`ContextEnumerator`) | one copy of the body's actions per binding, in enumeration order. If a domain cannot be evaluated, the enumeration is empty, or splitting the body throws, the actions added so far are removed and the whole `\E` is one action |
//! | [`ActionTree::Inline`] | a user-defined operator (`opcode == 0`, the lookup gives an `OpDefNode`) whose arguments all have constant level | the arguments are evaluated in the empty state and bound to the parameters, and the body's actions are named after the operator. If an argument fails to evaluate, or splitting the body throws, the application is one action named after the operator; actions the body added before throwing stay |
//! | [`ActionTree::Scope`] | `LET` (its definitions are *not* bound: TLC evaluates them by name), labels, substitution-free `SubstIn` | the body's actions |
//! | [`ActionTree::Leaf`] | everything else, including a `SubstIn` with substitutions and a user operator whose arguments are not all constant | one action |
//! | [`ActionTree::Throws`] | an operator whose lookup is not an `OpDefNode` and which is not one itself (a parameter, bound variable, declaration or theorem name): `(OpDefNode) opNode` throws `ClassCastException`; and expressions that are not boolean at all (`Assert.fail`) | the nearest enclosing `Inline` or `Exists` catches it; at the top it is TLC's error |
//! | [`ActionTree::Configured`] | a constant or constant operator: the lookup gives whatever the configuration binds | inlined like a definition if the configuration binds it to one (`C <- D`), otherwise it throws as above |
//!
//! A definition's subtree depends only on the definition, so [`Split`]
//! holds one per definition and [`ActionTree::Inline`] refers to it by id:
//! a recursive operator gives a finite graph. (TLC itself recurses until the
//! arguments stop being constant, an argument fails, or the stack overflows,
//! which `catch (Throwable)` turns into one action.)
//!
//! **Names and labels.** An action's name is the innermost operator inlined
//! around it, or, for an application that is not inlined, the operator
//! applied. TLC prints an action as `<Name(v1,v2) location>`
//! (`tlc2/tool/Action.java`, `getLocation`), where the values are those of
//! the named definition's parameters bound by inlining, and the location is
//! that of the leaf predicate. An action with no name prints as `Action`.
//! [`label`] formats it. Actions are numbered after the initial predicates
//! (`Tool.java:225-241`).

use crate::expr::{Arg, Expr, JunctionKind, OpRef, QuantKind};
use crate::ids::{DefId, ExprId};
use crate::std_op::StdOp;
use crate::{DefScope, Ir, Level, SlotKind};
use std::collections::BTreeMap;

/// What an action is named after.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionName {
    /// A user definition (TLC's `OpDefNode.getName`, `I!Op` for an
    /// instantiated one).
    Def(DefId),
    /// An operator TLC overrides in Java, applied without being inlined.
    Std(StdOp),
    /// No definition: the next-state relation of a `SPECIFICATION` that is
    /// not an operator application. TLC prints `Action`.
    Unnamed,
}

/// A node of the action tree. See the module documentation for what each
/// means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionTree {
    /// A disjunction: each child's actions, in order.
    Disj {
        /// The disjunction.
        expr: ExprId,
        /// The disjuncts, in order.
        children: Vec<ActionTree>,
    },
    /// A bounded `\E`: one copy of the body per binding of its domains.
    Exists {
        /// The quantifier.
        expr: ExprId,
        /// The name of the single action it becomes when it is not split.
        name: ActionName,
        /// The body.
        body: Box<ActionTree>,
    },
    /// An application of a user definition, inlined if its arguments are
    /// constant and evaluate. The definition's subtree is
    /// [`Split::defs`]`[def]`.
    Inline {
        /// The application (or the use of a zero-arity definition).
        expr: ExprId,
        /// The definition applied.
        def: DefId,
        /// Whether every argument has constant level, so that TLC tries to
        /// inline it. Otherwise the application is one action named `def`.
        constant_args: bool,
    },
    /// A `LET`, label or substitution-free `Subst`: transparent.
    Scope {
        /// The node.
        expr: ExprId,
        /// Its body.
        body: Box<ActionTree>,
    },
    /// One action.
    Leaf {
        /// The action's predicate.
        expr: ExprId,
        /// Its name.
        name: ActionName,
    },
    /// TLC throws here while splitting.
    Throws {
        /// The node.
        expr: ExprId,
    },
    /// A constant or constant operator: depends on the configuration.
    Configured {
        /// The use or application.
        expr: ExprId,
        /// The name of the action if the configuration does not bind it to a
        /// definition and the exception is caught above.
        name: ActionName,
    },
}

/// The action tree of a next-state relation, with the subtree of every
/// definition it may inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The tree of the relation itself.
    pub root: ActionTree,
    /// The tree of each definition's body, named after the definition.
    pub defs: BTreeMap<DefId, ActionTree>,
}

/// Splits `root` the way `Tool.getActions` does. `name` is the definition
/// the relation comes from (`NEXT Next` gives `Next`), or
/// [`ActionName::Unnamed`] for the `A` of a `SPECIFICATION`'s `[][A]_v`.
/// `frame` is the definition whose frame `root` is evaluated in (for `NEXT
/// Next`, `Next` itself). `redirects` are the configuration's `<-`
/// replacements of definitions (`Op <- Other`): TLC's lookup finds the
/// replacement first (`SymbolNodeValueLookupProvider.lookup`), so a use of
/// `Op` inlines `Other`, under `Other`'s name.
#[must_use]
pub fn split(
    ir: &Ir,
    root: ExprId,
    name: ActionName,
    frame: DefId,
    redirects: &BTreeMap<DefId, DefId>,
) -> Split {
    let mut s = Splitter {
        ir,
        defs: BTreeMap::new(),
        pending: Vec::new(),
        redirects,
    };
    let root = s.tree(root, name, frame);
    while let Some(d) = s.pending.pop() {
        if s.defs.contains_key(&d) {
            continue;
        }
        let def = ir.def(d);
        // A zero-arity LET value is evaluated in its parent's frame.
        let frame = match def.scope {
            DefScope::LetValue { parent, .. } => parent,
            _ => d,
        };
        let t = s.tree(def.body, ActionName::Def(d), frame);
        s.defs.insert(d, t);
    }
    Split { root, defs: s.defs }
}

struct Splitter<'a> {
    ir: &'a Ir,
    defs: BTreeMap<DefId, ActionTree>,
    pending: Vec<DefId>,
    redirects: &'a BTreeMap<DefId, DefId>,
}

impl Splitter<'_> {
    fn inline(&mut self, expr: ExprId, def: DefId, constant_args: bool) -> ActionTree {
        if !self.defs.contains_key(&def) {
            self.pending.push(def);
        }
        ActionTree::Inline {
            expr,
            def,
            constant_args,
        }
    }

    fn arg_level(&self, a: &Arg) -> Level {
        match a {
            Arg::Expr { expr, .. } => self.ir.node(*expr).level,
            Arg::Op { level, .. } => *level,
        }
    }

    fn tree(&mut self, e: ExprId, name: ActionName, frame: DefId) -> ActionTree {
        let ir = self.ir;
        match ir.expr(e) {
            Expr::Junction {
                kind: JunctionKind::Or,
                items,
                ..
            } => ActionTree::Disj {
                expr: e,
                children: items.iter().map(|&i| self.tree(i, name, frame)).collect(),
            },
            Expr::Quant {
                kind: QuantKind::Exists,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => ActionTree::Exists {
                expr: e,
                name,
                body: Box::new(self.tree(*body, name, frame)),
            },
            Expr::Let { body, .. } | Expr::Label { body, .. } => ActionTree::Scope {
                expr: e,
                body: Box::new(self.tree(*body, name, frame)),
            },
            Expr::Subst { bindings, body } if bindings.is_empty() => ActionTree::Scope {
                expr: e,
                body: Box::new(self.tree(*body, name, frame)),
            },
            Expr::Def(d) => self.use_of(e, *d, true),
            Expr::Apply { callee, args } => match callee {
                OpRef::Def(d) => {
                    let constant = args.iter().all(|a| self.arg_level(a) == Level::Constant);
                    self.use_of(e, *d, constant)
                }
                OpRef::Std(op) => ActionTree::Leaf {
                    expr: e,
                    name: ActionName::Std(*op),
                },
                OpRef::Const(_) | OpRef::Subst(_) => ActionTree::Configured { expr: e, name },
                OpRef::Param(_) | OpRef::Builtin(_) | OpRef::Unbound(_) => {
                    ActionTree::Throws { expr: e }
                }
            },
            Expr::Local(r) => match self.let_value(*r, frame) {
                Some(d) => self.inline(e, d, true),
                None => ActionTree::Throws { expr: e },
            },
            Expr::Const(_) | Expr::SubstRef { .. } => ActionTree::Configured { expr: e, name },
            Expr::Var { .. }
            | Expr::Unbound(_)
            | Expr::Num(_)
            | Expr::Str(_)
            | Expr::Decimal { .. }
            | Expr::ExceptAt
            | Expr::Unevaluable(_) => ActionTree::Throws { expr: e },
            _ => ActionTree::Leaf { expr: e, name },
        }
    }

    /// A use of a definition: inlined like TLC's `OpDefNode` case, unless
    /// TLC overrides it in Java (the lookup then gives the override, and the
    /// application is one action named after it).
    fn use_of(&mut self, e: ExprId, d: DefId, constant: bool) -> ActionTree {
        if let Some(&r) = self.redirects.get(&d) {
            return self.inline(e, r, constant);
        }
        match self.ir.def(d).std {
            Some(op) => ActionTree::Leaf {
                expr: e,
                name: ActionName::Std(op),
            },
            None => self.inline(e, d, constant),
        }
    }

    /// The `LET` definition a local reference names, if it is a zero-arity
    /// `LET` value. TLC does not bind `LET` definitions while splitting, so
    /// its lookup finds the `OpDefNode` and inlines it.
    fn let_value(&self, r: crate::LocalRef, frame: DefId) -> Option<DefId> {
        let mut target = frame;
        for _ in 0..r.up {
            target = match self.ir.def(target).scope {
                DefScope::Local { parent } | DefScope::Inline { parent } => parent,
                DefScope::TopLevel | DefScope::LetValue { .. } => return None,
            };
        }
        match self
            .ir
            .def(target)
            .frame
            .get(r.slot.index())
            .map(|s| &s.kind)
        {
            Some(SlotKind::LetValue(d)) => Some(*d),
            _ => None,
        }
    }
}

/// Formats TLC's label for an action: `<Name(v1,v2) location>`
/// (`Action.getLocation`), with `values` the printed values of the named
/// definition's parameters bound by inlining, in parameter order.
#[must_use]
pub fn label(ir: &Ir, name: ActionName, values: &[String], leaf: ExprId) -> String {
    let n = match name {
        ActionName::Def(d) => ir.def(d).name.clone(),
        ActionName::Std(op) => op.name().to_owned(),
        ActionName::Unnamed => "Action".to_owned(),
    };
    let params = if values.is_empty() {
        String::new()
    } else {
        format!("({})", values.join(","))
    };
    format!("<{n}{params} {}>", ir.location(&ir.node(leaf).origin))
}

/// The name TLC gives an action, or `None` for an unnamed one.
#[must_use]
pub fn name_text(ir: &Ir, name: ActionName) -> Option<String> {
    match name {
        ActionName::Def(d) => Some(ir.def(d).name.clone()),
        ActionName::Std(op) => Some(op.name().to_owned()),
        ActionName::Unnamed => None,
    }
}

/// How an action changes one variable, in the [`ActionShape`] view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// `x' = e`.
    Assign(ExprId),
    /// `x' \in S`.
    In(ExprId),
    /// `x' \subseteq S`.
    Subseteq(ExprId),
    /// `UNCHANGED x`, directly or in a tuple.
    Unchanged,
}

/// An action as guards and per-variable updates (ADR 0013 §4), for export
/// (the Verus exporter) and diagnostics. Checking never uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionShape {
    /// A conjunction of guards and updates.
    Conjunction {
        /// Conjuncts of at most state level, in order.
        guards: Vec<ExprId>,
        /// Single-variable updates, by variable. A variable updated twice
        /// keeps its first update; the other is a residual.
        updates: BTreeMap<crate::VarId, Update>,
        /// Action-level conjuncts that are not single-variable updates.
        residual: Vec<ExprId>,
    },
    /// Not a conjunction of guards and updates: the action is itself a
    /// disjunction, `IF`, `CASE` or quantifier.
    Opaque(&'static str),
}

/// The shape of the action whose predicate is `leaf`. Conjunctions are
/// flattened through labels, `LET`s and uses of zero-arity definitions.
#[must_use]
pub fn shape(ir: &Ir, leaf: ExprId) -> ActionShape {
    let mut conjuncts = Vec::new();
    flatten(ir, leaf, &mut conjuncts, 0);
    if let [only] = conjuncts.as_slice() {
        match ir.expr(*only) {
            Expr::Junction {
                kind: JunctionKind::Or,
                ..
            } => return ActionShape::Opaque("a disjunction"),
            Expr::If { .. } => return ActionShape::Opaque("an IF"),
            Expr::Case { .. } => return ActionShape::Opaque("a CASE"),
            Expr::Quant { .. } if ir.node(*only).level > Level::State => {
                return ActionShape::Opaque("a quantified action");
            }
            _ => {}
        }
    }
    let mut guards = Vec::new();
    let mut updates = BTreeMap::new();
    let mut residual = Vec::new();
    for c in conjuncts {
        let node = ir.node(c);
        if node.level <= Level::State {
            guards.push(c);
        } else if !update(ir, &node.expr, &mut updates) {
            residual.push(c);
        }
    }
    ActionShape::Conjunction {
        guards,
        updates,
        residual,
    }
}

/// Records `expr` as an update if it is one; whether it was.
fn update(ir: &Ir, expr: &Expr, updates: &mut BTreeMap<crate::VarId, Update>) -> bool {
    match expr {
        Expr::Builtin { op, args } if args.len() == 2 => {
            let Expr::Var { var, primed: true } = ir.expr(args[0]) else {
                return false;
            };
            let u = match op {
                crate::BuiltinOp::Eq => Update::Assign(args[1]),
                crate::BuiltinOp::In => Update::In(args[1]),
                crate::BuiltinOp::Subseteq => Update::Subseteq(args[1]),
                _ => return false,
            };
            if updates.contains_key(var) {
                return false;
            }
            updates.insert(*var, u);
            true
        }
        Expr::Unchanged(e) => {
            let vars: Option<Vec<crate::VarId>> = match ir.expr(*e) {
                Expr::Var { var, primed: false } => Some(vec![*var]),
                Expr::Tuple(items) => items
                    .iter()
                    .map(|&i| match ir.expr(i) {
                        Expr::Var { var, primed: false } => Some(*var),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            };
            match vars {
                Some(vs) if vs.iter().all(|v| !updates.contains_key(v)) => {
                    for v in vs {
                        updates.insert(v, Update::Unchanged);
                    }
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn flatten(ir: &Ir, e: ExprId, out: &mut Vec<ExprId>, depth: usize) {
    if depth > 64 {
        out.push(e);
        return;
    }
    match ir.expr(e) {
        Expr::Junction {
            kind: JunctionKind::And,
            items,
            ..
        } => {
            for &i in items {
                flatten(ir, i, out, depth + 1);
            }
        }
        Expr::Label { body, .. } | Expr::Let { body, .. } => flatten(ir, *body, out, depth + 1),
        Expr::Def(d) if ir.def(*d).std.is_none() && ir.def(*d).params.is_empty() => {
            flatten(ir, ir.def(*d).body, out, depth + 1);
        }
        _ => out.push(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dive_sema::{Options, analyze_source};

    fn ir(src: &str) -> Option<Ir> {
        crate::lower(&analyze_source("M.tla", src, &Options::default())).ok()
    }

    #[test]
    fn shapes() {
        let Some(ir) = ir("---- MODULE M ----
EXTENDS Naturals
VARIABLES x, y, z
G == x > 0
A == G /\\ x' = x - 1 /\\ y' \\in {1, 2} /\\ UNCHANGED <<z>>
B == x' = 1 \\/ x' = 2
C == x' = 1 /\\ x' = 2
====
")
        else {
            unreachable!("lowering failed");
        };
        let body = |n: &str| ir.def(ir.root_def(n).unwrap_or(DefId(0))).body;
        let ActionShape::Conjunction {
            guards,
            updates,
            residual,
        } = shape(&ir, body("A"))
        else {
            unreachable!("A is a conjunction");
        };
        assert_eq!(guards.len(), 1);
        assert_eq!(updates.len(), 3);
        assert_eq!(residual, Vec::<ExprId>::new());
        assert!(matches!(
            updates.get(&crate::VarId(2)),
            Some(Update::Unchanged)
        ));
        assert!(matches!(shape(&ir, body("B")), ActionShape::Opaque(_)));
        let ActionShape::Conjunction { residual, .. } = shape(&ir, body("C")) else {
            unreachable!("C is a conjunction");
        };
        assert_eq!(residual.len(), 1, "the second update of x is a residual");
    }

    #[test]
    fn split_follows_tlc_rules() {
        let Some(ir) = ir("---- MODULE M ----
EXTENDS Naturals
VARIABLE x
A == x' = 1
B(i) == x' = i
Next == A \\/ \\E i \\in {1, 2} : B(i) \\/ B(x) \\/ (x' = 3 /\\ x = 0)
====
")
        else {
            unreachable!("lowering failed");
        };
        let next = ir.root_def("Next").unwrap_or(DefId(0));
        let s = split(
            &ir,
            ir.def(next).body,
            ActionName::Def(next),
            next,
            &BTreeMap::new(),
        );
        let ActionTree::Disj { children, .. } = &s.root else {
            unreachable!("Next is a disjunction");
        };
        // Binary \/ nests: ((A \/ \E) \/ B(x)) \/ (...).
        assert_eq!(children.len(), 2);
        let text = format!("{s:?}");
        assert!(text.contains("Exists"), "{text}");
        assert!(
            text.contains("constant_args: false"),
            "B(x) is not inlined: {text}"
        );
        assert!(text.contains("constant_args: true"), "{text}");
        assert!(s.defs.contains_key(&ir.root_def("A").unwrap_or(DefId(0))));
    }
}
