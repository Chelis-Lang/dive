//! Pinned TLC action-context logical invocation costs for the admitted Reference slice.
use crate::next::Action;
use crate::{SourceCoverage, SourceLocation};
use dive_ir::{Arg, Expr, ExprId, Ir, OpRef, Origin};
use dive_value::{Set, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

/// One collapsed TLC logical-cost row, in its action's calling tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcCoverageRow {
    /// Original source position, independent of internal node numbering.
    pub location: SourceLocation,
    /// Tree depth; zero denotes a direct child of the action.
    pub depth: usize,
    /// Logical invocations, including failing demands.
    pub invocations: u64,
    /// Logical enumeration work. Zero is omitted from TLC text rendering.
    pub cost: u64,
}
/// One Init or split Next action, with a separate logical calling tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcActionCoverage {
    /// TLC's declaration-based label (without bound parameter values).
    pub label: String,
    /// Whether this is the initial predicate.
    pub initial: bool,
    /// Init emissions, or newly discovered Next state identities.
    pub distinct: u64,
    /// Init candidates, or Next emissions, including duplicates.
    pub generated: u64,
    /// Collapsed logical expression tree, following pinned TLC reporting rules.
    pub rows: Vec<TlcCoverageRow>,
}
/// A stopped or live logical coverage snapshot; never a model proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcCoverageSnapshot {
    /// Initial action followed by split Next actions, in producer order.
    pub actions: Vec<TlcActionCoverage>,
    /// A counter saturated; exact counts are unavailable.
    pub overflowed: bool,
    /// Whether producer collection is still enabled.
    pub active: bool,
    /// A named unsupported coverage feature. When present, no costs are claimed.
    pub unsupported: Option<String>,
}
#[derive(Debug)]
struct Node {
    expr: ExprId,
    location: SourceLocation,
    parent: Option<usize>,
    children: Vec<usize>,
    count: u64,
    cost: u64,
    primed: bool,
}
#[derive(Debug)]
struct Tree {
    label: String,
    initial: bool,
    distinct: u64,
    generated: u64,
    nodes: Vec<Node>,
    by_expr: BTreeMap<ExprId, Vec<usize>>,
}
#[derive(Default)]
pub(crate) struct Logical {
    trees: BTreeMap<usize, Tree>,
    aliases: BTreeMap<usize, usize>,
    current: Option<(usize, Option<usize>)>,
    thread: Option<std::thread::ThreadId>,
    overflowed: bool,
    unsupported: Option<String>,
    sets: Vec<(Set, usize, usize)>,
}
impl std::fmt::Debug for Logical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logical")
            .field("actions", &self.trees.len())
            .field("set_sources", &self.sets.len())
            .finish_non_exhaustive()
    }
}
pub(crate) type Collector = Mutex<Logical>;
/// Restores observation context without touching any evaluator or thunk.
pub(crate) struct LogicalGuard {
    owner: Arc<SourceCoverage>,
    previous: Option<(usize, Option<usize>)>,
    not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for LogicalGuard {
    fn drop(&mut self) {
        if let Some(logical) = &self.owner.logical {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .current = self.previous;
        }
    }
}
fn location(ir: &Ir, origin: Origin) -> Result<SourceLocation, String> {
    let file = origin
        .file
        .and_then(|id| ir.files().get(id.index()))
        .ok_or("TLC coverage requires an original source location")?;
    Ok(SourceLocation {
        module: file.module.clone(),
        file: file.display.clone(),
        bytes: origin.range,
        begin: origin.begin,
        end: origin.end,
        instance: None,
    })
}
fn add(ir: &Ir, expr: ExprId, tree: &mut Tree, depth: usize) -> Result<Option<usize>, String> {
    if depth > 256 || tree.nodes.len() >= 100_000 {
        return Err("TLC coverage calling-tree capacity".into());
    }
    let node = ir.node(expr);
    let mut children = Vec::new();
    if matches!(
        node.expr,
        Expr::Builtin {
            op: dive_ir::BuiltinOp::Cup
                | dive_ir::BuiltinOp::Cap
                | dive_ir::BuiltinOp::Subseteq
                | dive_ir::BuiltinOp::BigUnion
                | dive_ir::BuiltinOp::Domain
                | dive_ir::BuiltinOp::Times,
            ..
        } | Expr::Tuple(_)
    ) {
        return Err("TLC coverage compound construction/materialization costs outside the scalar/quantifier slice".into());
    }
    match &node.expr {
        Expr::Num(_) | Expr::Decimal { .. } | Expr::Str(_) => return Ok(None),
        Expr::Bool(_) | Expr::Var { .. } | Expr::Local(_) | Expr::Const(_) => {}
        Expr::Def(def) => {
            let def = ir.def(*def);
            if def.recursive || def.origin.instance.is_some() {
                return Err("TLC coverage recursive or instantiated definitions".into());
            }
            children.push(def.body);
        }
        Expr::Apply { callee, args } => {
            match callee {
                OpRef::Def(id) => {
                    let def = ir.def(*id);
                    if def.recursive || def.origin.instance.is_some() {
                        return Err("TLC coverage recursive or instantiated definitions".into());
                    }
                    children.push(def.body);
                }
                OpRef::Std(op)
                    if matches!(op.module(), "Naturals" | "Integers" | "FiniteSets")
                        || (op.module() == "TLC"
                            && matches!(op.name(), "Print" | "PrintT" | "Assert")) => {}
                _ => return Err("TLC coverage this native or higher-order operator".into()),
            }
            for arg in args {
                match arg {
                    Arg::Expr { expr, .. } => children.push(*expr),
                    Arg::Op { .. } => return Err("TLC coverage operator arguments".into()),
                }
            }
        }
        Expr::Quant { bounds, body, .. } => {
            children.push(*body);
            for bound in bounds {
                children.push(bound.domain.ok_or("TLC coverage unbounded quantifiers")?);
            }
        }
        Expr::Builtin { .. }
        | Expr::Junction { .. }
        | Expr::Not(_)
        | Expr::Implies(..)
        | Expr::Equiv(..)
        | Expr::If { .. }
        | Expr::SetEnum(_)
        | Expr::Tuple(_) => node.expr.for_each_child(|child| children.push(child)),
        _ => {
            return Err(format!(
                "TLC coverage expression outside the finite scalar/quantifier slice at {}",
                ir.location(&node.origin)
            ));
        }
    }
    let index = tree.nodes.len();
    tree.by_expr.entry(expr).or_default().push(index);
    tree.nodes.push(Node {
        expr,
        location: location(ir, node.origin)?,
        parent: None,
        children: Vec::new(),
        count: 0,
        cost: 0,
        primed: matches!(&node.expr, Expr::Builtin { op: dive_ir::BuiltinOp::Eq | dive_ir::BuiltinOp::In, args } if args.first().is_some_and(|left| matches!(ir.expr(*left),Expr::Var {primed:true,..}))),
    });
    for child in children {
        if let Some(child) = add(ir, child, tree, depth + 1)? {
            tree.nodes[child].parent = Some(index);
            tree.nodes[index].children.push(child);
        }
    }
    Ok(Some(index))
}
impl Logical {
    pub(crate) fn prepare(
        &mut self,
        ir: &Ir,
        inits: &[Action],
        actions: &[Action],
    ) -> Result<(), String> {
        if ir.vars().is_empty() {
            return Err("TLC coverage currently requires at least one state variable".into());
        }
        if inits.len() != 1 {
            return Err("TLC coverage currently requires one initial predicate".into());
        }
        let mut predicates = BTreeMap::new();
        for (initial, action) in inits
            .iter()
            .map(|a| (true, a))
            .chain(actions.iter().map(|a| (false, a)))
        {
            if let Some(id) = predicates.get(&action.expr()) {
                self.aliases.insert(action.id(), *id);
                continue;
            }
            predicates.insert(action.expr(), action.id());
            self.aliases.insert(action.id(), action.id());
            let label = match action.name() {
                dive_ir::actions::ActionName::Def(id) => {
                    let definition = ir.def(id);
                    let header = ir
                        .definition_declaration_origin(id)
                        .ok_or("TLC coverage action declaration has no source header")?;
                    let actual = ir.node(action.expr()).origin;
                    let detail = if actual == ir.node(definition.body).origin {
                        String::new()
                    } else {
                        format!(
                            " ({} {} {} {})",
                            actual.begin.0, actual.begin.1, actual.end.0, actual.end.1
                        )
                    };
                    format!("<{} {}{detail}>", definition.name, ir.location(&header))
                }
                _ => return Err("TLC coverage unnamed actions".into()),
            };
            let mut tree = Tree {
                label,
                initial,
                distinct: 0,
                generated: 0,
                nodes: Vec::new(),
                by_expr: BTreeMap::new(),
            };
            add(ir, action.expr(), &mut tree, 0)?;
            self.trees.insert(action.id(), tree);
        }
        Ok(())
    }
    pub(crate) fn record(&mut self, _expr: ExprId, kind: usize) {
        let Some((action, current)) = self.current else {
            return;
        };
        let Some(tree) = self.trees.get_mut(&action) else {
            return;
        };
        if kind == 3 || kind == 4 {
            increment(&mut tree.generated, &mut self.overflowed);
            if tree.initial {
                increment(&mut tree.distinct, &mut self.overflowed);
            }
            if kind == 3
                && let Some(index) = current
            {
                increment(&mut tree.nodes[index].count, &mut self.overflowed);
            }
        }
    }
    pub(crate) fn discovered(&mut self, action: usize) {
        if let Some(action) = self.aliases.get(&action)
            && let Some(tree) = self.trees.get_mut(action)
            && !tree.initial
        {
            increment(&mut tree.distinct, &mut self.overflowed);
        }
    }
    pub(crate) fn continued(&mut self) {
        if let Some((action, Some(index))) = self.current
            && let Some(tree) = self.trees.get_mut(&action)
        {
            increment(&mut tree.nodes[index].count, &mut self.overflowed);
        }
    }
    pub(crate) fn rejected_init(&mut self) {
        if let Some((action, _)) = self.current
            && let Some(tree) = self.trees.get_mut(&action)
        {
            increment(&mut tree.generated, &mut self.overflowed);
        }
    }
    fn locate(&self, action: usize, current: Option<usize>, expr: ExprId) -> Option<usize> {
        let tree = self.trees.get(&action)?;
        let mut parent = current;
        while let Some(index) = parent {
            let node = &tree.nodes[index];
            if node.expr == expr {
                return Some(index);
            }
            if let Some(child) = node
                .children
                .iter()
                .find(|index| tree.nodes[**index].expr == expr)
            {
                return Some(*child);
            }
            parent = node.parent;
        }
        let matches = tree.by_expr.get(&expr)?;
        (matches.len() == 1).then(|| matches[0])
    }
    fn enumeration(&mut self, set: &Set) {
        if let Some((_, action, index)) = self.sets.iter().find(|(owner, _, _)| owner.ptr_eq(set))
            && let Some(tree) = self.trees.get_mut(action)
        {
            increment(&mut tree.nodes[*index].cost, &mut self.overflowed);
        }
    }
    pub(crate) fn snapshot(&self, active: bool) -> TlcCoverageSnapshot {
        if self.overflowed {
            return TlcCoverageSnapshot {
                active,
                overflowed: true,
                unsupported: Some("TLC logical coverage counter saturation".into()),
                actions: Vec::new(),
            };
        }
        if let Some(feature) = &self.unsupported {
            return TlcCoverageSnapshot {
                active,
                overflowed: self.overflowed,
                unsupported: Some(feature.clone()),
                actions: Vec::new(),
            };
        }
        TlcCoverageSnapshot {
            active,
            overflowed: self.overflowed,
            unsupported: self.unsupported.clone(),
            actions: self
                .trees
                .values()
                .map(|tree| {
                    let mut rows = Vec::new();
                    if !tree.nodes.is_empty() {
                        report(tree, 0, 0, &mut rows);
                    }
                    TlcActionCoverage {
                        label: tree.label.clone(),
                        initial: tree.initial,
                        distinct: tree.distinct,
                        generated: tree.generated,
                        rows,
                    }
                })
                .collect(),
        }
    }
}
fn increment(value: &mut u64, overflowed: &mut bool) {
    if let Some(n) = value.checked_add(1) {
        *value = n;
    } else {
        *overflowed = true;
    }
}
fn counts(tree: &Tree, index: usize, result: &mut BTreeSet<(u64, u64)>) {
    let node = &tree.nodes[index];
    if node.count > 0 || node.cost > 0 || node.primed {
        result.insert((node.count, node.cost));
    }
    for child in &node.children {
        counts(tree, *child, result);
    }
}
fn report(tree: &Tree, index: usize, depth: usize, rows: &mut Vec<TlcCoverageRow>) {
    let node = &tree.nodes[index];
    let mut children = BTreeSet::new();
    for child in &node.children {
        counts(tree, *child, &mut children);
    }
    let own = (node.count, node.cost);
    if children.len() == 1
        && let Some(child) = children.first()
    {
        if !node.primed && own == (0, 0) && *child != (0, 0) {
            rows.push(TlcCoverageRow {
                location: node.location.clone(),
                depth,
                invocations: child.0,
                cost: child.1,
            });
            return;
        }
        if own != (0, 0) && own == *child {
            rows.push(TlcCoverageRow {
                location: node.location.clone(),
                depth,
                invocations: own.0,
                cost: own.1,
            });
            return;
        }
    }
    let printed = own != (0, 0) || node.primed;
    if printed {
        rows.push(TlcCoverageRow {
            location: node.location.clone(),
            depth,
            invocations: own.0,
            cost: own.1,
        });
    }
    for child in &node.children {
        report(tree, *child, depth + usize::from(printed), rows);
    }
}
impl SourceCoverage {
    /// Whether this observer collects pinned TLC logical costs instead of native work.
    #[must_use]
    pub fn is_tlc(&self) -> bool {
        self.logical.is_some()
    }
    pub(crate) fn logical_action_active(&self) -> bool {
        let Some(logical) = &self.logical else {
            return false;
        };
        if !self.observing() {
            return false;
        }
        let logical = logical.lock().unwrap_or_else(PoisonError::into_inner);
        logical.current.is_some() && logical.thread == Some(std::thread::current().id())
    }
    /// Prepares the admitted logical calling forest without evaluating source.
    /// Native observers need no preparation.
    ///
    /// # Errors
    /// A source/caller shape outside the implemented logical-cost slice.
    pub fn prepare_actions(&self, inits: &[Action], actions: &[Action]) -> Result<(), String> {
        if let Some(logical) = &self.logical {
            if inits
                .iter()
                .chain(actions)
                .any(|action| !action.is_for_ir(&self.ir))
            {
                return Err("TLC coverage action belongs to a different IR".into());
            }
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .prepare(&self.ir, inits, actions)?;
        }
        Ok(())
    }
    pub(crate) fn action(self: &Arc<Self>, action: usize) -> Option<LogicalGuard> {
        if !self.observing() {
            return None;
        }
        let logical = self.logical.as_ref()?;
        let mut logical = logical.lock().unwrap_or_else(PoisonError::into_inner);
        let thread = std::thread::current().id();
        if logical.current.is_some() && logical.thread != Some(thread) {
            logical.unsupported = Some("TLC logical coverage requires one producer worker".into());
            return None;
        }
        logical.thread = Some(thread);
        let action = *logical.aliases.get(&action)?;
        let previous = logical.current.replace((action, None));
        Some(LogicalGuard {
            owner: self.clone(),
            previous,
            not_send: std::marker::PhantomData,
        })
    }
    pub(crate) fn generator(self: &Arc<Self>, expr: ExprId) -> Option<LogicalGuard> {
        if !self.observing() {
            return None;
        }
        let mut logical = self
            .logical
            .as_ref()?
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if logical.current.is_some() && logical.thread != Some(std::thread::current().id()) {
            logical.unsupported = Some("TLC logical coverage requires one producer worker".into());
            return None;
        }
        let previous = logical.current;
        if let Some((action, current)) = previous {
            let current = logical.locate(action, current, expr);
            if current.is_none()
                && !matches!(
                    self.ir.expr(expr),
                    Expr::Num(_) | Expr::Str(_) | Expr::Decimal { .. }
                )
            {
                logical.unsupported =
                    Some("TLC logical coverage could not map a reached caller expression".into());
            }
            logical.current = Some((action, current));
        }
        Some(LogicalGuard {
            owner: self.clone(),
            previous,
            not_send: std::marker::PhantomData,
        })
    }
    pub(crate) fn evaluation(self: &Arc<Self>, expr: ExprId) -> Option<LogicalGuard> {
        if !self.is_tlc() {
            self.record(expr, 0);
            return None;
        }
        let guard = self.generator(expr)?;
        let mut logical = self
            .logical
            .as_ref()?
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((action, Some(index))) = logical.current {
            let Logical {
                trees, overflowed, ..
            } = &mut *logical;
            if let Some(tree) = trees.get_mut(&action) {
                increment(&mut tree.nodes[index].count, overflowed);
            }
        }
        Some(guard)
    }
    pub(crate) fn constructed_set(&self, value: &Value) {
        if !self.observing() {
            return;
        }
        let Some(logical) = &self.logical else {
            return;
        };
        let Value::Set(set) = value else {
            return;
        };
        let mut logical = logical.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((action, Some(index))) = logical.current {
            if logical.sets.len() >= 100_000 {
                logical.unsupported = Some("TLC coverage set-source retention capacity".into());
            } else {
                logical.sets.push((set.clone(), action, index));
            }
        }
    }
    pub(crate) fn enumeration(&self, set: &Set) {
        if self.observing()
            && let Some(logical) = &self.logical
        {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .enumeration(set);
        }
    }
    pub(crate) fn scalar_values(&self, values: &[&Value]) {
        if !self.is_tlc() || !self.observing() {
            return;
        }
        if values
            .iter()
            .any(|value| matches!(value, Value::Set(_) | Value::Fcn(_)))
        {
            self.refuse_tlc("TLC coverage compound state/comparison/formatting costs");
        }
    }
    pub(crate) fn interval_base(&self, value: &Value) {
        if !self.is_tlc() || !self.observing() {
            return;
        }
        if !matches!(value, Value::Set(set) if set.as_interval().is_some()) {
            self.refuse_tlc("TLC coverage powerset/difference with a noninterval base requires value source maps");
        }
    }
    /// Refuses logical costs while retaining the independently reported model result.
    pub fn refuse_tlc(&self, feature: impl Into<String>) {
        if let Some(logical) = &self.logical {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .unsupported
                .get_or_insert_with(|| feature.into());
        }
    }
    /// Records a checker-admitted identity under its actual producing action.
    /// Native observation ignores this event; it is not a generator emission.
    pub fn discovered(&self, action: usize) {
        if self.observing()
            && let Some(logical) = &self.logical
        {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .discovered(action);
        }
    }
    pub(crate) fn continued(&self) {
        if self.observing()
            && let Some(logical) = &self.logical
        {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .continued();
        }
    }
    pub(crate) fn rejected_init(&self) {
        if self.observing()
            && let Some(logical) = &self.logical
        {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .rejected_init();
        }
    }
    /// Snapshots TLC logical costs without evaluating or formatting a TLA value.
    #[must_use]
    pub fn tlc_snapshot(&self) -> Option<TlcCoverageSnapshot> {
        self.logical.as_ref().map(|logical| {
            logical
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snapshot(self.observing())
        })
    }
}
