//! The interpreter for value nodes (`04-plan-compiler.md` §6): a recursive
//! `match` over the plan's node array, over one flat frame per activation.

use std::sync::Arc;

use crate::Error;
use crate::compare;
use crate::handle::Handle;
use crate::heap::{Cursor, Heap};
use crate::int::{IntMode, IntOp};
use crate::memo::{MAX_KEY, Memo};
use crate::node::Kind;
use crate::opts::Opts;
use crate::plan::{Arith, Bind, Cmp, Code, LibArg, Node, P, Quant, SetOp, StdFn, Update};
use crate::store::Store;

#[path = "eval_product.rs"]
mod product;
#[path = "eval_work.rs"]
mod work;

/// Conservative call nesting bound, including argument evaluation. Refuse
/// before unbounded model recursion consumes the host interpreter's stack.
const MAX_CALL_DEPTH: usize = 64;

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

/// An optional cooperative work checkpoint. A false result only interrupts
/// the machine; the caller must classify its own actual run authority.
pub trait WorkControl: std::fmt::Debug + Send + Sync {
    /// Whether evaluation may continue.
    fn proceed(&self) -> bool;
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
    /// What `Print` and `PrintT` printed, one line each, in evaluation
    /// order.
    pub printed: Vec<String>,
    output_limits: Option<(usize, usize)>,
    output_used: (usize, usize),
    output_failure: Option<&'static str>,
    pub(crate) checker_access: bool,
    /// The random stream draws come from: the startup stream or the
    /// expanded state's (`random.rs`); `None` outside a search.
    pub draw: Option<Box<dyn crate::random::DrawStream>>,
    /// How many draws were made, which the engine reads to refuse a draw
    /// in a check.
    pub draws: u64,
    /// `TLCGet("level")`: 0 in `Init`, the expanded state's level in the
    /// next-state relation and action checks, the checked state's level in
    /// state checks (initial states are level 1). The engine sets it.
    pub level: i64,
    pub(crate) conts: Vec<crate::generate::ContNode>,
    pub(crate) cvals: Vec<Handle>,
    pub(crate) saved: Vec<Handle>,
    scratch: Vec<Vec<Handle>>,
    depth: usize,
    /// Loop iterations left before evaluation gives up. Unbounded at run
    /// time; the compiler bounds it while folding constants (R9), and an
    /// isolated optional analysis machine bounds its own speculative work.
    pub(crate) fuel: u64,
    /// Whether a site where the reference evaluator prints a warning that
    /// Core does not record in [`Machine::warnings`] refuses instead
    /// ([`Machine::set_refuse_unrecorded_warnings`]).
    refuse_unrecorded_warnings: bool,
    work_control: Option<Arc<dyn WorkControl>>,
    pub(crate) debugger: Option<Arc<dyn crate::debug::Observer>>,
}

fn eval_err(what: &str) -> Error {
    Error::eval(what)
}

/// An `Assert.fail(EC.GENERAL, ...)` error the core raises exactly where
/// TLC does. Messages that begin like TLC's keep TLC's beginning, which the
/// engine's replay rule reads (`dive-check` `class.rs`).
fn general(what: &str) -> Error {
    Error::eval_class(crate::TlcClass::GENERAL, what)
}

/// An `EvalException` of TLC class `code` (`04-plan-compiler.md` §5).
fn eval_exc(code: u32, what: &str) -> Error {
    Error::eval_class(crate::TlcClass::eval_exception(code), what)
}

/// `EC.TLC_MODULE_COMPUTING_CARDINALITY`: `Cardinality` of a value that is
/// not an enumerable set (`FiniteSets.Cardinality`; a function, record or
/// tuple is not `Enumerable` there either).
pub(crate) fn cardinality_error() -> Error {
    eval_exc(2181, "Attempted to compute cardinality of the value")
}

/// `EC.TLC_MODULE_ONE_ARGUMENT_ERROR`, as `SequencesHelper` raises it.
fn one_arg(op: &str, what: &str) -> Error {
    eval_exc(
        2283,
        &format!("The argument of {op} should be a {what}, but instead it is not"),
    )
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
    pub fn new(store: Arc<Store>, nvars: usize, opts: Opts, mut memo: Memo) -> Self {
        memo.set_cap(opts.memo_cap);
        memo.reserve_members(opts.memo && store.semantics().tlc_comparisons);
        memo.reserve_member_index(opts.member_index && !store.semantics().tlc_comparisons);
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
            printed: Vec::new(),
            output_limits: None,
            output_used: (0, 0),
            output_failure: None,
            checker_access: true,
            draw: None,
            draws: 0,
            level: 0,
            conts: Vec::new(),
            cvals: Vec::new(),
            saved: Vec::new(),
            scratch: Vec::new(),
            depth: 0,
            fuel: u64::MAX,
            refuse_unrecorded_warnings: false,
            work_control: None,
            debugger: None,
        }
    }

    /// Bounds optional speculative work. Exhaustion yields unsupported and
    /// cannot be interpreted as a model verdict or a run-budget stop.
    ///
    /// ```
    /// # fn configure(machine: &mut dive_core::Machine) {
    /// machine.set_fuel(4096);
    /// # }
    /// ```
    pub fn set_fuel(&mut self, units: u64) {
        self.fuel = units;
        self.memo.reserve_member_index(
            units == u64::MAX
                && self.opts.member_index
                && !self.heap.store.semantics().tlc_comparisons,
        );
    }

    /// Makes every site where the reference evaluator prints a warning that
    /// this machine does not record in [`Machine::warnings`] fail as
    /// unsupported instead: an `EXCEPT` key outside the function's domain
    /// (TLC warning 2144). An isolated optional machine sets it so that its
    /// caller can fall back to the reference evaluator, which prints the
    /// warning. Ordinary machines keep TLC's unchanged-function result.
    ///
    /// ```
    /// # fn configure(machine: &mut dive_core::Machine) {
    /// machine.set_refuse_unrecorded_warnings(true);
    /// # }
    /// ```
    pub fn set_refuse_unrecorded_warnings(&mut self, enabled: bool) {
        self.refuse_unrecorded_warnings = enabled;
    }

    /// Installs the optional caller-owned cooperative work checkpoint.
    /// Ordinary machines have no checkpoint and unbounded fuel.
    ///
    /// ```
    /// # fn example(machine: &mut dive_core::Machine) {
    /// machine.set_work_control(None);
    /// # }
    /// ```
    pub fn set_work_control(&mut self, control: Option<Arc<dyn WorkControl>>) {
        self.heap.set_work_control(control.clone());
        self.work_control = control;
    }

    /// Installs explicit producer observation. Fresh replay/analysis machines
    /// remain unobserved unless a caller separately installs this capability.
    pub fn set_debugger(&mut self, observer: Option<Arc<dyn crate::debug::Observer>>) {
        self.debugger = observer;
    }

    /// Keeps generator continuation transients alive during one eager batch.
    ///
    /// Internal rollback calls are deferred only inside `work`; normal rollback
    /// behavior resumes before this method returns. The caller must retain any
    /// required values after their semantic demands, clear transient receivers,
    /// then roll back its enclosing batch mark. This does not persist or
    /// normalize values and must not wrap a whole search or repeated iterations.
    ///
    /// ```
    /// # fn example(machine: &mut dive_core::Machine) {
    /// let mark = machine.heap.mark();
    /// machine.with_deferred_reclamation(|_machine| ());
    /// machine.heap.rollback(mark);
    /// # }
    /// ```
    pub fn with_deferred_reclamation<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        let previous = std::mem::replace(&mut self.heap.defer_reclamation, true);
        let result = work(self);
        self.heap.defer_reclamation = previous;
        result
    }

    /// Disables TLCGet/TLCSet in finite modes without a live checker context.
    /// Access refuses when reached; an untaken branch remains unevaluated.
    pub fn deny_checker_access(&mut self) {
        self.checker_access = false;
    }

    fn check_finite_access(&self, op: dive_ir::StdOp) -> Result<(), Error> {
        if !self.checker_access
            && op.impure()
            && !matches!(op, dive_ir::StdOp::TlcPrint | dive_ir::StdOp::TlcPrintT)
        {
            return Err(Error::unsupported(format!(
                "{}: native finite mode runtime effect is unsupported",
                op.qualified_name()
            )));
        }
        Ok(())
    }

    /// Bounds retained Print events and UTF-8 bytes for a finite mode.
    /// The ordinary checker leaves this unset. Rendering occurs before admission.
    pub fn set_output_limits(&mut self, events: usize, bytes: usize) {
        self.output_limits = Some((events, bytes));
    }

    /// The first output-admission limit reached by this machine.
    #[must_use]
    pub const fn output_failure(&self) -> Option<&'static str> {
        self.output_failure
    }

    pub(crate) fn print(&mut self, text: String) -> Result<(), Error> {
        if let Some((events, bytes)) = self.output_limits {
            let total = self.output_used.1.checked_add(text.len());
            let failure = if self.output_used.0 >= events {
                Some("simulation output event bound")
            } else if total.is_none_or(|total| total > bytes) {
                Some("simulation output byte bound")
            } else {
                None
            };
            if let Some(failure) = failure {
                self.output_failure.get_or_insert(failure);
                return Err(Error::unsupported(failure));
            }
            self.output_used = (self.output_used.0 + 1, total.unwrap_or(bytes));
        }
        self.printed.push(text);
        Ok(())
    }

    /// Spends `n` units of fuel: one per element a loop visits.
    #[inline]
    pub(crate) fn charge(&mut self, n: usize) -> Result<(), Error> {
        if self
            .work_control
            .as_ref()
            .is_some_and(|control| !control.proceed())
        {
            self.heap.mark_work_interrupted();
            return Err(Error::stop());
        }
        if self.fuel == u64::MAX {
            return Ok(());
        }
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
        self.work_norm(v)
    }

    pub(crate) fn demand_equals(&mut self, a: Handle, b: Handle) -> Result<bool, Error> {
        let a = self.work_norm(a)?;
        let b = self.work_norm(b)?;
        compare::equals(&self.heap, a, b)
    }

    pub(crate) fn eval_component(&mut self, code: &Code, p: P) -> Result<Handle, Error> {
        let value = self.eval(code, p)?;
        if self.heap.kind(value) == Some(Kind::Product) {
            // Keep symbolic products lazy until an operator demands their elements.
            Ok(value)
        } else {
            self.work_norm(value)
        }
    }

    /// Evaluates a node.
    ///
    /// # Errors
    /// Evaluation errors, refusals and resource limits.
    pub fn eval(&mut self, code: &Code, p: P) -> Result<Handle, Error> {
        let Some(observer) = &self.debugger else {
            return self.eval_inner(code, p);
        };
        let instruction = crate::debug::Instruction::Value(p);
        let guard = crate::debug::Guard::enter(observer, instruction, code, self)?;
        let result = self.eval_inner(code, p);
        let returned = match &result {
            Ok(value) => crate::debug::Returned::Value(*value),
            Err(error) => crate::debug::Returned::Failed(error),
        };
        guard.returned(instruction, code, self, returned)?;
        result
    }

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per node kind, with operand names as in the semantics"
    )]
    fn eval_inner(&mut self, code: &Code, p: P) -> Result<Handle, Error> {
        if self.fuel != u64::MAX {
            self.charge(1)?;
        }
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
                let x = self.work_norm(x)?;
                let y = self.eval(code, *b)?;
                let elems = self.work_elements(x)?;
                self.charge(elems.len())?;
                for e in elems {
                    if !self.member(e, y)? {
                        return Ok(Handle::FALSE);
                    }
                }
                Ok(Handle::TRUE)
            }
            Node::Arith(op, a, b) => {
                let x = self.eval(code, *a)?;
                let y = self.eval(code, *b)?;
                // Inline operands whose result is inline (and, in TLC mode,
                // a Java `int`) take the fast path; anything else is the
                // heap's (big integers, TLC overflow, domain errors).
                if let (Some(i), Some(j)) = (x.as_int(), y.as_int())
                    && let Some(h) = fast_arith(*op, i, j, self.heap.int_mode())
                {
                    return Ok(h);
                }
                self.heap.int_arith(int_op(*op), x, y)
            }
            Node::Cmp(op, a, b) => {
                let x = self.eval(code, *a)?;
                let y = self.eval(code, *b)?;
                let o = match (x.as_int(), y.as_int()) {
                    (Some(i), Some(j)) => i.cmp(&j),
                    _ => self
                        .heap
                        .int_cmp(x, y)
                        .ok_or_else(|| eval_err("expected an integer"))?,
                };
                Ok(Handle::bool(match op {
                    Cmp::Lt => o.is_lt(),
                    Cmp::Le => o.is_le(),
                    Cmp::Gt => o.is_gt(),
                    Cmp::Ge => o.is_ge(),
                }))
            }
            Node::Neg(a) => {
                let x = self.eval(code, *a)?;
                if self.heap.int_mode() == IntMode::Big
                    && let Some(h) = x.as_int().and_then(i64::checked_neg).and_then(Handle::int)
                {
                    return Ok(h);
                }
                self.heap.int_neg(x)
            }
            Node::Range(a, b) => {
                let x = int_of(self.eval(code, *a)?)?;
                let y = int_of(self.eval(code, *b)?)?;
                Ok(self.heap.interval(x, y))
            }
            Node::SetEnum(items) => {
                let mut v = self.take_scratch();
                for &i in items {
                    match self.eval_component(code, i) {
                        Ok(e) => v.push(e),
                        Err(e) => {
                            self.give_scratch(v);
                            return Err(e);
                        }
                    }
                }
                for value in &mut v {
                    *value = self.work_norm(*value)?;
                }
                if let Err(error) = compare::require_materialized(&self.heap, &v) {
                    self.give_scratch(v);
                    return Err(error);
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
                let members = self.work_elements(x)?;
                // TLC's `UnionValue` stays lazy: membership tests the
                // members in turn. The core keeps it lazy only when some
                // member cannot be enumerated.
                let mut normed = Vec::with_capacity(members.len());
                for m in members {
                    normed.push(self.work_norm(m)?);
                }
                if normed.iter().any(|&m| !self.heap.is_finite(m)) {
                    let mut it = normed.into_iter();
                    let Some(first) = it.next() else {
                        return Err(Error::internal("an empty UNION with an infinite member"));
                    };
                    return Ok(it.fold(first, |acc, m| self.heap.union(acc, m)));
                }
                let mut out = Vec::new();
                for m in normed {
                    let m = self.work_norm(m)?;
                    let es = self.work_elements(m)?;
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
                let x = self.eval_component(code, *d)?;
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
            Node::Filter { bind, pred, subset } => self.filter(code, bind, *pred, *subset),
            Node::Map { binds, body } => {
                let mut out = Vec::new();
                self.for_each_binding(code, binds, &mut |m: &mut Self| {
                    let v = m.eval_component(code, *body)?;
                    let v = m.work_norm(v)?;
                    m.charge(1)?;
                    out.try_reserve(1)
                        .map_err(|_| Error::unsupported("MAP construction allocation"))?;
                    out.push(v);
                    Ok(true)
                })?;
                compare::require_materialized(&self.heap, &out)?;
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
                    match self.eval_component(code, i) {
                        Ok(e) => v.push(e),
                        Err(e) => {
                            self.give_scratch(v);
                            return Err(e);
                        }
                    }
                }
                let r = self.heap.tuple_literal(&v);
                self.give_scratch(v);
                Ok(r)
            }
            Node::Record(fields) => {
                let mut keys = self.take_scratch();
                let mut vals = self.take_scratch();
                for &(k, e) in fields {
                    keys.push(k);
                    match self.eval_component(code, e) {
                        Ok(v) => vals.push(v),
                        Err(e) => {
                            self.give_scratch(keys);
                            self.give_scratch(vals);
                            return Err(e);
                        }
                    }
                }
                let r = self.heap.record(&keys, &vals);
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
            Node::Lib {
                op,
                args,
                repeatable_args,
            } => {
                self.check_finite_access(*op)?;
                // A value printed keeps its form: TLC prints an interval as
                // `1..3` (`crate::print`), and draws from a lazy `[S -> T]`
                // one value per component (`crate::random`).
                let prints = matches!(
                    op,
                    dive_ir::StdOp::TlcPrint
                        | dive_ir::StdOp::TlcPrintT
                        | dive_ir::StdOp::TlcToString
                        | dive_ir::StdOp::TlcRandomElement
                );
                let mut a = Vec::with_capacity(args.len());
                for &x in args {
                    a.push(if prints {
                        self.eval(code, x)?
                    } else if *op == dive_ir::StdOp::TlcMakeFcn {
                        self.eval_component(code, x)?
                    } else {
                        let value = self.eval_component(code, x)?;
                        self.work_norm(value)?
                    });
                }
                // The evaluating override returns null for nonstrings and
                // TLC then evaluates the TLA+ definition. Its argument
                // demands must not be replaced by a single value snapshot.
                if !repeatable_args
                    && matches!(
                        op,
                        dive_ir::StdOp::SequencesExtReplaceFirstSubSeq
                            | dive_ir::StdOp::SequencesExtReplaceAllSubSeqs
                    )
                    && a.iter().any(|h| h.as_str_id().is_none())
                {
                    return Err(Error::unsupported(format!(
                        "{} with sequence arguments whose repeated fallback demand is not proven effect-free",
                        op.qualified_name()
                    )));
                }
                crate::stdlib::call(self, *op, &a)
            }
            Node::LibHo(op, args) => {
                self.check_finite_access(*op)?;
                let mut a = Vec::with_capacity(args.len());
                for x in args {
                    a.push(match x {
                        LibArg::Value(p) => {
                            let value = self.eval_component(code, *p)?;
                            crate::stdlib::Ho::Value(self.work_norm(value)?)
                        }
                        LibArg::Op { params, body } => {
                            crate::stdlib::Ho::Op(crate::stdlib::OpFn::new(code, params, *body))
                        }
                    });
                }
                crate::stdlib::call_ho(self, *op, &a)
            }
            Node::Enabled { g, .. } => self.enabled(code, *g),
            Node::Composition { first, then, .. } => self.composition_holds(code, *first, *then),
            Node::Memo { site, body } => self.memo_eval(code, *site, *body),
            Node::Spec(b) => self.eval(code, *b).map_err(speculative),
            Node::Beta { chain, fc, arg } => self.beta(code, *chain, *fc, *arg),
            Node::DomCheck { slot, domain, body } => {
                let d = self.eval(code, *domain).map_err(speculative)?;
                let x = self.slot(*slot);
                if !self.member(x, d)? {
                    return Err(general(
                        "In applying the function\na recursive function, the argument is not in its domain",
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
                    return Err(general(
                        "In applying the function\na function, the argument is not in its domain",
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
            Some(Node::Filter { bind, pred, .. }) => {
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
        // Step sites intern primed keys even when opts.memo is false. Candidate
        // spill owns transient values instead, so bypass before any promotion.
        if self.heap.speculative {
            return self.eval(code, body);
        }
        let Some(s) = code.sites.get(site as usize) else {
            return self.eval(code, body);
        };
        if !(self.opts.memo || s.step) || !self.memo.active(site) {
            return self.eval(code, body);
        }
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
        for &v in &s.primed {
            let h = self
                .s1
                .get(usize::from(v))
                .copied()
                .unwrap_or(Handle::EMPTY);
            // The successor's value is keyed by an exact persistent copy
            // (`Heap::intern_exact`): equal keys are equal representations.
            // The successor itself is left as it is.
            let h = match self.heap.intern_exact(h) {
                Ok(k) if !h.is_empty() && n < MAX_KEY => k,
                _ => {
                    self.memo.bypass(site);
                    return self.eval(code, body);
                }
            };
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
        // A step site keeps an exact persistent copy of its result; one
        // that cannot be kept (lazy) is returned uncached.
        let kept = if s.step && r.is_transient() {
            self.heap.intern_exact(r).unwrap_or(r)
        } else {
            r
        };
        if !kept.is_transient() {
            self.memo.put(site, &key[..n], kept);
        }
        Ok(kept)
    }

    fn call(&mut self, code: &Code, fun: u32, args: &[P]) -> Result<Handle, Error> {
        self.charge(1)?;
        if self.depth >= MAX_CALL_DEPTH {
            return Err(Error::unsupported(
                "Core recursive-call depth limit (64 activations)",
            ));
        }
        // Arguments can recursively call the same function too. Count their
        // activations and restore the counter on every success/error path.
        self.depth += 1;
        let result = self.call_inner(code, fun, args);
        self.depth -= 1;
        result
    }

    fn call_inner(&mut self, code: &Code, fun: u32, args: &[P]) -> Result<Handle, Error> {
        let Some(f) = code.funs.get(fun as usize) else {
            return Err(Error::internal("call of an unknown function"));
        };
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
        let r = self.eval(code, f.body);
        self.leave(old);
        r
    }

    // ------------------------------------------------------------------
    // Sets
    // ------------------------------------------------------------------

    /// Refuses a just-built set whose elements TLC cannot all compare
    /// (`compare::check_set`).
    fn checked_set(&self, s: Handle) -> Result<Handle, Error> {
        compare::require_materialized(&self.heap, &[s])?;
        if self.heap.store().semantics().tlc_comparisons && s.is_compound() {
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
        if x.as_model_id().is_some() {
            return self.mv_member(x);
        }
        Err(eval_err(&format!(
            "Attempted to check if a value that is not a function is in {what}"
        )))
    }

    /// `ModelValue.modelValueMember`: an untyped model value is in no set of
    /// another kind; a typed one is TLC's error.
    fn mv_member(&self, x: Handle) -> Result<bool, Error> {
        if compare::mv_type(&self.heap, x).is_none() {
            return Ok(false);
        }
        Err(general(
            "Attempted to check if the typed model value is an element of a set of another kind",
        ))
    }

    /// Whether `x` (normalized) is in `set`.
    ///
    /// # Errors
    /// `set` is not a set, or membership is undefined.
    #[allow(clippy::too_many_lines, reason = "one arm per set kind")]
    pub fn member(&mut self, x: Handle, set: Handle) -> Result<bool, Error> {
        if self.fuel != u64::MAX && set.is_compound() && self.heap.view(set).kind == Kind::SetEnum {
            self.domain_work(set, false, 0)?;
        }
        if !set.is_compound() {
            return Err(eval_err("membership in a value that is not a set"));
        }
        let kind = self.heap.view(set).kind;
        if kind != Kind::Set
            && kind != Kind::Product
            && kind != Kind::Subset
            && !self.opts.symbolic_member
            && self.heap.is_finite(set)
            && !self.heap.is_lazy(set)
        {
            let m = self.work_materialize_cached(set)?;
            return self.member(x, m);
        }
        let v = self.heap.view(set);
        match v.kind {
            Kind::Set => {
                let indexed = if self.fuel == u64::MAX && !v.is_lazy() {
                    self.memo.member_index.search(&self.heap, set, x, v.data)?
                } else {
                    None
                };
                if indexed.is_none() {
                    compare::require_materialized(&self.heap, &[x, set])?;
                }
                if indexed
                    .unwrap_or_else(|| v.data.binary_search_by(|&e| self.heap.cmp(e, x)).is_ok())
                {
                    return Ok(true);
                }
                compare::miss_cached(&self.heap, &mut self.memo, x, set, v.data)?;
                Ok(false)
            }
            Kind::Interval => {
                let hi = v.data[0].as_int().unwrap_or(v.lo - 1);
                let lo = v.lo;
                match x.as_int() {
                    Some(i) => Ok(lo <= i && i <= hi),
                    // TLC answers FALSE for an empty interval, and for an
                    // untyped model value.
                    None if lo > hi => Ok(false),
                    None if x.as_model_id().is_some()
                        && compare::mv_type(&self.heap, x).is_none() =>
                    {
                        Ok(false)
                    }
                    // Bounds are inline, so a big integer is outside.
                    None if self.heap.is_int(x) => Ok(false),
                    None => Err(general(
                        "Attempted to check if the value:\na value that is not an integer\nis in the integer interval",
                    )),
                }
            }
            Kind::Nat | Kind::Int if self.heap.is_int(x) => Ok(match x.as_int() {
                Some(i) => v.kind == Kind::Int || i >= 0,
                // A big integer: in `Int`, and in `Nat` when positive.
                None => {
                    v.kind == Kind::Int
                        || self
                            .heap
                            .int_cmp(x, Handle::int(0).unwrap_or(Handle::EMPTY))
                            == Some(std::cmp::Ordering::Greater)
                }
            }),
            Kind::Nat | Kind::Int if x.as_model_id().is_some() => self.mv_member(x),
            Kind::Nat | Kind::Int => Err(eval_exc(
                2158,
                "Attempted to check if the value:\na value that is not an integer\nis an element of Nat or Int.",
            )),
            Kind::Subset => {
                let base = v.data[0];
                let query_kind = self.heap.kind(x);
                if !matches!(query_kind, Some(Kind::Set | Kind::SetEnum | Kind::Interval)) {
                    return Err(eval_err(
                        "Attempted to check if a non-enumerable value is an element of SUBSET",
                    ));
                }
                // SubsetValue.member demands the query's Enumerable.elements.
                // SetEnumValue's enumerator normalizes that actual receiver
                // before the first member call; it does not normalize the base.
                let mut elements = self.work_cursor(x)?;
                while let Some(e) = self.work_next(&mut elements)? {
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
                    // TLC fails on a function it cannot view as a tuple.
                    compare::guard_kind_member(
                        &self.heap,
                        "a membership test of a function that is not a sequence in Seq(S)",
                    )?;
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
                let d = self.work_norm(d)?;
                let dom = self.work_norm(dom)?;
                if !self.heap.eq(d, dom) {
                    // TLC compares the domains with `equals`, which may fail.
                    if self.heap.store().semantics().tlc_comparisons {
                        compare::equals(&self.heap, d, dom)?;
                    }
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
                if xv.kind != Kind::Fcn || xv.keys().iter().any(|k| k.as_str_id().is_none()) {
                    // TLC fails on a function it cannot view as a record.
                    compare::guard_kind_member(
                        &self.heap,
                        "a membership test of a function that is not a record in a set of records",
                    )?;
                    return Ok(false);
                }
                if xv.keys() != names.as_slice() {
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
                if xv.kind == Kind::Fcn && xv.keys().iter().any(|k| k.as_int().is_none()) {
                    // TLC fails on a function over other values than integers.
                    compare::guard_kind_member(
                        &self.heap,
                        "a membership test of a function over non-integers in a set of tuples",
                    )?;
                }
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
            Kind::IntFcn | Kind::Fcn | Kind::Lambda => Err(eval_err("membership in a function")),
            Kind::BigInt => Err(eval_err("membership in a value that is not a set")),
            // TLC's `SetCupValue.member`: either side, left first.
            Kind::Union => {
                let (a, b) = (v.data[0], v.data[1]);
                Ok(self.member(x, a)? || self.member(x, b)?)
            }
            Kind::SetPred => Err(Error::unsupported(
                "membership in a lazy set predicate (the evaluator side is not built)",
            )),
            // TLC's unnormalized `SetEnumValue.member` is a linear scan whose
            // errors differ from a sorted search (C1); its compat-order
            // comparison is not built.
            Kind::SetEnum => Err(Error::unsupported(
                "membership in a set enumeration TLC has not normalized (--compat=tlc)",
            )),
        }
    }

    fn set_bin(&mut self, op: SetOp, x: Handle, y: Handle) -> Result<Handle, Error> {
        let (x, y) = (self.work_norm(x)?, self.work_norm(y)?);
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
                // TLC's `SetCupValue` stays lazy: membership is either
                // side's, and enumerating it fails as TLC's does.
                if !fx || !fy {
                    return Ok(self.heap.union(x, y));
                }
                if let Some(union) = self.union_concrete(x, y)? {
                    return self.checked_set(union);
                }
                let mut v = self.work_elements(x)?;
                v.extend(self.work_elements(y)?);
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
                for e in self.work_elements(a)? {
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
                if let Some(r) = self.minus_small(x, y)? {
                    return Ok(r);
                }
                let mut out = Vec::new();
                for e in self.work_elements(x)? {
                    if !self.member(e, y)? {
                        out.push(e);
                    }
                }
                Ok(self.heap.set_sorted(&out))
            }
        }
    }

    /// Merge canonical explicit operands; the caller retains identity/class checks.
    fn union_concrete(&mut self, x: Handle, y: Handle) -> Result<Option<Handle>, Error> {
        if !x.is_compound() || !y.is_compound() {
            return Ok(None);
        }
        let (left_view, right_view) = (self.heap.view(x), self.heap.view(y));
        if left_view.kind != Kind::Set
            || right_view.kind != Kind::Set
            || left_view.is_lazy()
            || right_view.is_lazy()
        {
            return Ok(None);
        }
        let capacity = left_view
            .data
            .len()
            .checked_add(right_view.data.len())
            .ok_or_else(|| Error::unsupported("set union address space"))?;
        let mut out = Vec::new();
        out.try_reserve_exact(capacity)
            .map_err(|_| Error::unsupported("set union allocation"))?;
        let (mut i, mut j) = (0, 0);
        while i < left_view.data.len() || j < right_view.data.len() {
            self.heap.work_poll()?;
            let value = match (left_view.data.get(i), right_view.data.get(j)) {
                (Some(&left), Some(&right)) => match self.heap.cmp(left, right) {
                    std::cmp::Ordering::Less => {
                        i += 1;
                        left
                    }
                    std::cmp::Ordering::Greater => {
                        j += 1;
                        right
                    }
                    std::cmp::Ordering::Equal => {
                        i += 1;
                        j += 1;
                        left
                    }
                },
                (Some(&left), None) => {
                    i += 1;
                    left
                }
                (None, Some(&right)) => {
                    j += 1;
                    right
                }
                (None, None) => return Err(Error::internal("union cursor past both operands")),
            };
            out.push(value);
        }
        Ok(Some(self.heap.set_sorted(&out)))
    }

    /// `x \ y` for enumerated sets with `y` smaller than `x` (`S \ {e}` in
    /// a recursive fold): each element of `y` is looked up in `x`, instead
    /// of each element of `x` in `y`. The elements of `x` kept are checked
    /// against `y` as a failed membership test checks them
    /// ([`compare::miss`]), so errors are those of the general path.
    fn minus_small(&mut self, x: Handle, y: Handle) -> Result<Option<Handle>, Error> {
        if !x.is_compound() || !y.is_compound() {
            return Ok(None);
        }
        let (vx, vy) = (self.heap.view(x), self.heap.view(y));
        if vx.kind != Kind::Set || vy.kind != Kind::Set || vy.data.len() >= vx.data.len() {
            return Ok(None);
        }
        let (xs, ys) = (vx.data, vy.data);
        let mut drop = Vec::with_capacity(ys.len());
        for &z in ys {
            if let Ok(i) = xs.binary_search_by(|&e| self.heap.cmp(e, z)) {
                drop.push(i);
            }
        }
        drop.sort_unstable();
        let mut out = Vec::with_capacity(xs.len() - drop.len());
        for (i, &e) in xs.iter().enumerate() {
            if drop.binary_search(&i).is_err() {
                compare::miss(&self.heap, e, ys)?;
                out.push(e);
            }
        }
        Ok(Some(self.heap.set_sorted(&out)))
    }

    fn filter(
        &mut self,
        code: &Code,
        bind: &Bind,
        pred: P,
        subset: Option<P>,
    ) -> Result<Handle, Error> {
        if let Some(mut cur) = self.subset_cursor(code, subset)? {
            let mut out = Vec::new();
            while let Some(e) = self.work_next(&mut cur)? {
                self.charge(1)?;
                out.push(e);
            }
            return Ok(self.heap.set_sorted(&out));
        }
        // The cursor enumerates a symbolic domain in canonical order without
        // materializing it (R9, R10).
        let dom = self.eval(code, bind.domain)?;
        let mut cur = self.work_cursor(dom)?;
        let mut out = Vec::new();
        loop {
            // Free a rejected candidate, and the predicate's transients, by
            // arena rollback (R10). Only when no thunk was forced to a
            // transient meanwhile: its slot may hold a value built after the
            // mark (`03-value-model.md` §11.5 rule 2).
            let before = self.heap.mark();
            let Some(e) = self.work_next(&mut cur)? else {
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

    /// The cursor over `SUBSET F` of a proven filter rewrite
    /// (`Node::Filter::subset`), whose elements are exactly the filter's,
    /// in the same canonical order (the subsets of `F` keep their relative
    /// order among those of its superset). An evaluation error while
    /// building `F` declines, so that the filter itself raises the error
    /// TLC raises, where it raises it; `F`'s predicate is pure, so
    /// evaluating it again is unobservable.
    pub(crate) fn subset_cursor(
        &mut self,
        code: &Code,
        subset: Option<P>,
    ) -> Result<Option<Cursor>, Error> {
        let Some(set) = subset else {
            return Ok(None);
        };
        let before = self.heap.mark();
        let forced = self.stats.transient_thunks;
        match self.eval_norm(code, set) {
            Ok(f) => {
                let powerset = self.heap.symbolic(Kind::Subset, 0, &[f]);
                self.work_cursor(powerset).map(Some)
            }
            Err(e) if e.kind() == crate::ErrorKind::Eval => {
                if self.stats.transient_thunks == forced {
                    self.heap.rollback(before);
                }
                Ok(None)
            }
            Err(e) => Err(e),
        }
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
            let d = self.work_norm(d)?;
            let elems = self.work_elements(d)?;
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
                    self.charge(elems.len() + 1)?;
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
        if self.fuel != u64::MAX {
            return self.binding_stream(code, binds, f);
        }
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
                let mut cur = self.work_cursor(d)?;
                while let Some(e) = self.work_next(&mut cur)? {
                    self.charge(1)?;
                    self.bind_one(&binds[0], e)?;
                    if self.eval_bool(code, body)? {
                        return Ok(e);
                    }
                }
                Err(general(
                    "Attempted to compute the value of an expression of form\nCHOOSE x \\in S: P, but no element of S satisfied P.",
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
                vals.push(self.eval_component(code, body)?);
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
            vals.push(self.eval_component(code, body)?);
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
                } else if let Some(i) = self.grid_index(keys, key) {
                    Ok(Some(i))
                } else {
                    Ok(keys.binary_search_by(|&k| self.heap.cmp(k, key)).ok())
                }
            }
            _ => Err(eval_err("application of a set as a function")),
        }
    }

    /// The position of the pair `<<a, b>>` of integers in the keys of a
    /// function whose domain is a dense rectangle `l1..h1 \X l2..h2` in
    /// canonical order (`grid` and `sc` in `GameOfLife`): computed, then
    /// checked against the key found there. `None` sends the caller to the
    /// binary search, so a domain of another shape is still exact.
    fn grid_index(&self, keys: &[Handle], key: Handle) -> Option<usize> {
        let (row, col) = self.int_pair(key)?;
        let (l1, l2) = self.int_pair(*keys.first()?)?;
        let (h1, h2) = self.int_pair(*keys.last()?)?;
        let rows = usize::try_from(h1.checked_sub(l1)?.checked_add(1)?).ok()?;
        let width = usize::try_from(h2.checked_sub(l2)?.checked_add(1)?).ok()?;
        if rows.checked_mul(width)? != keys.len()
            || !(l1..=h1).contains(&row)
            || !(l2..=h2).contains(&col)
        {
            return None;
        }
        let at = usize::try_from(row - l1).ok()? * width + usize::try_from(col - l2).ok()?;
        let k = *keys.get(at)?;
        self.heap.eq(k, key).then_some(at)
    }

    /// `<<a, b>>` for a pair of small integers.
    fn int_pair(&self, h: Handle) -> Option<(i64, i64)> {
        if !h.is_compound() {
            return None;
        }
        let v = self.heap.view(h);
        match (v.kind, v.lo, v.data) {
            (Kind::IntFcn, 1, &[a, b]) => Some((a.as_int()?, b.as_int()?)),
            _ => None,
        }
    }

    /// `f[a]`.
    ///
    /// # Errors
    /// `a` is not in the domain of `f`.
    pub fn apply(&mut self, f: Handle, a: Handle) -> Result<Handle, Error> {
        if let Some(i) = self.fcn_index(f, a)? {
            return Ok(self.heap.view(f).values()[i]);
        }
        let msg = "Attempted to apply function to an argument not in its domain";
        let v = self.heap.view(f);
        match v.kind {
            // TLC compares the argument with the domain on the way: a value
            // of another class fails that comparison first.
            Kind::Fcn => {
                compare::miss(&self.heap, a, v.keys())?;
                Err(general(msg))
            }
            Kind::IntFcn if a.as_int().is_some() => Err(general(msg)),
            _ => Err(eval_err(msg)),
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
        let len = self.heap.view(f).len();
        self.charge(len.saturating_mul(2))?;
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
        self.charge(self.heap.view(f).len())?;
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
            if self.refuse_unrecorded_warnings {
                return Err(Error::unsupported(
                    "an EXCEPT key outside the function's domain, where the reference prints warning 2144",
                ));
            }
            return Ok(f);
        };
        let old = self.heap.view(f).values()[i];
        let new = if depth + 1 == u.path.len() {
            self.set_slot(u.at, old);
            self.eval_component(code, u.value)?
        } else {
            self.except(code, old, u, depth + 1)?
        };
        if new == old {
            return Ok(f);
        }
        self.charge(self.heap.view(f).data.len())?;
        Ok(self.heap.fcn_with(f, i, new))
    }

    // ------------------------------------------------------------------
    // Standard operators
    // ------------------------------------------------------------------

    fn seq_vals(&mut self, s: Handle) -> Result<Vec<Handle>, Error> {
        if !s.is_compound() {
            return Err(eval_err("expected a sequence"));
        }
        let v = self.heap.view(s);
        if v.kind != Kind::IntFcn || v.lo != 1 {
            return Err(eval_err("expected a sequence"));
        }
        let len = v.data.len();
        self.charge(len)?;
        Ok(self.heap.view(s).data.to_vec())
    }

    /// Bags (`tlc2.module.Bags`; the reference engine's
    /// `crates/dive-eval/src/community.rs`).
    fn bag(&mut self, op: StdFn, args: &[Handle]) -> Result<Handle, Error> {
        let arg = |i: usize| {
            args.get(i)
                .copied()
                .ok_or_else(|| Error::internal("missing argument"))
        };
        match op {
            StdFn::EmptyBag => Ok(self.heap.fcn_sorted(&[], &[])),
            StdFn::SetToBag => {
                let set = arg(0)?;
                if !set.is_compound()
                    || matches!(self.heap.view(set).kind, Kind::Fcn | Kind::IntFcn)
                {
                    return Err(eval_err(
                        "The argument of SetToBag should be a finite set, but instead it is not",
                    ));
                }
                let keys = self.work_elements(set)?;
                let one = mk_int(1)?;
                let ones = vec![one; keys.len()];
                Ok(self.heap.fcn_sorted(&keys, &ones))
            }
            StdFn::BagToSet => {
                let bag = arg(0)?;
                if !bag.is_compound()
                    || !matches!(self.heap.view(bag).kind, Kind::Fcn | Kind::IntFcn)
                {
                    return Err(eval_err(
                        "The argument of BagToSet should be a function with a finite domain",
                    ));
                }
                self.domain(bag)
            }
            StdFn::BagIn | StdFn::CopiesIn => {
                let (elem, bag) = (arg(0)?, arg(1)?);
                if !bag.is_compound()
                    || !matches!(self.heap.view(bag).kind, Kind::Fcn | Kind::IntFcn)
                {
                    return Err(eval_err("BagIn or CopiesIn of a value that is not a bag"));
                }
                // TLC compares the element with the bag's domain in order:
                // an element of another class is an error.
                let dom = self.domain(bag)?;
                if !self.member(elem, dom)? {
                    return Ok(if op == StdFn::BagIn {
                        Handle::bool(false)
                    } else {
                        mk_int(0)?
                    });
                }
                let count = self.apply(bag, elem)?;
                let n = count
                    .as_int()
                    .ok_or_else(|| eval_err("The second argument of BagIn should be a bag"))?;
                Ok(if op == StdFn::BagIn {
                    Handle::bool(n > 0)
                } else {
                    count
                })
            }
            _ => Err(Error::internal("not a Bags operator")),
        }
    }

    /// `Len(s)`: a sequence's length, or a string's in UTF-16 units, as
    /// Java counts.
    fn len(&mut self, s: Handle) -> Result<Handle, Error> {
        if let Some(id) = s.as_str_id() {
            let text = self
                .heap
                .store()
                .strings()
                .text(id)
                .ok_or_else(|| Error::internal("unknown string"))?;
            // UTF-8 byte length is O(1) and bounds UTF-16 traversal work.
            // Charge before scanning; the returned length remains Java's.
            self.charge(text.len())?;
            return mk_int(i64::try_from(text.encode_utf16().count()).unwrap_or(i64::MAX));
        }
        // Only a compound handle has a node to view.
        if !s.is_compound() {
            return Err(one_arg("Len", "sequence"));
        }
        let v = self.heap.view(s);
        if v.kind != Kind::IntFcn || v.lo != 1 {
            return Err(one_arg("Len", "sequence"));
        }
        mk_int(i64::try_from(v.data.len()).unwrap_or(i64::MAX))
    }

    fn std_arg(&mut self, code: &Code, f: StdFn, index: usize, x: P) -> Result<Handle, Error> {
        // Finiteness dispatch must inspect the original receiver before an
        // eager projection can erase its range methods or their demand.
        let value = if matches!(f, StdFn::IsFiniteSet | StdFn::Cardinality | StdFn::Seq) {
            self.eval(code, x)?
        } else {
            self.eval_component(code, x)?
        };
        let product = self.heap.kind(value) == Some(Kind::Product);
        if matches!(f, StdFn::Seq | StdFn::IsFiniteSet)
            || (f == StdFn::Append && index == 1)
            || (product && matches!(f, StdFn::Cardinality | StdFn::IsFiniteSet | StdFn::Seq))
        {
            Ok(value)
        } else {
            self.work_norm(value)
        }
    }

    fn std(&mut self, code: &Code, f: StdFn, args: &[P]) -> Result<Handle, Error> {
        let mut a = Vec::with_capacity(args.len());
        for (index, &x) in args.iter().enumerate() {
            a.push(self.std_arg(code, f, index, x)?);
        }
        let arg = |i: usize| {
            a.get(i)
                .copied()
                .ok_or_else(|| Error::internal("missing argument"))
        };
        match f {
            StdFn::Len => self.len(arg(0)?),
            StdFn::Head => {
                let vals = self.seq_vals(arg(0)?)?;
                vals.first()
                    .copied()
                    .ok_or_else(|| eval_exc(2184, "Attempted to apply Head to the empty sequence."))
            }
            StdFn::Tail => {
                let vals = self.seq_vals(arg(0)?)?;
                if vals.is_empty() {
                    return Err(eval_exc(
                        2184,
                        "Attempted to apply Tail to the empty sequence.",
                    ));
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
                let s = arg(0)?;
                if self.heap.kind(s) == Some(Kind::Set) {
                    compare::require_materialized(&self.heap, &[s])?;
                }
                if self.heap.kind(s) == Some(Kind::Product) {
                    return self.product_cardinality(s);
                }
                let kind = self.heap.kind(s);
                match kind {
                    // Not enumerable in TLC.
                    None | Some(Kind::IntFcn | Kind::Fcn | Kind::Nat | Kind::Int) => {
                        return Err(cardinality_error());
                    }
                    Some(Kind::SeqSet) => {
                        return Err(Error::unsupported("Cardinality of Seq(S) (M1 core)"));
                    }
                    Some(_) if !self.heap.is_finite(s) => return Err(cardinality_error()),
                    Some(_) => {}
                }
                let n = self.work_cardinality(s)?;
                mk_int(i64::try_from(n).unwrap_or(i64::MAX))
            }
            StdFn::Seq => {
                let s = arg(0)?;
                Ok(self.heap.symbolic(Kind::SeqSet, 0, &[s]))
            }
            StdFn::IsFiniteSet => self.symbolic_finiteness(arg(0)?).map(Handle::bool),
            StdFn::EmptyBag
            | StdFn::SetToBag
            | StdFn::BagToSet
            | StdFn::BagIn
            | StdFn::CopiesIn => self.bag(f, &a),
        }
    }
}

/// A binding unit (see [`Machine::units`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Unit {
    pub(crate) bind: usize,
    pub(crate) var: Option<usize>,
}

/// The plan's operator as the heap's.
const fn int_op(op: Arith) -> IntOp {
    match op {
        Arith::Add => IntOp::Add,
        Arith::Sub => IntOp::Sub,
        Arith::Mul => IntOp::Mul,
        Arith::Div => IntOp::Div,
        Arith::DivNaturals => IntOp::DivNaturals,
        Arith::Mod => IntOp::Mod,
        Arith::Pow => IntOp::Pow,
    }
}

/// Integer arithmetic on inline operands when its result is certain to be
/// inline and error-free in `mode`; `None` sends it to
/// [`crate::Heap::int_arith`], which handles every other case exactly.
fn fast_arith(op: Arith, x: i64, y: i64, mode: IntMode) -> Option<Handle> {
    let r = match op {
        Arith::Add => x.checked_add(y)?,
        Arith::Sub => x.checked_sub(y)?,
        Arith::Mul => x.checked_mul(y)?,
        // Floored division; in TLC mode only where `Naturals` and
        // `Integers` agree.
        Arith::Div | Arith::DivNaturals if y > 0 && (mode == IntMode::Big || x >= 0) => {
            x.div_euclid(y)
        }
        Arith::Mod if y > 0 => x.rem_euclid(y),
        _ => return None,
    };
    if mode == IntMode::Tlc && i32::try_from(r).is_err() {
        return None;
    }
    Handle::int(r)
}

#[cfg(test)]
#[path = "eval_union_tests.rs"]
mod union_tests;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;
    use crate::TlcClass;

    #[test]
    fn sequences_keep_the_original_product_range_and_its_member_demand() {
        for tlc in [false, true] {
            let semantics = if tlc {
                crate::Semantics::tlc()
            } else {
                crate::Semantics::default()
            };
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                1,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let empty = m.heap.interval(0, -1);
            let product = m.heap.symbolic(Kind::Product, 0, &[empty, zero]);
            let code = Code {
                nodes: vec![Node::Const(product), Node::Std(StdFn::Seq, vec![0].into())],
                ..Code::default()
            };
            let sequences = m.eval(&code, 1).unwrap();
            assert_eq!(m.heap.view(sequences).kind, Kind::SeqSet);
            assert_eq!(m.heap.view(sequences).data, &[product]);
            assert_eq!(m.heap.view(product).data, &[empty, zero]);
            let empty_sequence = m.heap.tuple(&[]);
            assert_eq!(m.member(empty_sequence, sequences), Ok(true));
            let scalar_sequence = m.heap.tuple(&[zero]);
            let error = m.member(scalar_sequence, sequences).unwrap_err();
            assert_eq!(error.kind(), crate::ErrorKind::Eval);
            assert!(error.message().contains("not a function"));
            assert_eq!(m.heap.view(sequences).data, &[product]);

            let mut values = vec![zero];
            let nonempty = m.heap.set_from(&mut values);
            let malformed = m.heap.symbolic(Kind::Product, 0, &[nonempty, zero]);
            let code = Code {
                nodes: vec![
                    Node::Const(malformed),
                    Node::Std(StdFn::Seq, vec![0].into()),
                ],
                ..Code::default()
            };
            let sequences = m.eval(&code, 1).unwrap();
            assert_eq!(m.member(empty_sequence, sequences), Ok(true));
            let element = m.heap.tuple(&[zero, zero]);
            let nonempty_sequence = m.heap.tuple(&[element]);
            let error = m.member(nonempty_sequence, sequences).unwrap_err();
            assert_eq!(error.kind(), crate::ErrorKind::Eval);
            assert!(error.message().contains("not a set"));
            assert_eq!(m.heap.view(sequences).data, &[malformed]);
        }
    }

    #[test]
    fn public_cap_shrink_and_memo_move_cannot_add_over_budget_certificates() {
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        for warm_first in [false, true] {
            let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
            let mut m = Machine::new(
                store.clone(),
                0,
                Opts {
                    memo_cap: 2048,
                    ..Opts::default()
                },
                Memo::new(&[]),
            );
            let vertex = m.heap.tuple(&[one]);
            let set = m.heap.set_from(&mut vec![vertex]);
            let set = m.heap.intern_exact(set).unwrap();
            let query = m.heap.tuple(&[two]);
            if warm_first {
                assert_eq!(m.member(query, set), Ok(false));
                assert_eq!(m.memo.bytes(), 128);
                assert_eq!(Arc::weak_count(&store), 1);
            }
            m.memo.set_cap(64);
            assert_eq!(m.memo.bytes(), 0);
            assert_eq!(Arc::weak_count(&store), 0, "release the certificate owner");
            let expected = compare::miss(&m.heap, query, m.heap.view(set).data).map(|()| false);
            assert_eq!(m.member(query, set), expected);
            assert!(m.memo.bytes() <= 64);
            assert!(!m.memo.members_active());

            let memo = std::mem::take(&mut m.memo);
            let mut moved = Machine::new(
                store,
                0,
                Opts {
                    memo_cap: 64,
                    ..Opts::default()
                },
                memo,
            );
            let query = moved.heap.tuple(&[two]);
            assert_eq!(moved.member(query, set), expected);
            assert!(moved.memo.bytes() <= 64);
            assert!(!moved.heap.poisoned());
        }
    }

    #[test]
    fn prepopulated_public_memo_keeps_membership_allocation_within_its_cap() {
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        for cap in [2048, 2079, 2112, 4096, 1 << 20, crate::memo::DEFAULT_CAP] {
            for (semantics, memo_enabled) in [
                (crate::Semantics::tlc(), true),
                (crate::Semantics::default(), true),
                (crate::Semantics::tlc(), false),
            ] {
                let mut memo = Memo::new(&[1]);
                memo.set_cap(cap);
                memo.put(0, &[one], two);
                assert_eq!(memo.bytes(), 2048);
                let body_bytes = memo.bytes();
                let store = Arc::new(Store::with_semantics(true, semantics));
                let opts = Opts {
                    memo_cap: cap,
                    memo: memo_enabled,
                    ..Opts::default()
                };
                let mut m = Machine::new(store, 0, opts, memo);
                let vertex = m.heap.tuple(&[one]);
                let set = m.heap.set_from(&mut vec![vertex]);
                let set = m.heap.intern_exact(set).unwrap();
                let query = m.heap.tuple(&[two]);
                assert_eq!(m.member(query, set), Ok(false));
                assert!(m.memo.bytes() <= cap);
                assert_eq!(m.memo.get(0, &[one]), Some(two), "keep the body result");
                if cap == 2048 || cap == 2079 || !memo_enabled || !semantics.tlc_comparisons {
                    assert!(!m.memo.members_active());
                    assert_eq!(m.memo.bytes(), body_bytes);
                } else {
                    assert!(m.memo.members_active());
                    assert!(m.memo.bytes() > body_bytes);
                    if cap == 2112 {
                        assert_eq!(m.memo.bytes(), 2112, "only 64 bytes remain");
                    }
                }
                if cap == 2048 {
                    m.memo.set_cap(64);
                    assert_eq!(m.memo.bytes(), body_bytes, "retain existing body storage");
                    assert_eq!(m.memo.get(0, &[one]), Some(two));
                    assert_eq!(m.member(query, set), Ok(false));
                    assert_eq!(m.memo.bytes(), body_bytes, "add no certificate bytes");
                }
                // Subsequent ordinary table growth must respect the reduced
                // allowance as well as the pre-existing table allocation.
                for i in 1..1000 {
                    m.memo.put(0, &[Handle::int(i).unwrap()], two);
                    assert!(m.memo.bytes() <= cap);
                }
                assert!(!m.heap.poisoned());
            }
        }
    }

    #[test]
    fn public_heap_replacement_cannot_reuse_another_stores_membership_proof() {
        fn values(heap: &mut Heap, name: &str) -> (Handle, Handle) {
            let one = Handle::int(1).unwrap();
            let model = Handle::model(heap.store().strings().intern(name).unwrap());
            let witness = heap.tuple(&[one]);
            let witness = heap.intern_exact(witness).unwrap();
            let other = heap.tuple(&[model]);
            let other = heap.intern_exact(other).unwrap();
            let set = heap.set_from(&mut vec![witness, other]);
            let set = heap.intern_exact(set).unwrap();
            (set, witness)
        }

        for hashcons in [true, false] {
            let store = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let old_owner = Arc::downgrade(&store);
            let mut m = Machine::new(store, 0, Opts::default(), Memo::new(&[]));
            let (old_set, old_witness) = values(&mut m.heap, "u");
            let two = Handle::int(2).unwrap();
            let query = m.heap.tuple(&[two]);
            assert_eq!(m.member(query, old_set), Ok(false));
            assert!(m.memo.bytes() > 0);

            let store = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let mut replacement = Heap::new(store, false, false);
            let (set, witness) = values(&mut replacement, "T_x");
            assert_eq!(old_set, set, "complete set handles collide across stores");
            assert_eq!(old_witness, witness, "witness handles also collide");
            m.heap = replacement;
            assert!(
                old_owner.upgrade().is_none(),
                "a certificate does not retain Store"
            );
            let query = m.heap.tuple(&[two]);
            let expected = compare::miss(&m.heap, query, m.heap.view(set).data).map(|()| false);
            assert!(expected.is_err(), "typed versus integer constraints refuse");
            assert_eq!(m.member(query, set), expected, "warm equals the full miss");
            assert_eq!(m.memo.bytes(), 0, "rebind drops the old entries");
            m.memo.clear();
            assert_eq!(m.member(query, set), expected, "cold equals the full miss");
            assert!(!m.heap.poisoned());
        }
    }

    #[test]
    fn membership_certificates_respect_profiles_epochs_and_machine_stores() {
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        for hashcons in [false, true] {
            let store = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let opts = Opts {
                memo_cap: 512,
                ..Opts::default()
            };
            let mut m = Machine::new(store.clone(), 0, opts, Memo::new(&[]));
            let u = Handle::model(store.strings().intern("u").unwrap());
            let genesis = m.heap.tuple(&[]);
            let vertex = m.heap.tuple(&[u, one]);
            let set = m.heap.set_from(&mut vec![genesis, vertex]);
            let set = m.heap.intern_exact(set).unwrap();
            let query = m.heap.tuple(&[u, two]);
            assert_eq!(m.member(query, set), Ok(false));
            let vertex = m.memo.member_witness(m.heap.store(), set).unwrap();
            assert_eq!(m.memo.bytes(), 32);
            m.heap.reset();
            let mark = m.heap.mark();
            let query = m.heap.tuple(&[u, two]);
            assert_eq!(m.member(query, set), Ok(false));
            m.heap.rollback(mark);
            let query = m.heap.tuple(&[u, two]);
            assert_eq!(m.member(query, set), Ok(false));
            let ephemeral = m.heap.set_from(&mut vec![query]);
            assert_eq!(m.member(vertex, ephemeral), Ok(false));
            assert_eq!(m.memo.member_witness(m.heap.store(), ephemeral), None);
            assert!(!m.heap.poisoned());

            let mut worker = Machine::new(store, 0, opts, Memo::new(&[]));
            assert_eq!(worker.memo.member_witness(worker.heap.store(), set), None);
            let query = worker.heap.tuple(&[u, two]);
            assert_eq!(worker.member(query, set), Ok(false));
            assert!(
                worker
                    .memo
                    .member_witness(worker.heap.store(), set)
                    .is_some()
            );
            let reused = std::mem::take(&mut worker.memo);
            let other = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let mut worker = Machine::new(other, 0, opts, reused);
            assert_eq!(worker.memo.member_witness(worker.heap.store(), set), None);
            assert_eq!(worker.memo.bytes(), 0);
        }
        for (semantics, memo_enabled) in [
            (crate::Semantics::default(), true),
            (crate::Semantics::tlc(), false),
        ] {
            let store = Arc::new(Store::with_semantics(true, semantics));
            let mut m = Machine::new(
                store,
                0,
                Opts {
                    memo: memo_enabled,
                    ..Opts::default()
                },
                Memo::new(&[]),
            );
            assert!(!m.memo.members_active());
            let vertex = m.heap.tuple(&[one]);
            let set = m.heap.set_from(&mut vec![vertex]);
            let set = m.heap.intern_exact(set).unwrap();
            let query = m.heap.tuple(&[two]);
            assert_eq!(m.member(query, set), Ok(false));
            assert_eq!(m.memo.bytes(), 0);
        }
    }

    #[test]
    fn arithmetic_errors_carry_tlc_classes() {
        let exc = |c| Some(TlcClass::eval_exception(c));
        for mode in [crate::int::IntMode::Big, crate::int::IntMode::Tlc] {
            let mut h =
                crate::Heap::new(std::sync::Arc::new(crate::Store::default()), false, false);
            h.set_int_mode(mode);
            let n = |v| Handle::int(v).unwrap();
            let mut class = |op, x, y| h.int_arith(op, n(x), n(y)).unwrap_err().tlc();
            assert_eq!(class(IntOp::Div, 1, 0), exc(2179));
            assert_eq!(class(IntOp::Mod, 1, 0), exc(2169));
            assert_eq!(class(IntOp::Pow, 2, -1), exc(2169));
            assert_eq!(class(IntOp::Pow, 0, 0), exc(2180));
        }
        let mut h = crate::Heap::new(std::sync::Arc::new(crate::Store::default()), false, false);
        h.set_int_mode(crate::int::IntMode::Tlc);
        let big = h
            .int_arith(
                IntOp::Mul,
                Handle::int(1 << 20).unwrap(),
                Handle::int(1 << 20).unwrap(),
            )
            .unwrap_err();
        assert_eq!(big.tlc(), exc(2178));
    }
}

#[cfg(test)]
mod recursive_call_bounds {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct StopAfterOne(AtomicUsize);
    impl WorkControl for StopAfterOne {
        fn proceed(&self) -> bool {
            self.0.fetch_add(1, Ordering::Relaxed) == 0
        }
    }

    #[test]
    fn recursive_calls_poll_work_and_restore_depth_and_frames_after_stops()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = Code {
            nodes: vec![Node::Call {
                fun: 0,
                args: Box::new([]),
            }],
            funs: vec![crate::plan::Fun {
                name: "loop".into(),
                params: 0,
                frame: 0,
                body: 0,
                caps: Box::new([]),
            }],
            ..Default::default()
        };
        let mut m = Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            Memo::default(),
        );
        let control = Arc::new(StopAfterOne(AtomicUsize::new(0)));
        m.set_work_control(Some(control.clone()));
        let stopped = m.eval(&code, 0).err().ok_or("expected cooperative stop")?;
        assert_eq!(stopped.kind(), crate::ErrorKind::Stop);
        assert_eq!(control.0.load(Ordering::Relaxed), 2);
        assert_eq!(m.depth, 0);
        assert_eq!(m.frame, Vec::<Handle>::new());
        m.set_work_control(None);
        let bounded = m.eval(&code, 0).err().ok_or("expected depth refusal")?;
        assert_eq!(bounded.kind(), crate::ErrorKind::Unsupported);
        assert!(bounded.message().contains("recursive-call depth limit"));
        assert_eq!(m.depth, 0);
        assert_eq!(m.frame, Vec::<Handle>::new());
        Ok(())
    }
}
