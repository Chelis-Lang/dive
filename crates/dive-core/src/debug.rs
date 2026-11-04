//! Opt-in maps and observation of actual compiled instruction activations.
use crate::plan::{Code, Node};
use crate::{Error, Handle, Machine};
use dive_ir::{FileInfo, Origin};
use std::sync::Arc;

/// An actual compiled instruction, not a source-text projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instruction {
    /// A value instruction.
    Value(u32),
    /// A generator instruction.
    Generator(u32),
}

/// A local's existing storage; inspection never evaluates a recipe.
#[derive(Clone, Debug)]
pub enum Binding {
    /// A flat activation slot, possibly still unforced/unbound.
    Slot(u32),
    /// A compiled alias, readable only when already a scalar/storage read.
    Alias(u32),
    /// No readable storage exists for this source local.
    Unavailable,
}

/// A source local and its actual compiled storage association.
#[derive(Clone, Debug)]
pub struct Local {
    /// Original source name.
    pub name: String,
    /// Existing storage association.
    pub binding: Binding,
}

/// An actual active compiler substitution, including dynamic INSTANCE contexts.
#[derive(Clone, Debug)]
pub struct Substitution {
    /// The declaration being substituted.
    pub declaration: String,
    /// Origin of the actual expression, if the binding is an expression.
    pub actual: Option<Origin>,
    /// Unique substitution identity in this compilation.
    pub identity: u32,
}

/// Source context captured while compiling an instruction.
#[derive(Clone, Debug)]
pub struct Location {
    /// Exact IR origin, including INSTANCE ownership.
    pub origin: Origin,
    /// Definition whose environment compiled the instruction.
    pub definition: String,
    /// Definition identity; names alone do not identify an activation.
    pub definition_id: usize,
    /// Actual compiler environment identity, distinguishing inlined call sites.
    pub environment_id: usize,
    /// Source locals associated with actual flat storage/aliases.
    pub locals: Vec<Local>,
    /// Calling compilation context, before inlining/substitution.
    pub caller: Option<usize>,
    /// Actual active substitution prefix, distinct from static IR instantiation.
    pub substitutions: Vec<Substitution>,
}

/// Optional instruction/source tables. Ordinary compilation allocates none.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    /// Source files already loaded by the caller, never opened by inspection.
    pub files: Vec<FileInfo>,
    /// State-variable names in actual machine order.
    pub variables: Vec<String>,
    /// Source compilation contexts.
    pub locations: Vec<Location>,
    /// Value instruction to source context; synthetic unmapped nodes are None.
    pub values: Vec<Option<usize>>,
    /// Generator instruction to source context.
    pub generators: Vec<Option<usize>>,
    /// Actual invariant/property root instructions and their configured names.
    pub obligations: Vec<(u32, String)>,
    /// Instructions reachable from the actual compiled plan roots.
    pub executable: Vec<Instruction>,
}

impl SourceMap {
    /// Finds the retained context for an actual instruction.
    #[must_use]
    pub fn location(&self, instruction: Instruction) -> Option<&Location> {
        let index = match instruction {
            Instruction::Value(index) => self.values.get(index as usize),
            Instruction::Generator(index) => self.generators.get(index as usize),
        };
        index.and_then(|index| index.and_then(|index| self.locations.get(index)))
    }
}

/// Actual completion of an observed instruction.
#[derive(Clone, Copy, Debug)]
pub enum Returned<'a> {
    /// An actual returned value.
    Value(Handle),
    /// Completed generator traversal.
    Generated,
    /// The actual error before activation unwinding.
    Failed(&'a Error),
}

/// Explicit producer-only instruction observer. Replay machines stay unobserved.
pub trait Observer: Send + Sync + std::fmt::Debug {
    /// Publishes the real compiled map before search begins.
    ///
    /// # Errors
    /// Cancellation or an unsupported observation boundary.
    fn prepared(&self, map: &SourceMap) -> Result<(), Error>;
    /// Enters an actual instruction activation.
    ///
    /// # Errors
    /// Cancellation or an unsupported observation boundary.
    fn enter(&self, instruction: Instruction, code: &Code, machine: &Machine) -> Result<(), Error>;
    /// Observes a result while its activation remains live.
    ///
    /// # Errors
    /// Cancellation or an unsupported observation boundary.
    fn returned(
        &self,
        instruction: Instruction,
        code: &Code,
        machine: &Machine,
        result: Returned<'_>,
    ) -> Result<(), Error>;
    /// Leaves the matching activation, including on errors.
    fn leave(&self);
}

pub(crate) struct Guard(Arc<dyn Observer>);
impl Guard {
    pub(crate) fn enter(
        observer: &Arc<dyn Observer>,
        instruction: Instruction,
        code: &Code,
        machine: &Machine,
    ) -> Result<Self, Error> {
        observer.enter(instruction, code, machine)?;
        Ok(Self(Arc::clone(observer)))
    }
    pub(crate) fn returned(
        &self,
        instruction: Instruction,
        code: &Code,
        machine: &Machine,
        result: Returned<'_>,
    ) -> Result<(), Error> {
        self.0.returned(instruction, code, machine, result)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.leave();
    }
}

struct Suspended<'a> {
    machine: &'a mut Machine,
    observer: Option<Arc<dyn Observer>>,
}
impl Drop for Suspended<'_> {
    fn drop(&mut self) {
        self.machine.debugger = self.observer.take();
    }
}

impl Machine {
    /// Runs nonproducer reconstruction without this machine's debugger capability.
    /// The original capability is restored when the operation returns or unwinds,
    /// including on errors. The controller and its stopped snapshots are untouched.
    pub fn without_debugger<T>(&mut self, operation: impl FnOnce(&mut Self) -> T) -> T {
        let observer = self.debugger.take();
        let suspended = Suspended {
            machine: self,
            observer,
        };
        operation(&mut *suspended.machine)
    }
}

/// Reads only an already-existing scalar/storage alias. Compound expressions,
/// unforced thunks and by-name recipes remain unavailable; no evaluator is called.
#[must_use]
pub fn read_binding(
    binding: &Binding,
    code: &Code,
    machine: &Machine,
    base: usize,
) -> Option<Handle> {
    let slot = |slot: u32| {
        base.checked_add(slot as usize)
            .and_then(|slot| machine.frame.get(slot))
            .copied()
    };
    match binding {
        Binding::Slot(index) => slot(*index),
        Binding::Alias(index) => match code.nodes.get(*index as usize)? {
            Node::Const(value) => Some(*value),
            Node::Var(index) => machine.s0.get(usize::from(*index)).copied(),
            Node::Primed(index) => machine.s1.get(usize::from(*index)).copied(),
            Node::Slot(index) | Node::Lazy { slot: index, .. } => slot(*index),
            _ => None,
        },
        Binding::Unavailable => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per actual instruction kind for debug-only graph reachability"
)]
pub(crate) fn reachable(plan: &crate::plan::Plan) -> Vec<Instruction> {
    use crate::plan::{GNode, LibArg, Unch};
    let mut pending = Vec::new();
    pending.extend(
        plan.init
            .iter()
            .chain(&plan.next)
            .map(|action| Instruction::Generator(action.g)),
    );
    for check in plan
        .invariants
        .iter()
        .chain(&plan.constraints)
        .chain(&plan.action_constraints)
        .chain(&plan.implied_inits)
        .chain(&plan.implied_actions)
        .chain(plan.view.iter())
    {
        pending.extend(
            check
                .conjuncts
                .iter()
                .map(|part| Instruction::Value(part.p)),
        );
    }
    let mut seen = std::collections::HashSet::new();
    while let Some(instruction) = pending.pop() {
        if !seen.insert(instruction) {
            continue;
        }
        let mut values = Vec::new();
        let mut generators = Vec::new();
        match instruction {
            Instruction::Value(index) => {
                if let Some(node) = plan.code.nodes.get(index as usize) {
                    match node {
                        Node::Const(_) | Node::Slot(_) | Node::Var(_) | Node::Primed(_) => {}
                        Node::Not(a)
                        | Node::Neg(a)
                        | Node::Powerset(a)
                        | Node::BigUnion(a)
                        | Node::Domain(a)
                        | Node::Spec(a)
                        | Node::Select(a, _) => values.push(*a),
                        Node::Lazy { body, .. }
                        | Node::Scope { body, .. }
                        | Node::Memo { body, .. } => values.push(*body),
                        Node::Implies(a, b)
                        | Node::Equiv(a, b)
                        | Node::Eq(a, b)
                        | Node::Neq(a, b)
                        | Node::In(a, b)
                        | Node::NotIn(a, b)
                        | Node::Subseteq(a, b)
                        | Node::Range(a, b)
                        | Node::FcnSet(a, b)
                        | Node::Apply(a, b)
                        | Node::Arith(_, a, b)
                        | Node::Cmp(_, a, b)
                        | Node::SetBin(_, a, b) => values.extend([*a, *b]),
                        Node::If(a, b, c) => values.extend([*a, *b, *c]),
                        Node::And(items)
                        | Node::Or(items)
                        | Node::Product(items)
                        | Node::Tuple(items)
                        | Node::Std(_, items)
                        | Node::SetEnum(items, _)
                        | Node::Lib { args: items, .. } => values.extend(items.iter().copied()),
                        Node::Record(items) | Node::RecSet(items) => {
                            values.extend(items.iter().map(|(_, p)| *p));
                        }
                        Node::Case(arms, other) => {
                            for (a, b) in arms {
                                values.extend([*a, *b]);
                            }
                            values.extend(*other);
                        }
                        Node::Filter { bind, pred } => values.extend([bind.domain, *pred]),
                        Node::Map { binds, body }
                        | Node::FcnCons { binds, body }
                        | Node::Quant { binds, body, .. } => {
                            values.extend(binds.iter().map(|bind| bind.domain));
                            values.push(*body);
                        }
                        Node::ClosedUpdate {
                            base,
                            key,
                            domain,
                            value,
                            ..
                        } => {
                            values.extend([*base, *key, *value]);
                            values.extend(*domain);
                        }
                        Node::Except { base, updates } => {
                            values.push(*base);
                            for update in updates {
                                values.extend(update.path.iter().copied());
                                values.push(update.value);
                            }
                        }
                        Node::Call { fun, args } => {
                            values.extend(args.iter().copied());
                            if let Some(fun) = plan.code.funs.get(*fun as usize) {
                                values.push(fun.body);
                            }
                        }
                        Node::LibHo(_, args) => {
                            for arg in args {
                                values.push(match arg {
                                    LibArg::Value(p) => *p,
                                    LibArg::Op { body, .. } => *body,
                                });
                            }
                        }
                        Node::Enabled { g, .. } => generators.push(*g),
                        Node::InLazy { elem, set, .. } => values.extend([*elem, *set]),
                        Node::Beta { chain, fc, arg } => values.extend([*chain, *fc, *arg]),
                        Node::DomCheck { domain, body, .. } => values.extend([*domain, *body]),
                    }
                }
            }
            Instruction::Generator(index) => {
                if let Some(node) = plan.code.gnodes.get(index as usize) {
                    let mut unchanged = |items: &[Unch]| {
                        values.extend(items.iter().filter_map(|item| {
                            if let Unch::Verify(p) = item {
                                Some(*p)
                            } else {
                                None
                            }
                        }));
                    };
                    match node {
                        GNode::Conj(items) => {
                            for (g, p) in items {
                                generators.push(*g);
                                values.push(*p);
                            }
                        }
                        GNode::Disj(items) => generators.extend(items.iter().copied()),
                        GNode::Exists {
                            binds,
                            body,
                            body_p,
                        }
                        | GNode::Forall {
                            binds,
                            body,
                            body_p,
                        } => {
                            values.extend(binds.iter().map(|bind| bind.domain));
                            values.push(*body_p);
                            generators.push(*body);
                        }
                        GNode::If { cond, then, other } => {
                            values.push(*cond);
                            generators.extend([*then, *other]);
                        }
                        GNode::Case { arms, other } => {
                            for (p, g) in arms {
                                values.push(*p);
                                generators.push(*g);
                            }
                            generators.extend(*other);
                        }
                        GNode::Implies { lhs, rhs } => {
                            values.push(*lhs);
                            generators.push(*rhs);
                        }
                        GNode::Assign { rhs, .. } | GNode::Guard(rhs) => values.push(*rhs),
                        GNode::AssignIn { set, .. } => values.push(*set),
                        GNode::AssignInRaw { set, raw, .. } => {
                            values.push(*set);
                            raw_instructions(raw, &mut values);
                        }
                        GNode::Unchanged(items) => unchanged(items),
                        GNode::StepBox { action, vars } => {
                            unchanged(vars);
                            generators.push(*action);
                        }
                        GNode::StepAngle { action, same } => {
                            values.push(*same);
                            generators.push(*action);
                        }
                        GNode::FcnAction { binds, arg, body } => {
                            values.extend(binds.iter().map(|bind| bind.domain));
                            values.push(*arg);
                            generators.push(*body);
                        }
                        GNode::Scope { body, .. } => generators.push(*body),
                    }
                }
            }
        }
        pending.extend(values.into_iter().map(Instruction::Value));
        pending.extend(generators.into_iter().map(Instruction::Generator));
    }
    seen.into_iter().collect()
}

fn raw_instructions(raw: &crate::plan::Raw, values: &mut Vec<u32>) {
    use crate::plan::Raw;
    let mut pending = vec![raw];
    while let Some(raw) = pending.pop() {
        match raw {
            Raw::Leaf(node) | Raw::UnionOf(node) => values.push(*node),
            Raw::Cup(left, right) => pending.extend([&**left, &**right]),
            Raw::Union(items) => pending.extend(items.iter()),
            Raw::Filter { bind, pred, inner } => {
                values.extend([bind.domain, *pred]);
                pending.push(inner);
            }
            Raw::Cap(left, right, first, second) => {
                pending.extend([&**left, &**right]);
                values.extend([*first, *second]);
            }
            Raw::Minus(inner, other) => {
                pending.push(inner);
                values.push(*other);
            }
            Raw::Odometer(items, node) => {
                pending.extend(items.iter());
                values.push(*node);
            }
        }
    }
}
