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
use crate::plan::{Bind, Code, G, GNode, Node, P, Raw, Unch};

mod constructive;

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
    /// `<<A>>_v`'s `CHANGED v`: `v = v'`, which must be false.
    Changed(P),
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

    /// Whether actual Next traversal uses constructive index ordering. This
    /// does not affect separately evaluated predicates or ENABLED queries.
    fn constructive(&self) -> bool {
        false
    }

    /// Chooses an index below the positive bound for constructive traversal.
    ///
    /// # Errors
    /// An ordinary sink has no random-index owner.
    fn generation_index(&mut self, _: u64) -> Result<u64, Error> {
        Err(Error::unsupported(
            "constructive generation has no index chooser",
        ))
    }
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

    /// Runs actual Next traversal with bounded randomized disjunction and
    /// existential ordering. The sink stops after its first admitted complete
    /// row; continuing searches remaining alternatives. `choose(n)` must return
    /// a value below the positive `n`. Ordinary predicate/ENABLED evaluation
    /// never inherits these operational choices.
    ///
    /// # Errors
    /// The ordinary generation errors, unsupported indexed domains or an
    /// out-of-range caller index.
    pub fn run_action_generation(
        &mut self,
        code: &Code,
        g: G,
        sink: &mut dyn Sink,
        choose: &mut dyn FnMut(u64) -> u64,
    ) -> Result<(), Error> {
        struct Constructive<'a> {
            sink: &'a mut dyn Sink,
            choose: &'a mut dyn FnMut(u64) -> u64,
        }
        impl Sink for Constructive<'_> {
            fn emit(&mut self, machine: &mut Machine, code: &Code) -> Result<(), Error> {
                self.sink.emit(machine, code)
            }
            fn constructive(&self) -> bool {
                true
            }
            fn generation_index(&mut self, bound: u64) -> Result<u64, Error> {
                Ok((self.choose)(bound))
            }
        }
        self.run_action(code, g, &mut Constructive { sink, choose })
    }

    /// `ENABLED A` in the current state `s0` (TLC's `Tool.enabled`): runs
    /// `A` with a fresh successor buffer and stops at the first successor,
    /// complete or not. The buffer and the continuation stack of an
    /// enclosing generation are saved and restored around it.
    ///
    /// # Errors
    /// Evaluation errors and refusals inside `A`.
    pub(crate) fn enabled(&mut self, code: &Code, g: G) -> Result<Handle, Error> {
        struct First(bool);
        impl Sink for First {
            fn emit(&mut self, _: &mut Machine, _: &Code) -> Result<(), Error> {
                self.0 = true;
                Err(Error::stop())
            }
        }
        let saved = std::mem::take(&mut self.s1);
        self.s1 = vec![Handle::EMPTY; saved.len()];
        let assigned = self.assigned;
        let mut first = First(false);
        let r = self.run_action(code, g, &mut first);
        self.s1 = saved;
        self.assigned = assigned;
        match r {
            Ok(()) => Ok(Handle::bool(first.0)),
            Err(e) if first.0 && e.kind() == crate::ErrorKind::Stop => Ok(Handle::TRUE),
            Err(e) => Err(e),
        }
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
            Item::Changed(same) => {
                if self.changed(code, same)? {
                    self.cont(code, c.next, sink)
                } else {
                    Ok(())
                }
            }
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
                Item::Changed(same) => {
                    if !self.changed(code, same)? {
                        return Ok(());
                    }
                }
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

    /// TLC's `CHANGED` item: `v` evaluated in `s0` and in `s1` (as the
    /// current state) differ.
    fn changed(&mut self, code: &Code, same: P) -> Result<bool, Error> {
        Ok(!self.eval_bool(code, same)?)
    }

    fn unchanged(
        &mut self,
        code: &Code,
        g: G,
        from: u32,
        rest: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let items: &[Unch] = match code.gnodes.get(g as usize) {
            Some(GNode::Unchanged(v) | GNode::StepBox { vars: v, .. }) => v,
            _ => return Err(Error::internal("UNCHANGED continuation of another node")),
        };
        let Some(&item) = items.get(from as usize) else {
            return self.cont(code, rest, sink);
        };
        let mark = self.conts.len();
        let next = if (from as usize + 1) < items.len() {
            self.push_cont(Item::Unch { g, from: from + 1 }, rest)
        } else {
            rest
        };
        let r = match item {
            Unch::Var { var, at } => {
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
                if cur.is_empty() {
                    self.bind_var(var, old);
                    let r = self.cont(code, next, sink);
                    self.unbind_var(var);
                    r
                } else if self.demand_equals(cur, old)? {
                    self.cont(code, next, sink)
                } else {
                    // TLC prints warning 2143 here (`Tool.java:1676`).
                    if !self.warnings.contains(&(var, at)) {
                        self.warnings.push((var, at));
                    }
                    Ok(())
                }
            }
            // `verifyUnchanged`: `e` in `s0` and in `s1` as the current
            // state.
            Unch::Verify(same) => {
                if self.eval_bool(code, same)? {
                    self.cont(code, next, sink)
                } else {
                    Ok(())
                }
            }
        };
        self.conts.truncate(mark);
        r
    }

    fn gen_node(&mut self, code: &Code, g: G, acts: u32, sink: &mut dyn Sink) -> Result<(), Error> {
        let Some(observer) = &self.debugger else {
            return self.gen_node_inner(code, g, acts, sink);
        };
        let instruction = crate::debug::Instruction::Generator(g);
        let guard = crate::debug::Guard::enter(observer, instruction, code, self)?;
        let result = self.gen_node_inner(code, g, acts, sink);
        let returned = match &result {
            Ok(()) => crate::debug::Returned::Generated,
            Err(error) => crate::debug::Returned::Failed(error),
        };
        guard.returned(instruction, code, self, returned)?;
        result
    }

    #[allow(clippy::too_many_lines, reason = "one arm per generator rule")]
    fn gen_node_inner(
        &mut self,
        code: &Code,
        g: G,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        if self.fuel != u64::MAX {
            self.charge(1)?;
        }
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
                if sink.constructive() {
                    return self.generation_disjunction(code, items, acts, sink);
                }
                for &gi in items {
                    self.gen_node(code, gi, acts, sink)?;
                }
                Ok(())
            }
            GNode::Exists { binds, body, .. } => {
                self.charge(binds.iter().map(|b| b.slots.len()).sum())?;
                let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                let mark = self.save(slots.iter().copied());
                let r = if sink.constructive() {
                    self.generation_exists(code, binds, *body, acts, sink)
                } else {
                    self.for_each_binding(code, binds, &mut |machine| {
                        machine.gen_node(code, *body, acts, sink)?;
                        Ok(true)
                    })
                    .map(|_| ())
                };
                self.restore(mark, slots.iter().copied());
                r
            }
            GNode::Forall { binds, body, .. } => {
                self.charge(binds.iter().map(|b| b.slots.len()).sum())?;
                let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                let mark = self.save(slots.iter().copied());
                let r = (|| {
                    let mut contexts: Vec<Vec<Handle>> = Vec::new();
                    self.for_each_binding(code, binds, &mut |machine| {
                        machine.charge(slots.len() + 1)?;
                        contexts.push(slots.iter().map(|&s| machine.slot(s)).collect());
                        Ok(true)
                    })?;
                    if contexts.is_empty() {
                        return self.cont(code, acts, sink);
                    }
                    let cmark = self.conts.len();
                    let vmark = self.cvals.len();
                    let result = (|| {
                        let mut head = acts;
                        for ctx in contexts.iter().skip(1) {
                            self.charge(ctx.len() + 1)?;
                            #[allow(
                                clippy::cast_possible_truncation,
                                reason = "small value stacks"
                            )]
                            let at = self.cvals.len() as u32;
                            self.cvals.extend_from_slice(ctx);
                            head = self.push_cont(Item::Under { g, vals: at }, head);
                        }
                        for (&s, &v) in slots.iter().zip(&contexts[0]) {
                            self.set_slot(s, v);
                        }
                        self.gen_node(code, *body, head, sink)
                    })();
                    self.conts.truncate(cmark);
                    self.cvals.truncate(vmark);
                    result
                })();
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
                if sink.constructive() {
                    return Err(Error::unsupported(
                        "constructive generation of CASE actions",
                    ));
                }
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
                let r = self.eval_component(code, *rhs)?;
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
                } else if self.demand_equals(cur, r)? {
                    self.cont(code, acts, sink)
                } else {
                    Ok(())
                }
            }
            GNode::AssignIn { var, set } => self.assign_in(code, *var, *set, acts, sink),
            GNode::AssignInRaw { var, set, raw } => {
                self.assign_in_raw(code, *var, *set, raw, acts, sink)
            }
            GNode::Unchanged(_) => self.unchanged(code, g, 0, acts, sink),
            GNode::StepBox { action, .. } => {
                self.gen_node(code, *action, acts, sink)?;
                self.unchanged(code, g, 0, acts, sink)
            }
            GNode::StepAngle { action, same } => {
                let mark = self.conts.len();
                let head = self.push_cont(Item::Changed(*same), acts);
                let r = self.gen_node(code, *action, head, sink);
                self.conts.truncate(mark);
                r
            }
            GNode::FcnAction { binds, arg, body } => {
                self.fcn_action(code, binds, *arg, *body, acts, sink)
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
            // Enumerated without normalizing, so that a symbolic domain's
            // candidates are built one at a time.
            let dom = self.eval(code, bind.domain)?;
            let mut c = self.work_cursor(dom)?;
            let mark = self.save(bind.slots.iter().copied());
            let r = (|| {
                loop {
                    let before = self.heap.mark();
                    let Some(e) = self.work_next(&mut c)? else {
                        break;
                    };
                    let forced = self.stats.transient_thunks;
                    self.bind_one(bind, e)?;
                    if self.eval_bool(code, *pred)? {
                        self.bind_var(var, e);
                        let r = self.cont(code, acts, sink);
                        self.unbind_var(var);
                        r?;
                    }
                    // A rejected candidate is freed (R10; as in
                    // `Machine::filter`), and so is an accepted one once its
                    // continuation has emitted its states (#551).
                    if self.stats.transient_thunks == forced {
                        self.heap.rollback(before);
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
        let mut c = self.work_cursor(s)?;
        self.each_in(&mut c, code, var, acts, sink)
    }
}

impl Machine {
    /// Binds `var` to each element of `c` in turn and runs the
    /// continuation. An element and the continuation's transients are freed
    /// once it returns (R10, #551): an `Init` over a large set would
    /// otherwise hold every candidate state until the level ends. Only when
    /// no thunk was forced to a transient meanwhile, whose slot may hold a
    /// value built after the mark (`03-value-model.md` §11.5 rule 2); the
    /// cursor's own state predates the mark.
    fn each_in(
        &mut self,
        c: &mut crate::heap::Cursor,
        code: &Code,
        var: u16,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        loop {
            let before = self.heap.mark();
            let Some(e) = self.work_next(c)? else {
                return Ok(());
            };
            let forced = self.stats.transient_thunks;
            self.bind_var(var, e);
            let r = self.cont(code, acts, sink);
            self.unbind_var(var);
            r?;
            if self.stats.transient_thunks == forced {
                self.heap.rollback(before);
            }
        }
    }

    /// `x' \in S` over a set whose TLC `elements()` may repeat elements
    /// (§6): TLC's multiplicity is computed, and the run refuses when it
    /// differs from the normalized set's, because which one TLC walks
    /// depends on the set object's history. Otherwise the normalized set
    /// is enumerated, in the run's value order (D-010).
    fn assign_in_raw(
        &mut self,
        code: &Code,
        var: u16,
        set: P,
        raw: &Raw,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let cur = self
            .s1
            .get(usize::from(var))
            .copied()
            .unwrap_or(Handle::EMPTY);
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
        let elems = self.raw_elems(code, raw)?;
        let listed = elems.len() as u64;
        let distinct = self.work_cardinality(s)?;
        if listed != distinct {
            return Err(Error::unsupported(format!(
                "x' \\in a lazily represented set: TLC's unnormalized enumeration of it ({listed} elements) differs from its normalized one ({distinct} elements), and which one TLC uses depends on whether it has already compared, sized or fingerprinted the same set object"
            )));
        }
        // Under TLC's exploration order the order is observable too (A6):
        // the parts' sequence must be the normalized order.
        if self.heap.store().semantics().tlc_exploration
            && elems
                .windows(2)
                .any(|w| self.heap.cmp(w[0], w[1]) != std::cmp::Ordering::Less)
        {
            return Err(Error::unsupported(
                "x' \\in a lazily represented set under --compat=tlc: TLC's unnormalized enumeration of it lists the elements in another order than its normalized one, and which one TLC uses depends on the set object's history (A6)",
            ));
        }
        let mut c = self.work_cursor(s)?;
        self.each_in(&mut c, code, var, acts, sink)
    }

    fn finite_elems(&mut self, h: Handle) -> Result<Vec<Handle>, Error> {
        if !h.is_compound() || !self.heap.view(h).kind.is_set() {
            return Err(Error::eval("enumerating a value that is not a set"));
        }
        if !self.heap.is_finite(h) {
            return Err(Error::unsupported(
                "TLC's enumeration of a lazily represented set with an infinite part (core)",
            ));
        }
        let n = self.work_norm(h)?;
        self.work_elements(n)
    }

    fn distinct(&mut self, elems: &[Handle]) -> usize {
        let mut v = elems.to_vec();
        let set = self.heap.set_from(&mut v);
        self.heap.view(set).len()
    }

    /// TLC's `elements()` sequence of a lazily represented set, repeats
    /// included (the order within a part is normalized; only the length is
    /// observable here).
    #[allow(clippy::too_many_lines, reason = "one arm per lazy set class")]
    fn raw_elems(&mut self, code: &Code, raw: &Raw) -> Result<Vec<Handle>, Error> {
        match raw {
            Raw::Leaf(p) => {
                let h = self.eval(code, *p)?;
                self.finite_elems(h)
            }
            Raw::Cup(a, b) => {
                let mut x = self.raw_elems(code, a)?;
                x.extend(self.raw_elems(code, b)?);
                Ok(x)
            }
            Raw::Union(items) => {
                // The enumerated set of sets is normalized first: equal
                // members are listed once.
                let mut members: Vec<(Handle, Vec<Handle>)> = Vec::new();
                for r in items {
                    let elems = self.raw_elems(code, r)?;
                    let mut v = elems.clone();
                    let set = self.heap.set_from(&mut v);
                    if let Some((_, other)) = members.iter().find(|(m, _)| self.heap.eq(*m, set)) {
                        if other.len() != elems.len() {
                            return Err(Error::unsupported(
                                "UNION of equal sets that TLC enumerates differently (core)",
                            ));
                        }
                        continue;
                    }
                    members.push((set, elems));
                }
                Ok(members.into_iter().flat_map(|(_, e)| e).collect())
            }
            Raw::UnionOf(p) => {
                let outer = self.eval(code, *p)?;
                let members = self.finite_elems(outer)?;
                if members.len() == 1 {
                    // A lone member is never compared while the outer set
                    // is normalized, so TLC may still hold it lazily.
                    return Err(Error::unsupported(
                        "UNION of a one-member set whose member TLC may represent lazily (core)",
                    ));
                }
                let mut x = Vec::new();
                for m in members {
                    x.extend(self.finite_elems(m)?);
                }
                Ok(x)
            }
            Raw::Filter { bind, pred, inner } => {
                let elems = self.raw_elems(code, inner)?;
                let mark = self.save(bind.slots.iter().copied());
                let r = (|| {
                    let mut out = Vec::new();
                    for e in elems {
                        self.bind_one(bind, e)?;
                        if self.eval_bool(code, *pred)? {
                            out.push(e);
                        }
                    }
                    Ok(out)
                })();
                self.restore(mark, bind.slots.iter().copied());
                r
            }
            Raw::Cap(ra, rb, pa, pb) => {
                let a = self.eval(code, *pa)?;
                let (walk, other) = if self.heap.is_finite(a) {
                    (ra, *pb)
                } else {
                    (rb, *pa)
                };
                let elems = self.raw_elems(code, walk)?;
                let o = self.eval(code, other)?;
                let mut out = Vec::new();
                for e in elems {
                    if self.member(e, o)? {
                        out.push(e);
                    }
                }
                Ok(out)
            }
            Raw::Minus(ra, pb) => {
                let elems = self.raw_elems(code, ra)?;
                let o = self.eval(code, *pb)?;
                let mut out = Vec::new();
                for e in elems {
                    if !self.member(e, o)? {
                        out.push(e);
                    }
                }
                Ok(out)
            }
            Raw::Odometer(parts, p) => {
                let mut repeats = false;
                for r in parts {
                    let elems = self.raw_elems(code, r)?;
                    repeats |= self.distinct(&elems) != elems.len();
                }
                let h = self.eval(code, *p)?;
                let elems = self.finite_elems(h)?;
                if repeats && !elems.is_empty() {
                    return Err(Error::unsupported(
                        "x' \\in a set of functions, records or tuples over a lazily represented set whose enumeration repeats elements (core)",
                    ));
                }
                Ok(elems)
            }
        }
    }

    /// `f[a]` as an action over an unmaterialized function constructor
    /// (`Tool.java:1263-1289`, `getFcnContext` `:3997-4074`): the domains
    /// are evaluated, then the argument; the argument is checked against
    /// the domains and bound to the parameters; the body is enumerated.
    fn fcn_action(
        &mut self,
        code: &Code,
        binds: &[Bind],
        arg: P,
        body: G,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let mut domains = Vec::with_capacity(binds.len());
        for b in binds {
            domains.push(self.eval(code, b.domain)?);
        }
        let a = self.eval(code, arg)?;
        let slots: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
        let mark = self.save(slots.iter().copied());
        let r = (|| {
            let params: usize = binds
                .iter()
                .map(|b| if b.tuple { 1 } else { b.slots.len() })
                .sum();
            if params == 1 {
                let (Some(b), Some(&d)) = (binds.first(), domains.first()) else {
                    return Err(Error::internal("a function action without parameters"));
                };
                if !self.member(a, d)? {
                    return Err(Error::eval(
                        "In applying a function, the argument is not in its domain",
                    ));
                }
                self.bind_one(b, a)?;
            } else {
                let elems = self.tuple_elems(a).ok_or_else(|| {
                    Error::eval("Attempted to apply a function to an argument not in its domain")
                })?;
                let mut k = 0usize;
                for (b, &d) in binds.iter().zip(&domains) {
                    if b.tuple {
                        let e = *elems.get(k).ok_or_else(|| {
                            Error::eval("a function argument has too few components")
                        })?;
                        k += 1;
                        if !self.member(e, d)? {
                            return Err(Error::eval(
                                "In applying a function, an argument is not in its domain",
                            ));
                        }
                        self.bind_one(b, e)?;
                    } else {
                        for &s in &b.slots {
                            let e = *elems.get(k).ok_or_else(|| {
                                Error::eval("a function argument has too few components")
                            })?;
                            k += 1;
                            if !self.member(e, d)? {
                                return Err(Error::eval(
                                    "In applying a function, an argument is not in its domain",
                                ));
                            }
                            self.set_slot(s, e);
                        }
                    }
                }
            }
            self.gen_node(code, body, acts, sink)
        })();
        self.restore(mark, slots.iter().copied());
        r
    }

    /// The components of a tuple (TLC's `toTuple`: a function over
    /// `1..n`).
    fn tuple_elems(&self, h: Handle) -> Option<Vec<Handle>> {
        if !h.is_compound() {
            return None;
        }
        let v = self.heap.view(h);
        (v.kind == crate::node::Kind::IntFcn && (v.lo == 1 || v.data.is_empty()))
            .then(|| v.data.to_vec())
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
