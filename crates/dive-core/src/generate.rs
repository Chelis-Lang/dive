//! Successor and initial-state generation with TLC's `getNextStates` rules
//! (`05-successor-generation.md` §3, §4).
//!
//! The continuation list (TLC's `ActionItemList`) is a stack of nodes in the
//! machine, linked by index; a call pushes its items and truncates them on
//! return, so `\A` and `[A]_v` can run two continuations from one tail. The
//! successor buffer is `Machine::s1`, bound and unbound in place: each
//! emitted successor is a snapshot of it (§8.2). Bindings of frame slots
//! are saved and restored around every generator scope, so a continuation
//! always runs with the bindings it was created under.

use crate::Error;
use crate::eval::Machine;
use crate::handle::Handle;
use crate::node::Kind;
use crate::plan::{Bind, Code, G, GNode, Node, P};

const NONE: u32 = u32::MAX;

/// A pending item of the continuation list.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Item {
    /// A conjunct: its generator and its boolean form.
    Conj(G, P),
    /// The remaining variables of an `UNCHANGED` (a `GNode::Unchanged` or
    /// `GNode::StepBox`), from index `from`.
    Unch { g: G, from: u32 },
    /// An `\A` body under one later binding, whose slot values start at
    /// `vals` in `Machine::cvals`.
    Under { g: G, vals: u32 },
}

/// A node of the continuation stack.
#[derive(Clone, Copy, Debug)]
pub struct ContNode {
    item: Item,
    next: u32,
}

/// Where emitted states go.
pub trait Sink {
    /// Receives the successor in `m.s1` (complete or not). Returning an
    /// error of kind [`crate::ErrorKind::Stop`] ends the enumeration.
    ///
    /// # Errors
    /// Stop, or any error of the sink's own checks.
    fn emit(&mut self, m: &mut Machine, code: &Code) -> Result<(), Error>;
}

impl Machine {
    fn push_cont(&mut self, item: Item, next: u32) -> u32 {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "continuation stacks are small"
        )]
        let at = self.conts.len() as u32;
        self.conts.push(ContNode { item, next });
        at
    }

    fn save(&mut self, slots: impl Iterator<Item = u32>) -> usize {
        let mark = self.saved.len();
        for s in slots {
            let v = self.slot(s);
            self.saved.push(v);
        }
        mark
    }

    fn restore(&mut self, mark: usize, slots: impl Iterator<Item = u32>) {
        for (k, s) in slots.enumerate() {
            let v = self.saved.get(mark + k).copied().unwrap_or(Handle::EMPTY);
            self.set_slot(s, v);
        }
        self.saved.truncate(mark);
    }

    fn bind_var(&mut self, var: u16, v: Handle) {
        if let Some(x) = self.s1.get_mut(usize::from(var)) {
            *x = v;
            self.assigned += 1;
        }
    }

    fn unbind_var(&mut self, var: u16) {
        if let Some(x) = self.s1.get_mut(usize::from(var)) {
            *x = Handle::EMPTY;
            self.assigned -= 1;
        }
    }

    /// Clears the successor buffer.
    pub fn clear_buffer(&mut self) {
        self.s1.iter_mut().for_each(|x| *x = Handle::EMPTY);
        self.assigned = 0;
    }

    /// Runs one action (or initial predicate) from `self.s0`, emitting every
    /// successor into `sink`. The caller has entered the action's frame.
    ///
    /// # Errors
    /// Evaluation errors, refusals, and the sink's stop.
    pub fn run_action(&mut self, code: &Code, g: G, sink: &mut dyn Sink) -> Result<(), Error> {
        self.clear_buffer();
        let mark = self.conts.len();
        let r = self.gen_node(code, g, NONE, sink);
        self.conts.truncate(mark);
        r
    }

    fn cont(&mut self, code: &Code, acts: u32, sink: &mut dyn Sink) -> Result<(), Error> {
        if acts == NONE {
            return sink.emit(self, code);
        }
        if self.warn && self.assigned == self.s1.len() {
            return self.fast(code, acts, sink);
        }
        let Some(&c) = self.conts.get(acts as usize) else {
            return Err(Error::internal("continuation out of range"));
        };
        match c.item {
            Item::Conj(g, _) => self.gen_node(code, g, c.next, sink),
            Item::Unch { g, from } => self.unchanged(code, g, from, c.next, sink),
            Item::Under { g, vals } => {
                let binds = forall_binds(code, g)?;
                let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                let mark = self.save(slots.iter().copied());
                for (k, &s) in slots.iter().enumerate() {
                    let v = self
                        .cvals
                        .get(vals as usize + k)
                        .copied()
                        .unwrap_or(Handle::EMPTY);
                    self.set_slot(s, v);
                }
                let body = match code.gnodes.get(g as usize) {
                    Some(GNode::Forall { body, .. }) => *body,
                    _ => return Err(Error::internal("continuation of a non-\\A node")),
                };
                let r = self.gen_node(code, body, c.next, sink);
                self.restore(mark, slots.iter().copied());
                r
            }
        }
    }

    /// TLC's `getNextStatesAllAssigned`: the pending items are evaluated as
    /// booleans; an `UNCHANGED` item leaves the shortcut.
    fn fast(&mut self, code: &Code, acts: u32, sink: &mut dyn Sink) -> Result<(), Error> {
        let mut cur = acts;
        while cur != NONE {
            let Some(&c) = self.conts.get(cur as usize) else {
                return Err(Error::internal("continuation out of range"));
            };
            match c.item {
                Item::Conj(_, p) => {
                    if !self.eval_bool(code, p)? {
                        return Ok(());
                    }
                }
                Item::Unch { g, from } => return self.unchanged(code, g, from, c.next, sink),
                Item::Under { g, vals } => {
                    let (binds, body_p) = match code.gnodes.get(g as usize) {
                        Some(GNode::Forall { binds, body_p, .. }) => (binds, *body_p),
                        _ => return Err(Error::internal("continuation of a non-\\A node")),
                    };
                    let slots: Vec<u32> =
                        binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                    let mark = self.save(slots.iter().copied());
                    for (k, &s) in slots.iter().enumerate() {
                        let v = self
                            .cvals
                            .get(vals as usize + k)
                            .copied()
                            .unwrap_or(Handle::EMPTY);
                        self.set_slot(s, v);
                    }
                    let b = self.eval_bool(code, body_p);
                    self.restore(mark, slots.iter().copied());
                    if !b? {
                        return Ok(());
                    }
                }
            }
            cur = c.next;
        }
        sink.emit(self, code)
    }

    fn unchanged(
        &mut self,
        code: &Code,
        g: G,
        from: u32,
        rest: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let vars: &[u16] = match code.gnodes.get(g as usize) {
            Some(GNode::Unchanged(v) | GNode::StepBox { vars: v, .. }) => v,
            _ => return Err(Error::internal("UNCHANGED continuation of another node")),
        };
        let Some(&var) = vars.get(from as usize) else {
            return self.cont(code, rest, sink);
        };
        let mark = self.conts.len();
        let next = if (from as usize + 1) < vars.len() {
            self.push_cont(Item::Unch { g, from: from + 1 }, rest)
        } else {
            rest
        };
        let old = self
            .s0
            .get(usize::from(var))
            .copied()
            .unwrap_or(Handle::EMPTY);
        let cur = self
            .s1
            .get(usize::from(var))
            .copied()
            .unwrap_or(Handle::EMPTY);
        let r = if cur.is_empty() {
            self.bind_var(var, old);
            let r = self.cont(code, next, sink);
            self.unbind_var(var);
            r
        } else if self.heap.eq(cur, old) {
            self.cont(code, next, sink)
        } else {
            // TLC prints warning 2143 here; M0 does not report warnings.
            Ok(())
        };
        self.conts.truncate(mark);
        r
    }

    #[allow(clippy::too_many_lines, reason = "one arm per generator rule")]
    fn gen_node(&mut self, code: &Code, g: G, acts: u32, sink: &mut dyn Sink) -> Result<(), Error> {
        let Some(node) = code.gnodes.get(g as usize) else {
            return Err(Error::internal("generator node out of range"));
        };
        match node {
            GNode::Conj(items) => {
                let Some((&(first, _), rest)) = items.split_first() else {
                    return self.cont(code, acts, sink);
                };
                let mark = self.conts.len();
                let mut head = acts;
                for &(gi, pi) in rest.iter().rev() {
                    head = self.push_cont(Item::Conj(gi, pi), head);
                }
                let r = self.gen_node(code, first, head, sink);
                self.conts.truncate(mark);
                r
            }
            GNode::Disj(items) => {
                for &gi in items {
                    self.gen_node(code, gi, acts, sink)?;
                }
                Ok(())
            }
            GNode::Exists { binds, body, .. } => {
                let units = self.units(code, binds)?;
                if units.iter().any(|(_, e)| e.is_empty()) {
                    return Ok(());
                }
                let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                let mark = self.save(slots.iter().copied());
                let mut idx = vec![0usize; units.len()];
                let r = (|| {
                    loop {
                        for (k, (u, elems)) in units.iter().enumerate() {
                            self.bind_unit(binds, *u, elems[idx[k]])?;
                        }
                        self.gen_node(code, *body, acts, sink)?;
                        let mut k = 0;
                        loop {
                            if k == units.len() {
                                return Ok(());
                            }
                            idx[k] += 1;
                            if idx[k] < units[k].1.len() {
                                break;
                            }
                            idx[k] = 0;
                            k += 1;
                        }
                    }
                })();
                self.restore(mark, slots.iter().copied());
                r
            }
            GNode::Forall { binds, body, .. } => {
                // Every binding, first position fastest.
                let units = self.units(code, binds)?;
                let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                let mark = self.save(slots.iter().copied());
                let mut contexts: Vec<Vec<Handle>> = Vec::new();
                if !units.iter().any(|(_, e)| e.is_empty()) {
                    let mut idx = vec![0usize; units.len()];
                    'outer: loop {
                        for (k, (u, elems)) in units.iter().enumerate() {
                            self.bind_unit(binds, *u, elems[idx[k]])?;
                        }
                        contexts.push(slots.iter().map(|&s| self.slot(s)).collect());
                        let mut k = 0;
                        loop {
                            if k == units.len() {
                                break 'outer;
                            }
                            idx[k] += 1;
                            if idx[k] < units[k].1.len() {
                                break;
                            }
                            idx[k] = 0;
                            k += 1;
                        }
                    }
                }
                let r = if contexts.is_empty() {
                    self.cont(code, acts, sink)
                } else {
                    let cmark = self.conts.len();
                    let vmark = self.cvals.len();
                    let mut head = acts;
                    for ctx in contexts.iter().skip(1) {
                        #[allow(clippy::cast_possible_truncation, reason = "small value stacks")]
                        let at = self.cvals.len() as u32;
                        self.cvals.extend_from_slice(ctx);
                        head = self.push_cont(Item::Under { g, vals: at }, head);
                    }
                    for (&s, &v) in slots.iter().zip(&contexts[0]) {
                        self.set_slot(s, v);
                    }
                    let r = self.gen_node(code, *body, head, sink);
                    self.conts.truncate(cmark);
                    self.cvals.truncate(vmark);
                    r
                };
                self.restore(mark, slots.iter().copied());
                r
            }
            GNode::If { cond, then, other } => {
                if self.eval_bool(code, *cond)? {
                    self.gen_node(code, *then, acts, sink)
                } else {
                    self.gen_node(code, *other, acts, sink)
                }
            }
            GNode::Case { arms, other } => {
                for &(p, gi) in arms {
                    if self.eval_bool(code, p)? {
                        return self.gen_node(code, gi, acts, sink);
                    }
                }
                match other {
                    Some(o) => self.gen_node(code, *o, acts, sink),
                    None => Err(Error::eval(
                        "TLC encountered a CASE with no conditions true",
                    )),
                }
            }
            GNode::Implies { lhs, rhs } => {
                if self.eval_bool(code, *lhs)? {
                    self.gen_node(code, *rhs, acts, sink)
                } else {
                    self.cont(code, acts, sink)
                }
            }
            GNode::Assign { var, rhs } => {
                let r = self.eval_norm(code, *rhs)?;
                let cur = self
                    .s1
                    .get(usize::from(*var))
                    .copied()
                    .unwrap_or(Handle::EMPTY);
                if cur.is_empty() {
                    self.bind_var(*var, r);
                    let res = self.cont(code, acts, sink);
                    self.unbind_var(*var);
                    res
                } else if self.heap.eq(cur, r) {
                    self.cont(code, acts, sink)
                } else {
                    Ok(())
                }
            }
            GNode::AssignIn { var, set } => self.assign_in(code, *var, *set, acts, sink),
            GNode::Unchanged(_) => self.unchanged(code, g, 0, acts, sink),
            GNode::StepBox { action, .. } => {
                self.gen_node(code, *action, acts, sink)?;
                self.unchanged(code, g, 0, acts, sink)
            }
            GNode::Scope { reset, body } => {
                let mark = self.save(reset.iter().copied());
                for &s in reset {
                    self.set_slot(s, Handle::EMPTY);
                }
                let r = self.gen_node(code, *body, acts, sink);
                self.restore(mark, reset.iter().copied());
                r
            }
            GNode::Guard(p) => {
                if self.eval_bool(code, *p)? {
                    self.cont(code, acts, sink)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn assign_in(
        &mut self,
        code: &Code,
        var: u16,
        set: P,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let cur = self
            .s1
            .get(usize::from(var))
            .copied()
            .unwrap_or(Handle::EMPTY);
        // A filter is streamed: its bound set's elements, filtered, without
        // building the set.
        if cur.is_empty()
            && let Some(Node::Filter { bind, pred }) = code.nodes.get(set as usize)
        {
            let dom = self.eval(code, bind.domain)?;
            let dom = self.heap.norm(dom)?;
            let mut c = self.heap.cursor(dom)?;
            let mark = self.save(bind.slots.iter().copied());
            let r = (|| {
                while let Some(e) = c.next(&mut self.heap)? {
                    self.bind_one(bind, e)?;
                    if self.eval_bool(code, *pred)? {
                        self.bind_var(var, e);
                        let r = self.cont(code, acts, sink);
                        self.unbind_var(var);
                        r?;
                    }
                }
                Ok(())
            })();
            self.restore(mark, bind.slots.iter().copied());
            return r;
        }
        let s = self.eval(code, set)?;
        if !cur.is_empty() {
            return if self.member(cur, s)? {
                self.cont(code, acts, sink)
            } else {
                Ok(())
            };
        }
        if !s.is_compound() {
            return Err(Error::eval("The right side of \\IN is not a set"));
        }
        if !self.heap.is_finite(s) {
            return Err(Error::eval("The right side of \\IN is not enumerable"));
        }
        let mut c = self.heap.cursor(s)?;
        while let Some(e) = c.next(&mut self.heap)? {
            self.bind_var(var, e);
            let r = self.cont(code, acts, sink);
            self.unbind_var(var);
            r?;
        }
        Ok(())
    }
}

fn forall_binds(code: &Code, g: G) -> Result<&[Bind], Error> {
    match code.gnodes.get(g as usize) {
        Some(GNode::Forall { binds, .. }) => Ok(binds),
        _ => Err(Error::internal("continuation of a non-\\A node")),
    }
}

/// Whether a value is a set kind (for diagnostics).
#[must_use]
pub fn is_set_kind(k: Kind) -> bool {
    k.is_set()
}
