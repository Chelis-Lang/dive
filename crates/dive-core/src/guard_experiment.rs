//! Opt-in experiment for evaluating bounded, pure generator guards.
//!
//! The original plan still owns admission and all error reporting. The tape
//! reads only scalar handles and touches no evaluator state before a fallback.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::eval::{Machine, fast_arith};
use crate::handle::Handle;
use crate::plan::{Arith, Cmp, Code, G, GNode, Node, P};

const MAX_OPS: usize = 32;

#[derive(Clone, Copy, Debug)]
enum Op {
    Const(P),
    Current(u16),
    Primed(u16),
    Slot(u32),
    Not,
    Bool,
    ShortCircuit { when: bool, end: usize },
    Equal(bool),
    Cmp(Cmp),
    Arith(Arith),
}

#[derive(Default, Debug)]
struct Counts {
    executed: AtomicU64,
    fallback: AtomicU64,
}

impl Drop for Counts {
    fn drop(&mut self) {
        eprintln!(
            "guard-spike executed={} fallback={}",
            self.executed.get_mut(),
            self.fallback.get_mut()
        );
    }
}

type GuardTape = Box<[Op]>;
type MemoTape = (P, GuardTape);

/// Private experiment state carried by an immutable compiled plan.
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct GuardExperiment {
    programs: Option<Arc<[Option<GuardTape>]>>,
    memo_bodies: Option<Arc<[Option<MemoTape>]>>,
    counts: Option<Arc<Counts>>,
}

impl GuardExperiment {
    pub(crate) fn lower(code: &Code) -> Self {
        let mode = std::env::var("DIVE_EXPERIMENT_GUARDS").ok();
        if !matches!(mode.as_deref(), Some("1" | "stats")) {
            return Self::default();
        }
        let start = std::time::Instant::now();
        let mut memo_bodies = vec![None; code.sites.len()];
        let programs: Vec<_> = code
            .gnodes
            .iter()
            .map(|g| {
                let GNode::Guard(root) = g else { return None };
                let mut ops = Vec::new();
                if let Some(Node::Memo { body, site }) = code.nodes.get(*root as usize) {
                    // Compile only the miss path. Core retains the original
                    // key, lookup, insertion, and cache bypass behavior.
                    emit(code, *body, &mut ops, 0)?;
                    *memo_bodies.get_mut(*site as usize)? = Some((*body, ops.into_boxed_slice()));
                    return None;
                }
                emit(code, *root, &mut ops, 0)?;
                Some(ops.into_boxed_slice())
            })
            .collect();
        let count = mode.as_deref() == Some("stats");
        if count {
            eprintln!(
                "guard-spike admitted={} guard_nodes={} lower_us={}",
                programs.iter().flatten().count() + memo_bodies.iter().flatten().count(),
                code.gnodes
                    .iter()
                    .filter(|n| matches!(n, GNode::Guard(_)))
                    .count(),
                start.elapsed().as_micros(),
            );
        }
        Self {
            programs: Some(programs.into()),
            memo_bodies: Some(memo_bodies.into()),
            counts: count.then(|| Arc::new(Counts::default())),
        }
    }

    pub(crate) fn evaluate(&self, code: &Code, machine: &Machine, g: G) -> Option<bool> {
        let ops = self.programs.as_ref()?.get(g as usize)?.as_deref()?;
        self.evaluate_tape(code, machine, ops)
    }

    pub(crate) fn memo_body(
        &self,
        code: &Code,
        machine: &Machine,
        site: u32,
        body: P,
    ) -> Option<Handle> {
        let (lowered_body, ops) = self.memo_bodies.as_ref()?.get(site as usize)?.as_ref()?;
        if body != *lowered_body {
            return None;
        }
        self.evaluate_tape(code, machine, ops).map(Handle::bool)
    }

    fn evaluate_tape(&self, code: &Code, machine: &Machine, ops: &[Op]) -> Option<bool> {
        let value = if machine.fuel == u64::MAX
            && machine.debugger.is_none()
            && machine.transient_bound == usize::MAX
        {
            run(code, machine, ops)
        } else {
            None
        };
        if let Some(counts) = &self.counts {
            counts.executed.fetch_add(1, Ordering::Relaxed);
            if value.is_none() {
                counts.fallback.fetch_add(1, Ordering::Relaxed);
            }
        }
        value
    }
}

fn emit(code: &Code, root: P, ops: &mut Vec<Op>, depth: usize) -> Option<()> {
    if depth == MAX_OPS || ops.len() == MAX_OPS {
        return None;
    }
    let emit_child = |ops: &mut Vec<Op>, child| emit(code, child, ops, depth + 1);
    let op = match code.nodes.get(root as usize)? {
        Node::Const(value) if value.as_bool().is_some() || value.as_int().is_some() => {
            Op::Const(root)
        }
        Node::Var(var) => Op::Current(*var),
        Node::Primed(var) => Op::Primed(*var),
        Node::Slot(slot) => Op::Slot(*slot),
        Node::Not(arg) => {
            emit_child(ops, *arg)?;
            Op::Not
        }
        Node::And(children) | Node::Or(children) => {
            let when = matches!(code.nodes[root as usize], Node::Or(_));
            let (last, prefix) = children.split_last()?;
            let start = ops.len();
            for &child in prefix {
                emit_child(ops, child)?;
                if ops.len() == MAX_OPS {
                    return None;
                }
                ops.push(Op::ShortCircuit { when, end: 0 });
            }
            emit_child(ops, *last)?;
            let end = ops.len() + 1;
            for op in &mut ops[start..] {
                if let Op::ShortCircuit { end: target, .. } = op
                    && *target == 0
                {
                    *target = end;
                }
            }
            Op::Bool
        }
        Node::Eq(a, b) | Node::Neq(a, b) => {
            emit_child(ops, *a)?;
            emit_child(ops, *b)?;
            Op::Equal(matches!(code.nodes[root as usize], Node::Eq(..)))
        }
        Node::Cmp(kind, a, b) => {
            emit_child(ops, *a)?;
            emit_child(ops, *b)?;
            Op::Cmp(*kind)
        }
        Node::Arith(kind @ (Arith::Add | Arith::Sub | Arith::Mul), a, b) => {
            emit_child(ops, *a)?;
            emit_child(ops, *b)?;
            Op::Arith(*kind)
        }
        _ => return None,
    };
    if ops.len() == MAX_OPS {
        return None;
    }
    ops.push(op);
    Some(())
}

fn run(code: &Code, machine: &Machine, ops: &[Op]) -> Option<bool> {
    let mut stack = [Handle::EMPTY; MAX_OPS];
    let mut top = 0_usize;
    let mut pc = 0;
    while let Some(op) = ops.get(pc) {
        pc += 1;
        let value = match *op {
            Op::Const(node) => match code.nodes.get(node as usize)? {
                Node::Const(value) => *value,
                _ => return None,
            },
            Op::Current(var) => *machine.s0.get(usize::from(var))?,
            Op::Primed(var) => *machine.s1.get(usize::from(var))?,
            Op::Slot(slot) => machine.slot(slot),
            Op::ShortCircuit { when, end } => {
                let value = stack.get(top.checked_sub(1)?)?.as_bool()?;
                if value == when {
                    pc = end;
                } else {
                    top -= 1;
                }
                continue;
            }
            Op::Not | Op::Bool => {
                top = top.checked_sub(1)?;
                let value = stack.get(top)?.as_bool()?;
                Handle::bool(value ^ matches!(op, Op::Not))
            }
            Op::Equal(_) | Op::Cmp(_) | Op::Arith(_) => {
                top = top.checked_sub(2)?;
                binary(op, *stack.get(top)?, *stack.get(top + 1)?, machine)?
            }
        };
        if value.as_bool().is_none() && value.as_int().is_none() {
            return None;
        }
        *stack.get_mut(top)? = value;
        top += 1;
    }
    if top == 1 { stack[0].as_bool() } else { None }
}

fn binary(op: &Op, a: Handle, b: Handle, machine: &Machine) -> Option<Handle> {
    match *op {
        Op::Equal(equal) => {
            let same = match (a.as_bool(), b.as_bool()) {
                (Some(a), Some(b)) => a == b,
                _ => a.as_int()? == b.as_int()?,
            };
            Some(Handle::bool(same == equal))
        }
        Op::Cmp(kind) => {
            let order = a.as_int()?.cmp(&b.as_int()?);
            Some(Handle::bool(match kind {
                Cmp::Lt => order.is_lt(),
                Cmp::Le => order.is_le(),
                Cmp::Gt => order.is_gt(),
                Cmp::Ge => order.is_ge(),
            }))
        }
        Op::Arith(kind) => fast_arith(kind, a.as_int()?, b.as_int()?, machine.heap.int_mode()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memo::Memo;
    use crate::opts::Opts;
    use crate::store::Store;

    fn machine(tlc: bool) -> Machine {
        let semantics = if tlc {
            crate::Semantics::tlc()
        } else {
            crate::Semantics::default()
        };
        Machine::new(
            Arc::new(Store::with_semantics(true, semantics)),
            1,
            Opts::default(),
            Memo::default(),
        )
    }

    #[test]
    fn guard_tape_keeps_short_circuit_order() {
        let code = Code {
            nodes: vec![
                Node::Const(Handle::FALSE),
                Node::Primed(0),
                Node::And(Box::new([0, 1])),
                Node::Const(Handle::TRUE),
                Node::Or(Box::new([2, 3, 1])),
                Node::And(Box::new([3, 1])),
            ],
            ..Code::default()
        };
        for tlc in [false, true] {
            let mut machine = machine(tlc);
            let mut tape = Vec::new();
            assert_eq!(emit(&code, 4, &mut tape, 0), Some(()));
            assert_eq!(run(&code, &machine, &tape), Some(true));
            assert_eq!(machine.eval_bool(&code, 4), Ok(true));

            tape.clear();
            assert_eq!(emit(&code, 5, &mut tape, 0), Some(()));
            assert_eq!(run(&code, &machine, &tape), None);
            assert!(machine.eval_bool(&code, 5).is_err());
        }
    }

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "the fixed test integers fit an inline handle"
    )]
    fn guard_tape_defers_out_of_line_integers_to_core() {
        let max_tlc = Handle::int(i64::from(i32::MAX)).expect("32-bit integer");
        let one = Handle::int(1).expect("positive unit");
        let code = Code {
            nodes: vec![
                Node::Var(0),
                Node::Const(one),
                Node::Arith(Arith::Add, 0, 1),
                Node::Cmp(Cmp::Gt, 2, 0),
            ],
            ..Code::default()
        };
        let mut tape = Vec::new();
        assert_eq!(emit(&code, 3, &mut tape, 0), Some(()));
        let mut native = machine(false);
        let mut tlc = machine(true);
        native.s0[0] = max_tlc;
        tlc.s0[0] = max_tlc;
        assert_eq!(run(&code, &native, &tape), Some(true));
        assert_eq!(native.eval_bool(&code, 3), Ok(true));
        assert_eq!(run(&code, &tlc, &tape), None);
        assert!(tlc.eval_bool(&code, 3).is_err());
    }
}
