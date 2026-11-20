//! Experimental lowering of bounded aggregate paths and straight-line actions.
//!
//! The first tier uses Core's existing contiguous function payloads. A probe
//! may only read handles. On a shape miss the source operation is interpreted
//! from its original frame. After a successful probe, the normal evaluator
//! supplies the replacement value and any error it raises remains authoritative.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{io::Write, path::PathBuf};

use crate::eval::Machine;
use crate::generate::Sink;
use crate::node::Kind;
use crate::plan::{Action, Code, G, GNode, Node, P, Unch};
use crate::{Error, Handle};

const MAX_PATH: usize = 4;
const MAX_STEPS: usize = 64;
const MAX_KEYED_DOMAIN: usize = 8;

#[derive(Clone, Copy, Debug)]
enum Key {
    Constant(Handle),
    Slot(u32),
}

impl Key {
    fn lower(code: &Code, p: P) -> Option<Self> {
        match code.nodes.get(p as usize)? {
            Node::Const(h) if !h.is_compound() && !h.is_empty() => Some(Self::Constant(*h)),
            Node::Slot(slot) => Some(Self::Slot(*slot)),
            _ => None,
        }
    }

    fn get(self, m: &Machine) -> Option<Handle> {
        let handle = match self {
            Self::Constant(handle) => handle,
            Self::Slot(slot) => m.slot(slot),
        };
        (!handle.is_empty() && !handle.is_compound()).then_some(handle)
    }
}

#[derive(Clone, Copy, Debug)]
enum Root {
    Current(u16),
    Successor(u16),
    Slot(u32),
}

impl Root {
    fn lower(node: &Node) -> Option<Self> {
        match node {
            Node::Var(var) => Some(Self::Current(*var)),
            Node::Primed(var) => Some(Self::Successor(*var)),
            Node::Slot(slot) => Some(Self::Slot(*slot)),
            _ => None,
        }
    }

    fn get(self, m: &Machine) -> Option<Handle> {
        match self {
            Self::Current(var) => m.s0.get(usize::from(var)).copied(),
            Self::Successor(var) => m.s1.get(usize::from(var)).copied(),
            Self::Slot(slot) => Some(m.slot(slot)),
        }
    }
}

#[derive(Clone, Debug)]
struct Path {
    root: Root,
    keys: Box<[Key]>,
}

#[derive(Clone, Copy, Debug)]
struct Location {
    parent: Handle,
    member: Handle,
    index: usize,
    entries: usize,
    payload: usize,
}

impl Path {
    fn lower_read<'a>(code: &'a Code, mut node: &'a Node) -> Option<Self> {
        let mut keys = Vec::new();
        while let Node::Apply(parent, key) = node {
            if keys.len() == MAX_PATH {
                return None;
            }
            keys.push(Key::lower(code, *key)?);
            node = code.nodes.get(*parent as usize)?;
        }
        keys.reverse();
        Some(Self {
            root: Root::lower(node)?,
            keys: keys.into_boxed_slice(),
        })
    }

    fn lower_update(code: &Code, root: P, path: &[P]) -> Option<Self> {
        if path.is_empty() || path.len() > MAX_PATH {
            return None;
        }
        Some(Self {
            root: Root::lower(code.nodes.get(root as usize)?)?,
            keys: path
                .iter()
                .map(|&p| Key::lower(code, p))
                .collect::<Option<_>>()?,
        })
    }

    fn locate(m: &Machine, parent: Handle, key: Handle) -> Option<Location> {
        if !parent.is_compound() {
            return None;
        }
        let view = m.heap.view(parent);
        let index = match view.kind {
            Kind::IntFcn => usize::try_from(key.as_int()?.checked_sub(view.lo)?).ok()?,
            Kind::Fcn if view.len() <= MAX_KEYED_DOMAIN => {
                // A small explicitly keyed function uses Core's linear search.
                // Compound keys could compare equal across representations.
                let keys = view.keys();
                if keys.iter().any(|key| key.is_compound()) {
                    return None;
                }
                keys.iter().position(|&candidate| candidate == key)?
            }
            _ => return None,
        };
        let member = *view.values().get(index)?;
        Some(Location {
            parent,
            member,
            index,
            entries: view.len(),
            payload: view.data.len(),
        })
    }

    fn read(&self, m: &Machine) -> Option<Handle> {
        self.keys.iter().try_fold(self.root.get(m)?, |value, key| {
            Some(Self::locate(m, value, key.get(m)?)?.member)
        })
    }

    fn replacement(&self, m: &Machine) -> Option<(Handle, [Option<Location>; MAX_PATH])> {
        let mut parents = [None; MAX_PATH];
        let mut current = self.root.get(m)?;
        for (key, parent) in self.keys.iter().zip(&mut parents) {
            let location = Self::locate(m, current, key.get(m)?)?;
            *parent = Some(location);
            current = location.member;
        }
        Some((current, parents))
    }
}

#[derive(Clone, Debug)]
enum Expr {
    Read(Path),
    Update { path: Path, at: u32, value: P },
}

impl Expr {
    fn lower(code: &Code, node: &Node) -> Option<Self> {
        match node {
            Node::Apply(_, _) => Some(Self::Read(Path::lower_read(code, node)?)),
            Node::Except { base, updates } => {
                let [update] = updates.as_ref() else {
                    return None;
                };
                Some(Self::Update {
                    path: Path::lower_update(code, *base, &update.path)?,
                    at: update.at,
                    value: update.value,
                })
            }
            _ => None,
        }
    }

    fn run(&self, m: &mut Machine, code: &Code) -> Option<Result<Handle, Error>> {
        match self {
            Self::Read(path) => Some(Ok(path.read(m)?)),
            Self::Update { path, at, value } => {
                let (old, parents) = path.replacement(m)?;
                Some((|| {
                    for location in parents.iter().flatten() {
                        m.charge(location.entries)?;
                    }
                    m.set_slot(*at, old);
                    let mut replacement = m.eval_component(code, *value)?;
                    for location in parents.into_iter().rev().flatten() {
                        replacement = if replacement == location.member {
                            location.parent
                        } else {
                            m.charge(location.payload)?;
                            m.heap
                                .fcn_with(location.parent, location.index, replacement)
                        };
                    }
                    Ok(replacement)
                })())
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Step {
    Guard(P),
    Assign(u16, P),
    Unchanged(u16),
    Scope(Box<[u32]>),
}

#[derive(Clone, Debug)]
struct Linear(Box<[Step]>);

impl Linear {
    fn lower(code: &Code, root: G, vars: usize) -> Option<Self> {
        let full = u64::MAX >> (64 - vars);
        let mut lower = Lower {
            code,
            full,
            assigned: 0,
            steps: Vec::new(),
        };
        lower.node(root, 0)?;
        (lower.assigned == full).then(|| Self(lower.steps.into_boxed_slice()))
    }

    fn run(&self, m: &mut Machine, code: &Code, sink: &mut dyn Sink) -> Result<(), Error> {
        m.clear_buffer();
        let mut reached = 0;
        let result = (|| {
            for step in &self.0 {
                reached += 1;
                match step {
                    Step::Guard(test) if !m.eval_bool(code, *test)? => return Ok(()),
                    Step::Guard(_) => {}
                    Step::Assign(var, value) => {
                        m.s1[usize::from(*var)] = m.eval_component(code, *value)?;
                        m.assigned += 1;
                    }
                    Step::Unchanged(var) => {
                        m.s1[usize::from(*var)] = m.s0[usize::from(*var)];
                        m.assigned += 1;
                    }
                    Step::Scope(slots) => {
                        for &slot in slots {
                            m.saved.push(m.slot(slot));
                            m.set_slot(slot, Handle::EMPTY);
                        }
                    }
                }
            }
            sink.emit(m, code)
        })();
        for step in self.0[..reached].iter().rev() {
            if let Step::Scope(slots) = step {
                for &slot in slots.iter().rev() {
                    let value = m.saved.pop().unwrap_or(Handle::EMPTY);
                    m.set_slot(slot, value);
                }
            }
        }
        m.clear_buffer();
        result
    }
}

struct Lower<'a> {
    code: &'a Code,
    full: u64,
    assigned: u64,
    steps: Vec<Step>,
}

#[derive(Clone, Debug)]
enum Generator {
    Linear(Linear),
    Exists(Box<[u32]>),
    Disjunction,
    Scope,
}

impl Generator {
    fn lower(
        code: &Code,
        plans: &mut [Option<Self>],
        g: G,
        vars: usize,
        depth: usize,
    ) -> Option<()> {
        if depth == MAX_STEPS {
            return None;
        }
        if plans.get(g as usize)?.is_some() {
            return Some(());
        }
        let plan = if let Some(linear) = Linear::lower(code, g, vars) {
            Self::Linear(linear)
        } else {
            match code.gnodes.get(g as usize)? {
                GNode::Exists { binds, body, .. } => {
                    Self::lower(code, plans, *body, vars, depth + 1)?;
                    Self::Exists(
                        binds
                            .iter()
                            .flat_map(|bind| bind.slots.iter().copied())
                            .collect(),
                    )
                }
                GNode::Disj(items) => {
                    let mut lowered = false;
                    for &child in items {
                        lowered |= Self::lower(code, plans, child, vars, depth + 1).is_some();
                    }
                    if !lowered {
                        return None;
                    }
                    Self::Disjunction
                }
                GNode::Scope { body, .. } => {
                    Self::lower(code, plans, *body, vars, depth + 1)?;
                    Self::Scope
                }
                _ => return None,
            }
        };
        plans[g as usize] = Some(plan);
        Some(())
    }
}

impl Lower<'_> {
    fn assignment(&mut self, var: u16, step: Step) -> Option<()> {
        let bit = 1_u64.checked_shl(u32::from(var))?;
        if self.full & bit == 0 || self.assigned & bit != 0 || self.steps.len() >= MAX_STEPS {
            return None;
        }
        self.assigned |= bit;
        self.steps.push(step);
        Some(())
    }

    fn node(&mut self, g: G, depth: usize) -> Option<()> {
        // Any continuation after the last binding would use TLC's boolean
        // shortcut. Leave that entire action to the existing generator.
        if self.assigned == self.full || depth >= MAX_STEPS || self.steps.len() >= MAX_STEPS {
            return None;
        }
        match self.code.gnodes.get(g as usize)? {
            GNode::Conj(items) => {
                for &(child, _) in items {
                    self.node(child, depth + 1)?;
                }
            }
            GNode::Assign { var, rhs } => self.assignment(*var, Step::Assign(*var, *rhs))?,
            GNode::Guard(test) => self.steps.push(Step::Guard(*test)),
            GNode::Unchanged(items) => {
                for item in items {
                    let Unch::Var { var, .. } = *item else {
                        return None;
                    };
                    self.assignment(var, Step::Unchanged(var))?;
                }
            }
            GNode::Scope { reset, body } if reset.len() <= MAX_STEPS => {
                if !reset.is_empty() {
                    self.steps.push(Step::Scope(reset.clone()));
                }
                self.node(*body, depth + 1)?;
            }
            _ => return None,
        }
        Some(())
    }
}

#[derive(Debug)]
struct Tally {
    path: PathBuf,
    eligible: (usize, usize, usize),
    reads: AtomicU64,
    writes: AtomicU64,
    misses: AtomicU64,
    actions: AtomicU64,
}

impl Drop for Tally {
    fn drop(&mut self) {
        // A temporal worker can drop the plan while the command thread
        // holds stderr and waits to join it.
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(
                file,
                "eligible_reads={} eligible_writes={} eligible_actions={} reads={} writes={} misses={} actions={}",
                self.eligible.0,
                self.eligible.1,
                self.eligible.2,
                self.reads.get_mut(),
                self.writes.get_mut(),
                self.misses.get_mut(),
                self.actions.get_mut(),
            );
        }
    }
}

/// Optional compiled aggregate paths and action instructions. For experiments
/// within Core. Callers should leave construction to the plan compiler.
#[derive(Clone, Debug, Default)]
pub struct PackedPlan {
    exprs: Box<[Option<Expr>]>,
    actions: Box<[Option<Generator>]>,
    tally: Option<Arc<Tally>>,
}

impl PackedPlan {
    pub(crate) fn lower(code: &Code, next: &[Action], vars: usize) -> Self {
        let setting = std::env::var_os("DIVE_EXPERIMENT_PACKED");
        if setting.as_deref() != Some(std::ffi::OsStr::new("1")) {
            return Self::default();
        }
        let exprs: Box<[_]> = code
            .nodes
            .iter()
            .map(|node| Expr::lower(code, node))
            .collect();
        let mut actions = vec![None; code.gnodes.len()];
        if (1..=64).contains(&vars) {
            for action in next {
                Generator::lower(code, &mut actions, action.g, vars, 0);
            }
        }
        let tally = std::env::var_os("DIVE_EXPERIMENT_PACKED_STATS").map(|path| {
            let eligible = (
                exprs
                    .iter()
                    .filter(|expr| matches!(expr, Some(Expr::Read(_))))
                    .count(),
                exprs
                    .iter()
                    .filter(|expr| matches!(expr, Some(Expr::Update { .. })))
                    .count(),
                actions
                    .iter()
                    .filter(|action| matches!(action, Some(Generator::Linear(_))))
                    .count(),
            );
            Arc::new(Tally {
                path: PathBuf::from(path),
                eligible,
                reads: AtomicU64::default(),
                writes: AtomicU64::default(),
                misses: AtomicU64::default(),
                actions: AtomicU64::default(),
            })
        });
        Self {
            exprs,
            actions: actions.into_boxed_slice(),
            tally,
        }
    }

    pub(crate) fn eval(&self, m: &mut Machine, code: &Code, p: P) -> Option<Result<Handle, Error>> {
        if m.debugger.is_some() || m.fuel != u64::MAX {
            return None;
        }
        let expr = self.exprs.get(p as usize)?.as_ref()?;
        let result = expr.run(m, code);
        if let Some(tally) = &self.tally {
            let counter = match (expr, result.is_some()) {
                (_, false) => &tally.misses,
                (Expr::Read(_), true) => &tally.reads,
                (Expr::Update { .. }, true) => &tally.writes,
            };
            counter.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    pub(crate) fn generate(
        &self,
        m: &mut Machine,
        code: &Code,
        g: G,
        sink: &mut dyn Sink,
    ) -> Option<Result<(), Error>> {
        if m.debugger.is_some() || m.fuel != u64::MAX || !m.warn || sink.constructive() {
            return None;
        }
        self.actions.get(g as usize)?.as_ref()?;
        m.clear_buffer();
        Some(self.generate_node(m, code, g, sink))
    }

    fn generate_node(
        &self,
        m: &mut Machine,
        code: &Code,
        g: G,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        match (
            self.actions.get(g as usize).and_then(Option::as_ref),
            code.gnodes.get(g as usize),
        ) {
            (Some(Generator::Linear(action)), _) => {
                if let Some(tally) = &self.tally {
                    tally.actions.fetch_add(1, Ordering::Relaxed);
                }
                action.run(m, code, sink)
            }
            (Some(Generator::Exists(slots)), Some(GNode::Exists { binds, body, .. })) => {
                m.charge(slots.len())?;
                let mark = m.save(slots.iter().copied());
                let result = m.for_each_binding(code, binds, &mut |m| {
                    self.generate_node(m, code, *body, sink)?;
                    Ok(true)
                });
                m.restore(mark, slots.iter().copied());
                result.map(|_| ())
            }
            (Some(Generator::Disjunction), Some(GNode::Disj(items))) => {
                for &child in items {
                    self.generate_node(m, code, child, sink)?;
                }
                Ok(())
            }
            (Some(Generator::Scope), Some(GNode::Scope { reset, body })) => {
                let mark = m.save(reset.iter().copied());
                for &slot in reset {
                    m.set_slot(slot, Handle::EMPTY);
                }
                let result = self.generate_node(m, code, *body, sink);
                m.restore(mark, reset.iter().copied());
                result
            }
            _ => m.run_action(code, g, sink),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test input and asserted results")]
mod tests {
    use super::*;
    use crate::memo::Memo;
    use crate::plan::{Arith, Bind, Update};
    use crate::{Opts, Store};

    fn int(value: i64) -> Handle {
        Handle::int(value).unwrap()
    }

    fn machine() -> Machine {
        Machine::new(
            Arc::new(Store::default()),
            2,
            Opts::default(),
            Memo::default(),
        )
    }

    #[test]
    fn nested_paths_agree_and_decline_before_touching_the_update_slot() {
        let mut m = machine();
        let row = m.heap.int_fcn(1, &[int(3), int(4)]);
        let matrix = m.heap.int_fcn(-1, &[row, row]);
        m.s0[0] = matrix;
        let code = Code {
            nodes: vec![
                Node::Var(0),
                Node::Slot(0),
                Node::Const(int(2)),
                Node::Apply(0, 1),
                Node::Apply(3, 2),
                Node::Const(int(5)),
                Node::Slot(1),
                Node::Arith(Arith::Add, 6, 5),
                Node::Except {
                    base: 0,
                    updates: Box::new([Update {
                        path: Box::new([1, 2]),
                        at: 1,
                        value: 7,
                    }]),
                },
            ],
            ..Code::default()
        };
        m.enter(2);
        m.set_slot(0, int(-1));
        let read = Expr::lower(&code, &code.nodes[4]).unwrap();
        let update = Expr::lower(&code, &code.nodes[8]).unwrap();
        assert_eq!(read.run(&mut m, &code).unwrap().unwrap(), int(4));
        let interpreted = m.eval(&code, 8).unwrap();
        let lowered = update.run(&mut m, &code).unwrap().unwrap();
        assert!(m.heap.eq(interpreted, lowered));
        assert_eq!(m.heap.hash(interpreted), m.heap.hash(lowered));
        assert_eq!(
            m.heap.view(m.heap.view(lowered).values()[0]).values(),
            &[int(3), int(9)]
        );
        assert_eq!(m.heap.view(lowered).values()[1], row);

        m.set_slot(0, int(9));
        m.set_slot(1, int(87));
        assert!(read.run(&mut m, &code).is_none());
        assert!(update.run(&mut m, &code).is_none());
        assert_eq!(m.slot(1), int(87));
        assert_eq!(m.eval(&code, 8).unwrap(), matrix);
        assert!(m.eval(&code, 4).is_err());
    }

    #[derive(Default)]
    struct Collect(Vec<Vec<Handle>>);

    impl Sink for Collect {
        fn emit(&mut self, m: &mut Machine, _: &Code) -> Result<(), Error> {
            self.0.push(m.s1.clone());
            Ok(())
        }
    }

    #[test]
    fn linear_scope_preserves_short_circuit_and_commits_a_complete_row() {
        let mut m = machine();
        m.s0.copy_from_slice(&[int(9), int(7)]);
        m.enter(2);
        m.set_slot(1, int(2));
        let code = Code {
            nodes: vec![Node::Slot(0), Node::Const(int(1)), Node::Slot(1)],
            gnodes: vec![
                GNode::Guard(0),
                GNode::Assign { var: 0, rhs: 2 },
                GNode::Unchanged(Box::new([Unch::Var { var: 1, at: 0 }])),
                GNode::Conj(Box::new([(0, 0), (1, 1), (2, 1)])),
                GNode::Scope {
                    reset: Box::new([1]),
                    body: 3,
                },
            ],
            ..Code::default()
        };
        let linear = Linear::lower(&code, 4, 2).unwrap();
        m.set_slot(0, Handle::FALSE);
        let mut sink = Collect::default();
        linear.run(&mut m, &code, &mut sink).unwrap();
        assert_eq!(sink.0, Vec::<Vec<Handle>>::new());
        assert_eq!(m.slot(1), int(2));
        m.set_slot(0, Handle::TRUE);
        assert!(linear.run(&mut m, &code, &mut sink).is_err());
        assert_eq!(sink.0, Vec::<Vec<Handle>>::new());
        assert_eq!(m.slot(1), int(2));
        let mut baseline = Collect::default();
        m.run_action(&code, 3, &mut baseline).unwrap();
        Linear::lower(&code, 3, 2)
            .unwrap()
            .run(&mut m, &code, &mut sink)
            .unwrap();
        assert_eq!(sink.0, baseline.0);
        assert_eq!(sink.0, [vec![int(2), int(7)]]);
        assert_eq!(m.assigned, 0);
    }

    #[test]
    fn lowered_existentials_keep_binding_and_duplicate_order() {
        let mut m = machine();
        let domain = m.heap.interval(1, 3);
        m.s0.copy_from_slice(&[int(8), int(9)]);
        m.enter(1);
        m.set_slot(0, int(87));
        let code = Code {
            nodes: vec![
                Node::Const(domain),
                Node::Slot(0),
                Node::Const(Handle::TRUE),
            ],
            gnodes: vec![
                GNode::Assign { var: 0, rhs: 1 },
                GNode::Unchanged(Box::new([Unch::Var { var: 1, at: 0 }])),
                GNode::Conj(Box::new([(0, 2), (1, 2)])),
                GNode::Disj(Box::new([2, 2])),
                GNode::Exists {
                    binds: Box::new([Bind {
                        slots: Box::new([0]),
                        tuple: false,
                        domain: 0,
                    }]),
                    body: 3,
                    body_p: 2,
                },
            ],
            ..Code::default()
        };
        let mut actions = vec![None; code.gnodes.len()].into_boxed_slice();
        Generator::lower(&code, &mut actions, 4, 2, 0).unwrap();
        let packed = PackedPlan {
            actions,
            ..PackedPlan::default()
        };
        let mut baseline = Collect::default();
        m.run_action(&code, 4, &mut baseline).unwrap();
        let mut lowered = Collect::default();
        packed
            .generate(&mut m, &code, 4, &mut lowered)
            .unwrap()
            .unwrap();
        let expected: Vec<_> = (1..=3)
            .flat_map(|n| [vec![int(n), int(9)], vec![int(n), int(9)]])
            .collect();
        assert_eq!(lowered.0, expected);
        assert_eq!(lowered.0, baseline.0);
        assert_eq!(m.slot(0), int(87));
        assert_eq!(m.assigned, 0);
    }
}
