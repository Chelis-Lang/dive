//! The interpreter for value nodes (`04-plan-compiler.md` §6): a recursive
//! `match` over the plan's node array, over one flat frame per activation.

use std::sync::Arc;

use crate::Error;
use crate::compare;
use crate::handle::Handle;
use crate::heap::Heap;
use crate::memo::{MAX_KEY, Memo};
use crate::node::Kind;
use crate::opts::Opts;
use crate::plan::{Arith, Bind, Cmp, Code, Node, P, Quant, SetOp, StdFn, Update};
use crate::store::Store;

/// The deepest recursive-operator nesting the interpreter allows.
const MAX_CALL_DEPTH: usize = 2000;

/// Evaluation counters.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvalStats {
    /// Thunks forced.
    pub forced: u64,
    /// Thunk reads answered from the cache.
    pub cached: u64,
    /// Forced thunks whose value is transient (an arena rollback must not
    /// free it while its slot holds it).
    pub transient_thunks: u64,
}

/// One worker's evaluation state.
#[derive(Debug)]
pub struct Machine {
    /// The value regions.
    pub heap: Heap,
    /// The frame stack.
    pub frame: Vec<Handle>,
    /// The base of the current frame.
    pub base: usize,
    /// The current state (`s0`).
    pub s0: Vec<Handle>,
    /// The successor buffer (`s1`); [`Handle::EMPTY`] is unassigned.
    pub s1: Vec<Handle>,
    /// Assigned variables in `s1`.
    pub assigned: usize,
    /// The strategy switches.
    pub opts: Opts,
    /// The memo tables.
    pub memo: Memo,
    /// Counters.
    pub stats: EvalStats,
    /// Whether the all-assigned shortcut runs for successors (TLC's
    /// `TLCGlobals.warn`; on in both profiles).
    pub warn: bool,
    /// Warnings 2143 raised by the generator (`UNCHANGED` of a variable
    /// already assigned another value), once per variable and site (an
    /// `ExprId` index), in the order raised.
    pub warnings: Vec<(u16, u32)>,
    pub(crate) conts: Vec<crate::generate::ContNode>,
    pub(crate) cvals: Vec<Handle>,
    pub(crate) saved: Vec<Handle>,
    scratch: Vec<Vec<Handle>>,
    depth: usize,
    /// Loop iterations left before evaluation gives up. Unbounded at run
    /// time; the compiler bounds it while folding constants (R9).
    pub(crate) fuel: u64,
}

fn eval_err(what: &str) -> Error {
    Error::eval(what)
}

/// An error raised inside `Node::Spec`: an evaluation error TLC might not
/// raise, because TLC would not have evaluated the expression yet.
fn speculative(e: Error) -> Error {
    if e.kind() == crate::ErrorKind::Eval {
        Error::unsupported(format!(
            "an evaluation error in an expression TLC evaluates lazily, where it may not fail: {}",
            e.message()
        ))
    } else {
        e
    }
}

fn int_of(h: Handle) -> Result<i64, Error> {
    h.as_int().ok_or_else(|| eval_err("expected an integer"))
}

fn bool_of(h: Handle) -> Result<bool, Error> {
    h.as_bool().ok_or_else(|| eval_err("expected a boolean"))
}

fn mk_int(v: i64) -> Result<Handle, Error> {
    Handle::int(v).ok_or_else(|| Error::unsupported("integer outside the M0 core's 61-bit range"))
}

impl Machine {
    /// A machine over `store` for states of `nvars` variables.
    #[must_use]
    pub fn new(store: Arc<Store>, nvars: usize, opts: Opts, memo: Memo) -> Self {
        Self {
            heap: Heap::new(store, !opts.arena, !opts.delta_hash),
            frame: Vec::new(),
            base: 0,
            s0: vec![Handle::EMPTY; nvars],
            s1: vec![Handle::EMPTY; nvars],
            assigned: 0,
            opts,
            memo,
            stats: EvalStats::default(),
            warn: true,
            warnings: Vec::new(),
            conts: Vec::new(),
            cvals: Vec::new(),
            saved: Vec::new(),
            scratch: Vec::new(),
            depth: 0,
            fuel: u64::MAX,
        }
    }

    /// Spends `n` units of fuel: one per element a loop visits.
    #[inline]
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        let n = n as u64;
        if self.fuel < n {
            self.fuel = 0;
            return Err(Error::unsupported("the constant-folding budget"));
        }
        self.fuel -= n;
        Ok(())
    }

    fn take_scratch(&mut self) -> Vec<Handle> {
        let mut v = self.scratch.pop().unwrap_or_default();
        v.clear();
        v
    }

    fn give_scratch(&mut self, v: Vec<Handle>) {
        self.scratch.push(v);
    }

    /// Enters a frame of `size` slots for a role, returning the old base.
    pub fn enter(&mut self, size: u32) -> usize {
        let old = self.base;
        self.base = self.frame.len();
        self.frame.resize(self.base + size as usize, Handle::EMPTY);
        old
    }

    /// Leaves the frame entered by [`Machine::enter`].
    pub fn leave(&mut self, old: usize) {
        self.frame.truncate(self.base);
        self.base = old;
    }

    #[inline]
    pub(crate) fn slot(&self, s: u32) -> Handle {
        self.frame
            .get(self.base + s as usize)
            .copied()
            .unwrap_or(Handle::EMPTY)
    }

    #[inline]
    pub(crate) fn set_slot(&mut self, s: u32, v: Handle) {
        if let Some(x) = self.frame.get_mut(self.base + s as usize) {
            *x = v;
        }
    }

    /// Evaluates a node to a boolean.
    ///
    /// # Errors
    /// Evaluation errors, or a non-boolean result.
    pub fn eval_bool(&mut self, code: &Code, p: P) -> Result<bool, Error> {
        let v = self.eval(code, p)?;
        bool_of(v)
    }

    /// Evaluates a node and normalizes the result.
    ///
    /// # Errors
    /// Evaluation errors.
    pub fn eval_norm(&mut self, code: &Code, p: P) -> Result<Handle, Error> {
        let v = self.eval(code, p)?;
        self.heap.norm(v)
    }

    /// Evaluates a node.
    ///
    /// # Errors
    /// Evaluation errors, refusals and resource limits.
    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per node kind, with operand names as in the semantics"
    )]
    pub fn eval(&mut self, code: &Code, p: P) -> Result<Handle, Error> {
        let Some(node) = code.nodes.get(p as usize) else {
            return Err(Error::internal("plan node out of range"));
        };
        match node {
            Node::Const(h) => Ok(*h),
            Node::Slot(s) => {
                let v = self.slot(*s);
                if v.is_empty() {
                    return Err(Error::internal("read of an unbound slot"));
                }
                Ok(v)
            }
            Node::Lazy { slot, body } => {
                let v = self.slot(*slot);
                if !v.is_empty() {
                    self.stats.cached += 1;
                    return Ok(v);
                }
                self.stats.forced += 1;
                let v = self.eval(code, *body)?;
                self.stats.transient_thunks += u64::from(v.is_transient());
                self.set_slot(*slot, v);
                Ok(v)
            }
            Node::Var(i) => {
                let v = self
                    .s0
                    .get(usize::from(*i))
                    .copied()
                    .unwrap_or(Handle::EMPTY);
                if v.is_empty() {
                    return Err(eval_err("read of an unassigned variable"));
                }
                Ok(v)
            }
            Node::Primed(i) => {
                let v = self
                    .s1
                    .get(usize::from(*i))
                    .copied()
                    .unwrap_or(Handle::EMPTY);
                if v.is_empty() {
                    return Err(eval_err("read of an unassigned primed variable"));
                }
                Ok(v)
            }
            Node::Not(a) => Ok(Handle::bool(!self.eval_bool(code, *a)?)),
            Node::And(items) => {
                for &i in items {
                    if !self.eval_bool(code, i)? {
                        return Ok(Handle::FALSE);
                    }
                }
                Ok(Handle::TRUE)
            }
            Node::Or(items) => {
                for &i in items {
                    if self.eval_bool(code, i)? {
                        return Ok(Handle::TRUE);
                    }
                }
                Ok(Handle::FALSE)
            }
            Node::Implies(a, b) => {
                if self.eval_bool(code, *a)? {
                    Ok(Handle::bool(self.eval_bool(code, *b)?))
                } else {
                    Ok(Handle::TRUE)
                }
            }
            Node::Equiv(a, b) => {
                let x = self.eval_bool(code, *a)?;
                let y = self.eval_bool(code, *b)?;
                Ok(Handle::bool(x == y))
            }
            Node::If(c, a, b) => {
                if self.eval_bool(code, *c)? {
                    self.eval(code, *a)
                } else {
                    self.eval(code, *b)
                }
            }
            Node::Case(arms, other) => {
                for &(g, v) in arms {
                    if self.eval_bool(code, g)? {
                        return self.eval(code, v);
                    }
                }
                match other {
                    Some(o) => self.eval(code, *o),
                    None => Err(eval_err("TLC encountered a CASE with no conditions true")),
                }
            }
            Node::Eq(a, b) => {
                let x = self.eval_norm(code, *a)?;
                let y = self.eval_norm(code, *b)?;
                Ok(Handle::bool(compare::equals(&self.heap, x, y)?))
            }
            Node::Neq(a, b) => {
                let x = self.eval_norm(code, *a)?;
                let y = self.eval_norm(code, *b)?;
                Ok(Handle::bool(!compare::equals(&self.heap, x, y)?))
            }
            Node::In(a, s) => {
                let x = self.eval_norm(code, *a)?;
                let set = self.eval(code, *s)?;
                Ok(Handle::bool(self.member(x, set)?))
            }
            Node::NotIn(a, s) => {
                let x = self.eval_norm(code, *a)?;
                let set = self.eval(code, *s)?;
                Ok(Handle::bool(!self.member(x, set)?))
            }
            Node::Subseteq(a, b) => {
                let x = self.eval(code, *a)?;
                let x = self.heap.norm(x)?;
                let y = self.eval(code, *b)?;
                let elems = self.heap.elements(x)?;
                self.charge(elems.len())?;
                for e in elems {
                    if !self.member(e, y)? {
                        return Ok(Handle::FALSE);
                    }
                }
                Ok(Handle::TRUE)
            }
            Node::Arith(op, a, b) => {
                let x = int_of(self.eval(code, *a)?)?;
                let y = int_of(self.eval(code, *b)?)?;
                arith(*op, x, y)
            }
            Node::Cmp(op, a, b) => {
                let x = int_of(self.eval(code, *a)?)?;
                let y = int_of(self.eval(code, *b)?)?;
                Ok(Handle::bool(match op {
                    Cmp::Lt => x < y,
                    Cmp::Le => x <= y,
                    Cmp::Gt => x > y,
                    Cmp::Ge => x >= y,
                }))
            }
            Node::Neg(a) => {
                let x = int_of(self.eval(code, *a)?)?;
                mk_int(
                    x.checked_neg()
                        .ok_or_else(|| Error::unsupported("integer overflow"))?,
                )
            }
            Node::Range(a, b) => {
                let x = int_of(self.eval(code, *a)?)?;
                let y = int_of(self.eval(code, *b)?)?;
                Ok(self.heap.interval(x, y))
            }
            Node::SetEnum(items) => {
                let mut v = self.take_scratch();
                for &i in items {
                    match self.eval_norm(code, i) {
                        Ok(e) => v.push(e),
                        Err(e) => {
                            self.give_scratch(v);
                            return Err(e);
                        }
                    }
                }
                let r = self.heap.set_from(&mut v);
                self.give_scratch(v);
                self.checked_set(r)
            }
            Node::SetBin(op, a, b) => {
                let x = self.eval(code, *a)?;
                let y = self.eval(code, *b)?;
                self.set_bin(*op, x, y)
            }
            Node::Powerset(a) => {
                let x = self.eval(code, *a)?;
                Ok(self.heap.symbolic(Kind::Subset, 0, &[x]))
            }
            Node::BigUnion(a) => {
                let x = self.eval_norm(code, *a)?;
                let members = self.heap.elements(x)?;
                let mut out = Vec::new();
                for m in members {
                    let m = self.heap.norm(m)?;
                    let es = self.heap.elements(m)?;
                    self.charge(es.len())?;
                    out.extend(es);
                }
                let r = self.heap.set_from(&mut out);
                self.checked_set(r)
            }
            Node::Product(items) => {
                let mut v = Vec::with_capacity(items.len());
                for &i in items {
                    v.push(self.eval(code, i)?);
                }
                Ok(self.heap.symbolic(Kind::Product, 0, &v))
            }
            Node::FcnSet(d, r) => {
                let x = self.eval_norm(code, *d)?;
                let y = self.eval(code, *r)?;
                Ok(self.heap.symbolic(Kind::FcnSet, 0, &[x, y]))
            }
            Node::RecSet(fields) => {
                let mut v: Vec<Handle> = fields.iter().map(|f| f.0).collect();
                for &(_, s) in fields {
                    v.push(self.eval(code, s)?);
                }
                Ok(self.heap.symbolic(Kind::RecSet, 0, &v))
            }
            Node::Filter { bind, pred } => self.filter(code, bind, *pred),
            Node::Map { binds, body } => {
                let mut out = Vec::new();
                self.for_each_binding(code, binds, &mut |m: &mut Self| {
                    let v = m.eval_norm(code, *body)?;
                    out.push(v);
                    Ok(true)
                })?;
                let r = self.heap.set_from(&mut out);
                self.checked_set(r)
            }
            Node::FcnCons { binds, body } => self.fcn_cons(code, binds, *body),
            Node::Quant { kind, binds, body } => self.quant(code, *kind, binds, *body),
            Node::Apply(f, a) => {
                let fv = self.eval(code, *f)?;
                let av = self.eval_norm(code, *a)?;
                self.apply(fv, av)
            }
            Node::Select(r, field) => {
                let rv = self.eval(code, *r)?;
                self.select(rv, *field)
            }
            Node::Domain(f) => {
                let fv = self.eval(code, *f)?;
                self.domain(fv)
            }
            Node::Except { base, updates } => {
                let mut f = self.eval_norm(code, *base)?;
                for u in updates {
                    f = self.except(code, f, u, 0)?;
                }
                Ok(f)
            }
            Node::Tuple(items) => {
                let mut v = self.take_scratch();
                for &i in items {
                    match self.eval_norm(code, i) {
                        Ok(e) => v.push(e),
                        Err(e) => {
                            self.give_scratch(v);
                            return Err(e);
                        }
                    }
                }
                let r = self.heap.tuple(&v);
                self.give_scratch(v);
                Ok(r)
            }
            Node::Record(fields) => {
                let mut keys = self.take_scratch();
                let mut vals = self.take_scratch();
                for &(k, e) in fields {
                    keys.push(k);
                    match self.eval_norm(code, e) {
                        Ok(v) => vals.push(v),
                        Err(e) => {
                            self.give_scratch(keys);
                            self.give_scratch(vals);
                            return Err(e);
                        }
                    }
                }
                let r = self.heap.fcn_sorted(&keys, &vals);
                self.give_scratch(keys);
                self.give_scratch(vals);
                Ok(r)
            }
            Node::Scope { reset, body } => {
                for &s in reset {
                    self.set_slot(s, Handle::EMPTY);
                }
                self.eval(code, *body)
            }
            Node::Call { fun, args } => self.call(code, *fun, args),
            Node::Std(f, args) => self.std(code, *f, args),
            Node::Memo { site, body } => self.memo_eval(code, *site, *body),
            Node::Spec(b) => self.eval(code, *b).map_err(speculative),
            Node::Beta { chain, fc, arg } => self.beta(code, *chain, *fc, *arg),
            Node::DomCheck { slot, domain, body } => {
                let d = self.eval(code, *domain).map_err(speculative)?;
                let x = self.slot(*slot);
                if !self.member(x, d)? {
                    return Err(eval_err(
                        "In applying a recursive function, the argument is not in its domain",
                    ));
                }
                self.eval(code, *body)
            }
            Node::InLazy { elem, set, neg } => {
                let x = self.eval_norm(code, *elem)?;
                let r = self.member_lazy(code, x, *set)?;
                Ok(Handle::bool(r != *neg))
            }
        }
    }

    /// `Node::Beta`: TLC's `FcnLambdaValue.apply` on a function constructor
    /// with one bound variable. The function's domain is evaluated first
    /// (TLC builds the lazy function before evaluating the argument), then
    /// the argument; the argument must be in the domain.
    fn beta(&mut self, code: &Code, chain: P, fc: P, arg: P) -> Result<Handle, Error> {
        match code.nodes.get(chain as usize) {
            Some(Node::Lazy { slot, body }) => {
                let v = self.slot(*slot);
                if v.is_empty() {
                    self.beta(code, *body, fc, arg)
                } else {
                    let a = self.eval_norm(code, arg)?;
                    self.apply(v, a)
                }
            }
            Some(Node::Scope { reset, body }) => {
                for &s in reset {
                    self.set_slot(s, Handle::EMPTY);
                }
                self.beta(code, *body, fc, arg)
            }
            Some(Node::Spec(b)) => self.beta(code, *b, fc, arg),
            Some(Node::FcnCons { binds, body }) if chain == fc => {
                let (Some(bind), Some(&slot)) =
                    (binds.first(), binds.first().and_then(|b| b.slots.first()))
                else {
                    return Err(Error::internal(
                        "a lazy application without a bound variable",
                    ));
                };
                let dom = self.eval(code, bind.domain)?;
                let x = self.eval_norm(code, arg)?;
                if !self.member(x, dom)? {
                    return Err(eval_err(
                        "In applying a function, the argument is not in its domain",
                    ));
                }
                let old = self.slot(slot);
                self.set_slot(slot, x);
                let out = self.eval(code, *body);
                self.set_slot(slot, old);
                out
            }
            _ => Err(Error::internal("a lazy application of a non-function node")),
        }
    }

    /// Membership in a set node TLC keeps lazy (`Node::InLazy`), without
    /// building the set. Any other node is evaluated and tested.
    fn member_lazy(&mut self, code: &Code, x: Handle, set: P) -> Result<bool, Error> {
        match code.nodes.get(set as usize) {
            // TLC's `SetPredValue.member`: the domain first, then the
            // predicate on the element.
            Some(Node::Filter { bind, pred }) => {
                if !self.member_lazy(code, x, bind.domain)? {
                    return Ok(false);
                }
                let mark = self.saved.len();
                for &s in &bind.slots {
                    let v = self.slot(s);
                    self.saved.push(v);
                }
                let r = self
                    .bind_one(bind, x)
                    .and_then(|()| self.eval_bool(code, *pred));
                for (k, &s) in bind.slots.iter().enumerate() {
                    let v = self.saved.get(mark + k).copied().unwrap_or(Handle::EMPTY);
                    self.set_slot(s, v);
                }
                self.saved.truncate(mark);
                r
            }
            // `SetCupValue`, `SetCapValue`, `SetDiffValue`: short-circuit, left
            // operand first.
            Some(Node::SetBin(op, a, b)) => {
                let (op, a, b) = (*op, *a, *b);
                let left = self.member_lazy(code, x, a)?;
                match op {
                    SetOp::Cup if left => Ok(true),
                    SetOp::Cap | SetOp::Minus if !left => Ok(false),
                    SetOp::Cup | SetOp::Cap => self.member_lazy(code, x, b),
                    SetOp::Minus => Ok(!self.member_lazy(code, x, b)?),
                }
            }
            // A thunk not yet forced is not forced: TLC's value behind it is
            // the lazy set.
            Some(Node::Lazy { slot, body }) => {
                let v = self.slot(*slot);
                if v.is_empty() {
                    self.member_lazy(code, x, *body)
                } else {
                    self.member(x, v)
                }
            }
            Some(Node::Scope { reset, body }) => {
                for &s in reset {
                    self.set_slot(s, Handle::EMPTY);
                }
                self.member_lazy(code, x, *body)
            }
            // The compiler emits `InLazy` only for sets TLC certainly keeps
            // lazy, so deciding membership structurally is exactly TLC's
            // evaluation: no speculation.
            Some(Node::Spec(b)) => self.member_lazy(code, x, *b),
            _ => {
                let s = self.eval(code, set)?;
                self.member(x, s)
            }
        }
    }

    /// Evaluates a memo site's body through its table.
    fn memo_eval(&mut self, code: &Code, site: u32, body: P) -> Result<Handle, Error> {
        if !self.opts.memo || !self.memo.active(site) {
            return self.eval(code, body);
        }
        let Some(s) = code.sites.get(site as usize) else {
            return self.eval(code, body);
        };
        let mut key = [Handle::EMPTY; MAX_KEY];
        let mut n = 0;
        for &v in &s.vars {
            let h = self
                .s0
                .get(usize::from(v))
                .copied()
                .unwrap_or(Handle::EMPTY);
            if h.is_transient() || h.is_empty() || n >= MAX_KEY {
                self.memo.bypass(site);
                return self.eval(code, body);
            }
            key[n] = h;
            n += 1;
        }
        for &sl in &s.slots {
            let h = self.slot(sl);
            if h.is_transient() || h.is_empty() || n >= MAX_KEY {
                self.memo.bypass(site);
                return self.eval(code, body);
            }
            key[n] = h;
            n += 1;
        }
        if let Some(r) = self.memo.get(site, &key[..n]) {
            return Ok(r);
        }
        let r = self.eval(code, body)?;
        if !r.is_transient() {
            self.memo.put(site, &key[..n], r);
        }
        Ok(r)
    }

    fn call(&mut self, code: &Code, fun: u32, args: &[P]) -> Result<Handle, Error> {
        let Some(f) = code.funs.get(fun as usize) else {
            return Err(Error::internal("call of an unknown function"));
        };
        if self.depth >= MAX_CALL_DEPTH {
            return Err(Error::unsupported(
                "recursion deeper than the M0 core's limit",
            ));
        }
        let mut vals = self.take_scratch();
        for &a in args {
            match self.eval_norm(code, a) {
                Ok(v) => vals.push(v),
                Err(e) => {
                    self.give_scratch(vals);
                    return Err(e);
                }
            }
        }
        let old = self.enter(f.frame);
        for (i, &v) in vals.iter().enumerate() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "parameter counts are small"
            )]
            let i = i as u32;
            let s = match i.checked_sub(f.params) {
                None => i,
                Some(c) => f.caps.get(c as usize).copied().unwrap_or(i),
            };
            self.set_slot(s, v);
        }
        self.give_scratch(vals);
        self.depth += 1;
        let r = self.eval(code, f.body);
        self.depth -= 1;
        self.leave(old);
        r
    }

    // ------------------------------------------------------------------
    // Sets
    // ------------------------------------------------------------------

    /// Refuses a just-built set whose elements TLC cannot all compare
    /// (`compare::check_set`).
    fn checked_set(&self, s: Handle) -> Result<Handle, Error> {
        if s.is_compound() {
            compare::check_set(&self.heap, self.heap.view(s).data)?;
        }
        Ok(s)
    }

    /// TLC's error for a membership test of a value that is not a function
    /// in a set of functions, records, tuples or sequences.
    fn non_fcn_member(&self, x: Handle, what: &str) -> Result<bool, Error> {
        if x.is_compound() && self.heap.view(x).kind.is_fcn() {
            return Ok(false);
        }
        Err(eval_err(&format!(
            "Attempted to check if a value that is not a function is in {what}"
        )))
    }

    /// Whether `x` (normalized) is in `set`.
    ///
    /// # Errors
    /// `set` is not a set, or membership is undefined.
    #[allow(clippy::too_many_lines, reason = "one arm per set kind")]
    pub fn member(&mut self, x: Handle, set: Handle) -> Result<bool, Error> {
        if !set.is_compound() {
            return Err(eval_err("membership in a value that is not a set"));
        }
        let kind = self.heap.view(set).kind;
        if kind != Kind::Set && !self.opts.symbolic_member && self.heap.is_finite(set) {
            let m = self.heap.materialize_cached(set)?;
            return self.member(x, m);
        }
        let v = self.heap.view(set);
        match v.kind {
            Kind::Set => {
                if v.data.binary_search_by(|&e| self.heap.cmp(e, x)).is_ok() {
                    return Ok(true);
                }
                compare::miss(&self.heap, x, v.data)?;
                Ok(false)
            }
            Kind::Interval => {
                let hi = v.data[0].as_int().unwrap_or(v.lo - 1);
                let lo = v.lo;
                match x.as_int() {
                    Some(i) => Ok(lo <= i && i <= hi),
                    // TLC answers FALSE for an empty interval.
                    None if lo > hi => Ok(false),
                    None => Err(eval_err(
                        "Attempted to check if a value that is not an integer is in an integer interval",
                    )),
                }
            }
            Kind::Nat | Kind::Int => match x.as_int() {
                Some(i) => Ok(v.kind == Kind::Int || i >= 0),
                None => Err(eval_err(
                    "Attempted to check if a value that is not an integer is an element of Nat or Int",
                )),
            },
            Kind::Subset => {
                let base = v.data[0];
                if !x.is_compound() || self.heap.view(x).kind != Kind::Set {
                    // A finite set is normalized before membership, so this is
                    // a non-set or a non-enumerable (infinite) set.
                    return Err(eval_err(
                        "Attempted to check if a non-enumerable value is an element of SUBSET",
                    ));
                }
                let elems = self.heap.view(x).data.to_vec();
                for e in elems {
                    if !self.member(e, base)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Kind::SeqSet => {
                let base = v.data[0];
                if !x.is_compound() || !self.heap.view(x).kind.is_fcn() {
                    return self.non_fcn_member(x, "a set of sequences");
                }
                let xv = self.heap.view(x);
                if xv.kind != Kind::IntFcn || xv.lo != 1 {
                    return Ok(false);
                }
                let vals = xv.data.to_vec();
                for e in vals {
                    if !self.member(e, base)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Kind::FcnSet => {
                let (dom, rng) = (v.data[0], v.data[1]);
                if !x.is_compound() || !self.heap.view(x).kind.is_fcn() {
                    return self.non_fcn_member(x, "a set of functions");
                }
                let d = self.domain(x)?;
                let d = self.heap.norm(d)?;
                let dom = self.heap.norm(dom)?;
                if !self.heap.eq(d, dom) {
                    return Ok(false);
                }
                let vals = self.heap.view(x).values().to_vec();
                for e in vals {
                    if !self.member(e, rng)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Kind::RecSet => {
                let n = v.data.len() / 2;
                let (names, sets) = (v.data[..n].to_vec(), v.data[n..].to_vec());
                if !x.is_compound() || !self.heap.view(x).kind.is_fcn() {
                    return self.non_fcn_member(x, "a set of records");
                }
                let xv = self.heap.view(x);
                if xv.kind != Kind::Fcn || xv.keys() != names.as_slice() {
                    return Ok(false);
                }
                let vals = xv.values().to_vec();
                for (e, s) in vals.into_iter().zip(sets) {
                    if !self.member(e, s)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Kind::Product => {
                let sets = v.data.to_vec();
                if !x.is_compound() || !self.heap.view(x).kind.is_fcn() {
                    return self.non_fcn_member(x, "a set of tuples");
                }
                let xv = self.heap.view(x);
                if xv.kind != Kind::IntFcn || xv.lo != 1 || xv.data.len() != sets.len() {
                    return Ok(false);
                }
                let vals = xv.data.to_vec();
                for (e, s) in vals.into_iter().zip(sets) {
                    if !self.member(e, s)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Kind::IntFcn | Kind::Fcn => Err(eval_err("membership in a function")),
        }
    }

    fn set_bin(&mut self, op: SetOp, x: Handle, y: Handle) -> Result<Handle, Error> {
        let (x, y) = (self.heap.norm(x)?, self.heap.norm(y)?);
        let fx = self.heap.is_finite(x);
        let fy = self.heap.is_finite(y);
        for (s, finite) in [(x, fx), (y, fy)] {
            if finite && s.is_compound() {
                let n = self.heap.view(s).len();
                self.charge(n)?;
            }
        }
        match op {
            SetOp::Cup => {
                if !fx || !fy {
                    return Err(Error::unsupported("union of an infinite set"));
                }
                let mut v = self.heap.elements(x)?;
                v.extend(self.heap.elements(y)?);
                let r = self.heap.set_from(&mut v);
                self.checked_set(r)
            }
            SetOp::Cap => {
                let (a, b) = if fx {
                    (x, y)
                } else if fy {
                    (y, x)
                } else {
                    return Err(Error::unsupported("intersection of infinite sets"));
                };
                let mut out = Vec::new();
                for e in self.heap.elements(a)? {
                    if self.member(e, b)? {
                        out.push(e);
                    }
                }
                Ok(self.heap.set_sorted(&out))
            }
            SetOp::Minus => {
                if !fx {
                    return Err(Error::unsupported("difference of an infinite set"));
                }
                let mut out = Vec::new();
                for e in self.heap.elements(x)? {
                    if !self.member(e, y)? {
                        out.push(e);
                    }
                }
                Ok(self.heap.set_sorted(&out))
            }
        }
    }

    fn filter(&mut self, code: &Code, bind: &Bind, pred: P) -> Result<Handle, Error> {
        // The cursor enumerates a symbolic domain in canonical order without
        // materializing it (R9, R10).
        let dom = self.eval(code, bind.domain)?;
        let mut cur = self.heap.cursor(dom)?;
        let mut out = Vec::new();
        loop {
            // Free a rejected candidate, and the predicate's transients, by
            // arena rollback (R10). Only when no thunk was forced to a
            // transient meanwhile: its slot may hold a value built after the
            // mark (`03-value-model.md` §11.5 rule 2).
            let before = self.heap.mark();
            let Some(e) = cur.next(&mut self.heap)? else {
                break;
            };
            self.charge(1)?;
            let after = self.heap.mark();
            let forced = self.stats.transient_thunks;
            self.bind_one(bind, e)?;
            let keep = self.eval_bool(code, pred)?;
            if self.stats.transient_thunks == forced {
                if keep {
                    self.heap.rollback(after);
                } else {
                    self.heap.rollback(before);
                }
            }
            if keep {
                out.push(e);
            }
        }
        Ok(self.heap.set_sorted(&out))
    }

    /// Binds one domain element to a bound group of one variable or a tuple
    /// pattern.
    pub(crate) fn bind_one(&mut self, bind: &Bind, e: Handle) -> Result<(), Error> {
        if bind.tuple {
            let v = self.heap.view(e);
            if v.kind != Kind::IntFcn || v.lo != 1 || v.data.len() != bind.slots.len() {
                return Err(eval_err("a tuple pattern does not match the element"));
            }
            let parts = v.data.to_vec();
            for (&s, p) in bind.slots.iter().zip(parts) {
                self.set_slot(s, p);
            }
        } else if let Some(&s) = bind.slots.first() {
            self.set_slot(s, e);
        }
        Ok(())
    }

    /// The binding units of bound groups: one per variable of a plain group,
    /// one per tuple-pattern group; each with its domain's elements.
    pub(crate) fn units(
        &mut self,
        code: &Code,
        binds: &[Bind],
    ) -> Result<Vec<(Unit, Vec<Handle>)>, Error> {
        let mut out = Vec::new();
        for (bi, b) in binds.iter().enumerate() {
            let d = self.eval(code, b.domain)?;
            let d = self.heap.norm(d)?;
            let elems = self.heap.elements(d)?;
            if b.tuple {
                out.push((
                    Unit {
                        bind: bi,
                        var: None,
                    },
                    elems,
                ));
            } else {
                for vi in 0..b.slots.len() {
                    out.push((
                        Unit {
                            bind: bi,
                            var: Some(vi),
                        },
                        elems.clone(),
                    ));
                }
            }
        }
        Ok(out)
    }

    pub(crate) fn bind_unit(&mut self, binds: &[Bind], u: Unit, e: Handle) -> Result<(), Error> {
        let b = &binds[u.bind];
        match u.var {
            Some(vi) => {
                self.set_slot(b.slots[vi], e);
                Ok(())
            }
            None => self.bind_one(b, e),
        }
    }

    /// Visits every binding of the bound groups, the first variable fastest
    /// (TLC's `ContextEnumerator`), until `f` returns `false`.
    pub(crate) fn for_each_binding(
        &mut self,
        code: &Code,
        binds: &[Bind],
        f: &mut dyn FnMut(&mut Self) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
        let units = self.units(code, binds)?;
        if units.iter().any(|(_, e)| e.is_empty()) {
            return Ok(true);
        }
        let mut idx = vec![0usize; units.len()];
        loop {
            self.charge(1)?;
            for (k, (u, elems)) in units.iter().enumerate() {
                self.bind_unit(binds, *u, elems[idx[k]])?;
            }
            if !f(self)? {
                return Ok(false);
            }
            let mut k = 0;
            loop {
                if k == units.len() {
                    return Ok(true);
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

    fn quant(
        &mut self,
        code: &Code,
        kind: Quant,
        binds: &[Bind],
        body: P,
    ) -> Result<Handle, Error> {
        match kind {
            Quant::Exists => {
                let mut found = false;
                self.for_each_binding(code, binds, &mut |m: &mut Self| {
                    if m.eval_bool(code, body)? {
                        found = true;
                        return Ok(false);
                    }
                    Ok(true)
                })?;
                Ok(Handle::bool(found))
            }
            Quant::Forall => {
                let all = self
                    .for_each_binding(code, binds, &mut |m: &mut Self| m.eval_bool(code, body))?;
                Ok(Handle::bool(all))
            }
            Quant::Choose => {
                if binds.len() != 1 || (!binds[0].tuple && binds[0].slots.len() != 1) {
                    return Err(Error::unsupported("CHOOSE with several bound variables"));
                }
                let d = self.eval(code, binds[0].domain)?;
                let mut cur = self.heap.cursor(d)?;
                while let Some(e) = cur.next(&mut self.heap)? {
                    self.charge(1)?;
                    self.bind_one(&binds[0], e)?;
                    if self.eval_bool(code, body)? {
                        return Ok(e);
                    }
                }
                Err(eval_err(
                    "Attempted to compute the value of an expression of form CHOOSE x \\in S: P, but no element of S satisfied P",
                ))
            }
        }
    }

    fn fcn_cons(&mut self, code: &Code, binds: &[Bind], body: P) -> Result<Handle, Error> {
        let units = self.units(code, binds)?;
        if units.len() == 1 {
            let (u, keys) = units
                .into_iter()
                .next()
                .unwrap_or((Unit { bind: 0, var: None }, Vec::new()));
            self.charge(keys.len())?;
            let mut vals = Vec::with_capacity(keys.len());
            for &k in &keys {
                self.bind_unit(binds, u, k)?;
                vals.push(self.eval_norm(code, body)?);
            }
            return Ok(self.heap.fcn_sorted(&keys, &vals));
        }
        if units.iter().any(|(u, _)| u.var.is_none()) {
            return Err(Error::unsupported(
                "a function constructor mixing tuple patterns and several bounds",
            ));
        }
        // Keys are tuples of the units' values, in canonical order (last
        // component fastest).
        let mut keys = Vec::new();
        let mut vals = Vec::new();
        if units.iter().any(|(_, e)| e.is_empty()) {
            return Ok(self.heap.fcn_sorted(&[], &[]));
        }
        let mut idx = vec![0usize; units.len()];
        'outer: loop {
            self.charge(1)?;
            let comps: Vec<Handle> = idx.iter().zip(&units).map(|(&i, (_, e))| e[i]).collect();
            for (k, (u, _)) in units.iter().enumerate() {
                self.bind_unit(binds, *u, comps[k])?;
            }
            keys.push(self.heap.tuple(&comps));
            vals.push(self.eval_norm(code, body)?);
            let mut k = units.len();
            loop {
                if k == 0 {
                    break 'outer;
                }
                k -= 1;
                idx[k] += 1;
                if idx[k] < units[k].1.len() {
                    break;
                }
                idx[k] = 0;
            }
        }
        Ok(self.heap.fcn_sorted(&keys, &vals))
    }

    // ------------------------------------------------------------------
    // Functions and records
    // ------------------------------------------------------------------

    /// The index of `key` in function `f`, if present.
    pub(crate) fn fcn_index(&self, f: Handle, key: Handle) -> Result<Option<usize>, Error> {
        if !f.is_compound() {
            return Err(eval_err("application of a value that is not a function"));
        }
        let v = self.heap.view(f);
        match v.kind {
            Kind::IntFcn => Ok(key.as_int().and_then(|k| {
                let i = k.checked_sub(v.lo)?;
                usize::try_from(i).ok().filter(|&i| i < v.data.len())
            })),
            Kind::Fcn => {
                let keys = v.keys();
                if key.as_str_id().is_some() || keys.len() <= 8 {
                    Ok(keys.iter().position(|&k| self.heap.eq(k, key)))
                } else {
                    Ok(keys.binary_search_by(|&k| self.heap.cmp(k, key)).ok())
                }
            }
            _ => Err(eval_err("application of a set as a function")),
        }
    }

    /// `f[a]`.
    ///
    /// # Errors
    /// `a` is not in the domain of `f`.
    pub fn apply(&mut self, f: Handle, a: Handle) -> Result<Handle, Error> {
        match self.fcn_index(f, a)? {
            Some(i) => Ok(self.heap.view(f).values()[i]),
            None => Err(eval_err(
                "Attempted to apply function to an argument not in its domain",
            )),
        }
    }

    fn select(&mut self, r: Handle, field: Handle) -> Result<Handle, Error> {
        if !r.is_compound() {
            return Err(eval_err("field selection on a value that is not a record"));
        }
        let v = self.heap.view(r);
        if v.kind == Kind::Fcn
            && let Some(i) = v.keys().iter().position(|&k| k == field)
        {
            return Ok(v.values()[i]);
        }
        Err(eval_err(
            "Attempted to access a record field that does not exist",
        ))
    }

    fn domain(&mut self, f: Handle) -> Result<Handle, Error> {
        if !f.is_compound() {
            return Err(eval_err("DOMAIN of a value that is not a function"));
        }
        let v = self.heap.view(f);
        match v.kind {
            #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
            Kind::IntFcn => {
                let (lo, n) = (v.lo, v.data.len() as i64);
                if n == 0 {
                    return Ok(self.heap.empty_set());
                }
                let mut out = Vec::with_capacity(v.data.len());
                for k in lo..lo + n {
                    out.push(mk_int(k)?);
                }
                Ok(self.heap.set_sorted(&out))
            }
            Kind::Fcn => {
                let keys = v.keys().to_vec();
                Ok(self.heap.set_sorted(&keys))
            }
            _ => Err(eval_err("DOMAIN of a set")),
        }
    }

    fn except(
        &mut self,
        code: &Code,
        f: Handle,
        u: &Update,
        depth: usize,
    ) -> Result<Handle, Error> {
        let Some(&step) = u.path.get(depth) else {
            return Err(Error::internal("EXCEPT path out of range"));
        };
        let key = self.eval_norm(code, step)?;
        if !f.is_compound() || !self.heap.view(f).kind.is_fcn() {
            return Err(eval_err("EXCEPT on a value that is not a function"));
        }
        let Some(i) = self.fcn_index(f, key)? else {
            // TLC leaves the function unchanged when the key is outside its
            // domain, but fails when it had to compare the key with a domain
            // value of another class on the way.
            let v = self.heap.view(f);
            if v.kind == Kind::IntFcn {
                if key.as_int().is_none() && !v.data.is_empty() {
                    return Err(eval_err(
                        "Attempted to apply EXCEPT to a tuple at a non-integral index",
                    ));
                }
            } else {
                compare::miss(&self.heap, key, v.keys())?;
            }
            return Ok(f);
        };
        let old = self.heap.view(f).values()[i];
        let new = if depth + 1 == u.path.len() {
            self.set_slot(u.at, old);
            self.eval_norm(code, u.value)?
        } else {
            self.except(code, old, u, depth + 1)?
        };
        if new == old {
            return Ok(f);
        }
        Ok(self.heap.fcn_with(f, i, new))
    }

    // ------------------------------------------------------------------
    // Standard operators
    // ------------------------------------------------------------------

    fn seq_vals(&self, s: Handle) -> Result<Vec<Handle>, Error> {
        if !s.is_compound() {
            return Err(eval_err("expected a sequence"));
        }
        let v = self.heap.view(s);
        if v.kind != Kind::IntFcn || v.lo != 1 {
            return Err(eval_err("expected a sequence"));
        }
        Ok(v.data.to_vec())
    }

    fn std(&mut self, code: &Code, f: StdFn, args: &[P]) -> Result<Handle, Error> {
        let mut a = Vec::with_capacity(args.len());
        for &x in args {
            a.push(self.eval_norm(code, x)?);
        }
        let arg = |i: usize| {
            a.get(i)
                .copied()
                .ok_or_else(|| Error::internal("missing argument"))
        };
        match f {
            StdFn::Len => {
                let s = arg(0)?;
                let v = self.heap.view(s);
                if !s.is_compound() || v.kind != Kind::IntFcn || v.lo != 1 {
                    return Err(eval_err("Len of a value that is not a sequence"));
                }
                mk_int(i64::try_from(v.data.len()).unwrap_or(i64::MAX))
            }
            StdFn::Head => {
                let vals = self.seq_vals(arg(0)?)?;
                vals.first()
                    .copied()
                    .ok_or_else(|| eval_err("Attempted to apply Head to the empty sequence"))
            }
            StdFn::Tail => {
                let vals = self.seq_vals(arg(0)?)?;
                if vals.is_empty() {
                    return Err(eval_err("Attempted to apply Tail to the empty sequence"));
                }
                Ok(self.heap.tuple(&vals[1..]))
            }
            StdFn::Append => {
                let mut vals = self.seq_vals(arg(0)?)?;
                vals.push(arg(1)?);
                Ok(self.heap.tuple(&vals))
            }
            StdFn::Concat => {
                let mut vals = self.seq_vals(arg(0)?)?;
                vals.extend(self.seq_vals(arg(1)?)?);
                Ok(self.heap.tuple(&vals))
            }
            StdFn::SubSeq => {
                let vals = self.seq_vals(arg(0)?)?;
                let m = int_of(arg(1)?)?;
                let n = int_of(arg(2)?)?;
                if m > n {
                    return Ok(self.heap.tuple(&[]));
                }
                let len = i64::try_from(vals.len()).unwrap_or(i64::MAX);
                if m < 1 || n > len {
                    return Err(eval_err("SubSeq indices out of range"));
                }
                #[allow(
                    clippy::cast_sign_loss,
                    clippy::cast_possible_truncation,
                    reason = "checked range"
                )]
                Ok(self.heap.tuple(&vals[(m - 1) as usize..n as usize]))
            }
            StdFn::Cardinality => {
                let n = self.heap.cardinality(arg(0)?)?;
                mk_int(i64::try_from(n).unwrap_or(i64::MAX))
            }
            StdFn::Seq => {
                let s = arg(0)?;
                Ok(self.heap.symbolic(Kind::SeqSet, 0, &[s]))
            }
            StdFn::IsFiniteSet => {
                let s = arg(0)?;
                Ok(Handle::bool(self.heap.is_finite(s)))
            }
        }
    }
}

/// A binding unit (see [`Machine::units`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Unit {
    pub(crate) bind: usize,
    pub(crate) var: Option<usize>,
}

fn arith(op: Arith, x: i64, y: i64) -> Result<Handle, Error> {
    let overflow = || Error::unsupported("integer outside the M0 core's 61-bit range");
    let r = match op {
        Arith::Add => x.checked_add(y).ok_or_else(overflow)?,
        Arith::Sub => x.checked_sub(y).ok_or_else(overflow)?,
        Arith::Mul => x.checked_mul(y).ok_or_else(overflow)?,
        Arith::Div => {
            if y == 0 {
                return Err(eval_err("The second argument of \\div is 0."));
            }
            // Floor division, as TLC. Inline integers are 61-bit, so the
            // quotient cannot overflow.
            let q = x / y;
            if (x % y != 0) && ((x < 0) != (y < 0)) {
                q - 1
            } else {
                q
            }
        }
        Arith::Mod => {
            if y <= 0 {
                return Err(eval_err(
                    "The second argument of % should be a positive number",
                ));
            }
            x.rem_euclid(y)
        }
        Arith::Pow => {
            if y < 0 {
                return Err(eval_err("negative exponent"));
            }
            let e = u32::try_from(y).map_err(|_| overflow())?;
            x.checked_pow(e).ok_or_else(overflow)?
        }
    };
    mk_int(r)
}
