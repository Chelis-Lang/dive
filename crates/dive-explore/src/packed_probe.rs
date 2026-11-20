//! Prototype for a bounded fixed-domain integer function. All reachable rows
//! stay encoded from generation through exact membership and the frontier.
//! Active only for the exact-store experiment. A shape miss or an ending
//! requiring a trace delegates the entire run to the ordinary explorer.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use dive_core::handle::Tag;
use dive_core::node::Kind;
use dive_core::plan::{Action, Arith, Cmp, Code, G, GNode, Node, P, Quant};
use dive_core::{Error, Handle, Machine, Plan, Sink, Store};
use serde_json::json;

use crate::{Config, Metrics, Outcome, Stats, Verdict};

const MAX_RANK_BITS: u32 = 24;
const MAX_CELL: u32 = 63;
const DISABLED: u8 = u8::MAX;
const DECLINE: u8 = u8::MAX - 1;

type Bindings = [(u32, Handle)];

#[derive(Default)]
struct Initial(Option<(Vec<Handle>, Vec<u32>)>);

impl Sink for Initial {
    fn emit(&mut self, m: &mut Machine, _: &Code) -> Result<(), Error> {
        let [root] = m.s1.as_slice() else {
            return Err(Error::stop());
        };
        if self.0.is_some() || !root.is_compound() {
            return Err(Error::stop());
        }
        let view = m.heap.view(*root);
        let keys = match view.kind {
            Kind::Fcn
                if view
                    .keys()
                    .iter()
                    .all(|key| matches!(key.tag(), Tag::Model | Tag::Int)) =>
            {
                view.keys().to_vec()
            }
            Kind::IntFcn => view
                .values()
                .iter()
                .enumerate()
                .map(|(i, _)| Handle::int(view.lo.checked_add(i64::try_from(i).ok()?)?))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(Error::stop)?,
            _ => return Err(Error::stop()),
        };
        let cells = view
            .values()
            .iter()
            .map(|value| {
                u32::try_from(value.as_int()?)
                    .ok()
                    .filter(|&n| n <= MAX_CELL)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(Error::stop)?;
        self.0 = Some((keys, cells));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Scalar {
    Constant(i64),
    Cell(usize),
}

impl Scalar {
    fn in_cell(self, lane: usize, value: u8) -> Option<i64> {
        match self {
            Self::Constant(n) => Some(n),
            Self::Cell(cell) if cell == lane => Some(i64::from(value)),
            Self::Cell(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Test {
    Constant(bool),
    Compare(Cmp, Scalar, Scalar),
    Equals(bool, Scalar, Scalar),
}

impl Test {
    fn holds(self, lane: usize, value: u8) -> Option<bool> {
        let get = |operand: Scalar| operand.in_cell(lane, value);
        Some(match self {
            Self::Constant(outcome) => outcome,
            Self::Equals(equal, a, b) => (get(a)? == get(b)?) == equal,
            Self::Compare(cmp, a, b) => {
                let (a, b) = (get(a)?, get(b)?);
                match cmp {
                    Cmp::Lt => a < b,
                    Cmp::Le => a <= b,
                    Cmp::Gt => a > b,
                    Cmp::Ge => a >= b,
                }
            }
        })
    }

    fn lane(self) -> Option<usize> {
        match self {
            Self::Compare(_, Scalar::Cell(lane), _)
            | Self::Compare(_, _, Scalar::Cell(lane))
            | Self::Equals(_, Scalar::Cell(lane), _)
            | Self::Equals(_, _, Scalar::Cell(lane)) => Some(lane),
            _ => None,
        }
    }
}

struct Shape<'a> {
    code: &'a Code,
    keys: Vec<Handle>,
    width: u32,
    mask: u32,
}

impl Shape<'_> {
    fn value(&self, p: P) -> Option<&Node> {
        let mut value = self.code.nodes.get(p as usize)?;
        // Memo and speculative wrappers contribute no effect on a proven
        // primitive, total operand. Reject shared slots that need forcing.
        for _ in 0..16 {
            match value {
                Node::Memo { body, .. } | Node::Spec(body) => {
                    value = self.code.nodes.get(*body as usize)?;
                }
                _ => return Some(value),
            }
        }
        None
    }

    fn literal(&self, p: P, bindings: &Bindings) -> Option<Handle> {
        match self.value(p)? {
            Node::Const(handle) => Some(*handle),
            Node::Slot(slot) => bindings
                .iter()
                .rev()
                .find(|(s, _)| s == slot)
                .map(|pair| pair.1),
            _ => None,
        }
    }

    fn scalar(&self, p: P, bindings: &Bindings) -> Option<Scalar> {
        if let Some(handle) = self.literal(p, bindings) {
            return Some(Scalar::Constant(handle.as_int()?));
        }
        let Node::Apply(parent, key) = self.value(p)? else {
            return None;
        };
        if !matches!(self.value(*parent)?, Node::Var(0)) {
            return None;
        }
        Some(Scalar::Cell(self.lane(*key, bindings)?))
    }

    fn lane(&self, key: P, bindings: &Bindings) -> Option<usize> {
        let key = self.literal(key, bindings)?;
        self.keys.iter().position(|&candidate| candidate == key)
    }

    fn test(&self, p: P, bindings: &Bindings) -> Option<Test> {
        match self.value(p)? {
            Node::Const(handle) => Some(Test::Constant(handle.as_bool()?)),
            Node::Cmp(op, a, b) => Some(Test::Compare(
                *op,
                self.scalar(*a, bindings)?,
                self.scalar(*b, bindings)?,
            )),
            value @ (Node::Eq(a, b) | Node::Neq(a, b)) => Some(Test::Equals(
                matches!(value, Node::Eq(_, _)),
                self.scalar(*a, bindings)?,
                self.scalar(*b, bindings)?,
            )),
            _ => None,
        }
    }

    fn invariant(
        &self,
        m: &Machine,
        p: P,
        bindings: &mut Vec<(u32, Handle)>,
        allowed: &mut [u64],
        depth: usize,
    ) -> Option<()> {
        if depth >= 16 {
            return None;
        }
        if let Node::Quant {
            kind: Quant::Forall,
            binds,
            body,
        } = self.value(p)?
        {
            let [bind] = binds.as_ref() else { return None };
            let [slot] = bind.slots.as_ref() else {
                return None;
            };
            if bind.tuple {
                return None;
            }
            let domain = self.literal(bind.domain, bindings)?;
            if !domain.is_compound() {
                return None;
            }
            let view = m.heap.view(domain);
            if view.kind != Kind::Set || view.data.len() > 32 {
                return None;
            }
            for &key in view.data {
                bindings.push((*slot, key));
                self.invariant(m, *body, bindings, allowed, depth + 1)?;
                bindings.pop();
            }
            return Some(());
        }
        let test = self.test(p, bindings)?;
        if let Some(lane) = test.lane() {
            let entry = allowed.get_mut(lane)?;
            for value in 0..=u8::try_from(self.mask).ok()? {
                if !test.holds(lane, value)? {
                    *entry &= !(1 << value);
                }
            }
        } else if !test.holds(0, 0)? {
            return None;
        }
        Some(())
    }

    fn action(&self, action: &Action) -> Option<Transition> {
        let mut guards = Vec::new();
        let mut assignment = None;
        self.action_steps(action.g, &action.preset, &mut guards, &mut assignment, 0)?;
        let (lane, update) = assignment?;
        let shift = u32::try_from(lane).ok()? * self.width;
        let mut next = [DECLINE; (MAX_CELL + 1) as usize];
        for value in 0..=u8::try_from(self.mask).ok()? {
            let mut enabled = true;
            for &test in &guards {
                if !test.holds(lane, value)? {
                    enabled = false;
                    break;
                }
            }
            next[usize::from(value)] = if enabled {
                update.replacement(value, self.mask).unwrap_or(DECLINE)
            } else {
                DISABLED
            };
        }
        Some(Transition { lane, shift, next })
    }

    fn action_steps(
        &self,
        g: G,
        bindings: &Bindings,
        guards: &mut Vec<Test>,
        assignment: &mut Option<(usize, Update)>,
        depth: usize,
    ) -> Option<()> {
        if depth >= 16 || guards.len() >= 16 || assignment.is_some() {
            return None;
        }
        match self.code.gnodes.get(g as usize)? {
            GNode::Scope { reset, body } if reset.is_empty() => {
                self.action_steps(*body, bindings, guards, assignment, depth + 1)?;
            }
            GNode::Conj(items) => {
                for &(child, _) in items {
                    self.action_steps(child, bindings, guards, assignment, depth + 1)?;
                }
            }
            GNode::Guard(p) => guards.push(self.test(*p, bindings)?),
            GNode::Assign { var: 0, rhs } => {
                let Node::Except { base, updates } = self.value(*rhs)? else {
                    return None;
                };
                let [update] = updates.as_ref() else {
                    return None;
                };
                let [key] = update.path.as_ref() else {
                    return None;
                };
                if !matches!(self.value(*base)?, Node::Var(0)) {
                    return None;
                }
                let lane = self.lane(*key, bindings)?;
                let replacement = match self.value(update.value)? {
                    Node::Const(handle) => Update::Constant(handle.as_int()?),
                    Node::Arith(Arith::Add, a, b) if matches!(self.value(*a)?, Node::Slot(s) if *s == update.at) => {
                        Update::Add(self.literal(*b, bindings)?.as_int()?)
                    }
                    _ => return None,
                };
                *assignment = Some((lane, replacement));
            }
            _ => return None,
        }
        Some(())
    }
}

enum Update {
    Constant(i64),
    Add(i64),
}

impl Update {
    fn replacement(&self, value: u8, mask: u32) -> Option<u8> {
        let replacement = match self {
            Self::Constant(n) => *n,
            Self::Add(delta) => i64::from(value).checked_add(*delta)?,
        };
        u8::try_from(replacement)
            .ok()
            .filter(|&n| u32::from(n) <= mask)
    }
}

struct Transition {
    lane: usize,
    shift: u32,
    next: [u8; (MAX_CELL + 1) as usize],
}

fn explore(
    shape: &Shape<'_>,
    cells: &[u32],
    transitions: &[Transition],
    allowed: &[u64],
    stop: &dyn Fn() -> bool,
) -> Option<Stats> {
    let bits = u32::try_from(cells.len()).ok()?.checked_mul(shape.width)?;
    if bits > MAX_RANK_BITS || cells.is_empty() {
        return None;
    }
    let mut root = 0;
    for (i, &value) in cells.iter().enumerate() {
        if value > shape.mask || allowed[i] & (1 << value) == 0 {
            return None;
        }
        root |= value << (u32::try_from(i).ok()? * shape.width);
    }
    let seen_words = (1_usize << bits).div_ceil(64);
    let mut seen = Vec::new();
    seen.try_reserve_exact(seen_words).ok()?;
    seen.resize(seen_words, 0_u64);
    seen[root as usize / 64] |= 1 << (root % 64);
    let mut frontier = vec![root];
    let mut next = Vec::new();
    let mut stats = Stats {
        distinct: 1,
        generated: 1,
        initial_generated: 1,
        ..Stats::default()
    };
    while !frontier.is_empty() {
        stats.depth += 1;
        for (position, &state) in frontier.iter().enumerate() {
            if position.is_multiple_of(1024) && stop() {
                return None;
            }
            let generated_before = stats.generated;
            for action in transitions {
                let value = (state >> action.shift) & shape.mask;
                let replacement = match action.next[value as usize] {
                    DISABLED => continue,
                    DECLINE => return None,
                    replacement => u32::from(replacement),
                };
                stats.generated += 1;
                let field = shape.mask << action.shift;
                let destination = (state & !field) | (replacement << action.shift);
                let word = &mut seen[destination as usize / 64];
                let mask = 1_u64 << (destination % 64);
                if *word & mask == 0 {
                    if allowed[action.lane] & (1 << replacement) == 0 {
                        return None;
                    }
                    *word |= mask;
                    stats.distinct += 1;
                    if next.len() == next.capacity() {
                        next.try_reserve(1).ok()?;
                    }
                    next.push(destination);
                }
            }
            if stats.generated == generated_before {
                return None;
            }
        }
        frontier.clear();
        std::mem::swap(&mut frontier, &mut next);
    }
    Some(stats)
}

fn run(
    plan: &Plan,
    store: &Arc<Store>,
    config: Config,
    stop: &dyn Fn() -> bool,
) -> Option<(Stats, u32)> {
    if plan.vars.len() != 1
        || plan.effects.any
        || plan.symmetry.is_some()
        || plan.view.is_some()
        || !plan.constraints.is_empty()
        || !plan.action_constraints.is_empty()
        || !plan.implied_inits.is_empty()
        || !plan.implied_actions.is_empty()
        || plan.startup_assumption_false.is_some()
        || plan.next.is_empty()
    {
        return None;
    }
    let initialization = plan.init.as_slice();
    let [action] = initialization else {
        return None;
    };
    let mut m = Machine::new(Arc::clone(store), 1, config.opts, crate::Memo::default());
    let old = m.enter(action.frame);
    for &(slot, value) in &action.preset {
        *m.frame.get_mut(m.base + slot as usize)? = value;
    }
    let mut initial = Initial::default();
    m.run_action(&plan.code, action.g, &mut initial).ok()?;
    m.leave(old);
    if m.heap.poisoned() {
        return None;
    }
    let (keys, cells) = initial.0?;
    let max_constant = plan
        .code
        .nodes
        .iter()
        .filter_map(|node| match node {
            Node::Const(handle) => u32::try_from(handle.as_int()?)
                .ok()
                .filter(|&n| n <= MAX_CELL),
            _ => None,
        })
        .chain(cells.iter().copied())
        .max()?;
    let width = max_constant.bit_width().max(1);
    let shape = Shape {
        code: &plan.code,
        keys,
        width,
        mask: (1 << width) - 1,
    };
    let mut allowed = vec![u64::MAX; cells.len()];
    for invariant in &plan.invariants {
        for conjunct in &invariant.conjuncts {
            shape.invariant(&m, conjunct.p, &mut Vec::new(), &mut allowed, 0)?;
        }
    }
    let transitions = plan
        .next
        .iter()
        .map(|action| shape.action(action))
        .collect::<Option<Vec<_>>>()?;
    if let Some(path) = std::env::var_os("DIVE_EXPERIMENT_RESIDENT_PLAN") {
        let program = json!({
            "width": width,
            "cells": cells,
            "allowed": allowed,
            "transitions": transitions.iter().map(|step| json!({
                "lane": step.lane, "shift": step.shift, "next": step.next.as_slice(),
            })).collect::<Vec<_>>(),
        });
        if let Ok(mut out) = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(path)
        {
            let _ = writeln!(out, "{program}");
        }
    }
    let stats = explore(&shape, &cells, &transitions, &allowed, stop)?;
    Some((stats, width * u32::try_from(cells.len()).ok()?))
}

pub(crate) fn try_run(
    plan: &Plan,
    store: &Arc<Store>,
    config: Config,
    stop: &dyn Fn() -> bool,
) -> Option<Outcome> {
    let enabled =
        std::env::var_os("DIVE_EXPERIMENT_RESIDENT").as_deref() == Some(std::ffi::OsStr::new("1"));
    let path = std::env::var_os("DIVE_EXPERIMENT_RESIDENT_PROBE");
    if !(enabled || path.is_some()) {
        return None;
    }
    let started = Instant::now();
    let result = run(plan, store, config, stop);
    let elapsed = started.elapsed();
    if let Some(path) = path
        && let Ok(mut out) = OpenOptions::new().append(true).create(true).open(path)
    {
        let report = result.map_or_else(
            || json!({"status": "declined"}),
            |(stats, bits)| {
                json!({
                    "status": "exhausted",
                    "depth": stats.depth,
                    "generated": stats.generated,
                    "distinct": stats.distinct,
                    "bits_per_state": bits,
                    "wall_s": elapsed.as_secs_f64(),
                })
            },
        );
        let _ = writeln!(out, "{report}");
    }
    if !enabled {
        return None;
    }
    let (stats, _) = result?;
    Some(Outcome {
        verdict: Verdict::Success,
        stats,
        metrics: Metrics {
            probed: stats.generated,
            discarded_before_intern: stats.generated.saturating_sub(stats.distinct),
            explore_time: elapsed,
            store: store.stats(),
            store_memory: store.memory_usage(),
            ..Metrics::default()
        },
        warnings: Vec::new(),
        trace: None,
        failure: None,
        in_init: false,
        expanded: true,
        replayed: None,
        sampled: Vec::new(),
        printed: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ternary(lane: usize) -> Transition {
        let mut next = [DISABLED; (MAX_CELL + 1) as usize];
        next[..3].copy_from_slice(&[1, 2, 0]);
        Transition {
            lane,
            shift: if lane == 0 { 0 } else { 2 },
            next,
        }
    }

    #[test]
    fn packed_membership_retains_ordered_bfs_depth_and_duplicate_multiplicity() {
        let code = Code::default();
        let shape = Shape {
            code: &code,
            keys: Vec::new(),
            width: 2,
            mask: 3,
        };
        let actual = explore(
            &shape,
            &[0, 0],
            &[ternary(0), ternary(1), ternary(0)],
            &[0b111, 0b111],
            &|| false,
        );
        assert_eq!(
            actual,
            Some(Stats {
                depth: 5,
                distinct: 9,
                generated: 28,
                initial_generated: 1,
                left_on_queue: 0,
            })
        );
    }

    #[test]
    fn no_holding_result_escapes_a_late_shape_or_invariant_failure() {
        let code = Code::default();
        let shape = Shape {
            code: &code,
            keys: Vec::new(),
            width: 2,
            mask: 3,
        };
        let mut shape_miss = ternary(0);
        shape_miss.next[2] = DECLINE;
        assert_eq!(
            explore(&shape, &[0], &[shape_miss], &[0b111], &|| false),
            None
        );
        assert_eq!(
            explore(&shape, &[0], &[ternary(0)], &[0b11], &|| false),
            None
        );
        assert_eq!(
            explore(&shape, &[0], &[ternary(0)], &[0b111], &|| true),
            None
        );
    }
}
