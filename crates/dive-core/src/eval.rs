//! The interpreter for value nodes: a recursive
//! `match` over the plan's node array, over one flat frame per activation.

use std::sync::Arc;

use crate::Error;
use crate::compare;
use crate::handle::Handle;
use crate::heap::{Cursor, Heap};
use crate::int::{IntMode, IntOp};
use crate::memo::{MAX_KEY, Memo};
use crate::node::{Kind, View};
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

/// Below this many elements, membership of a compound value in an enumerated
/// set scans element hashes instead of binary-searching by canonical order:
/// one header read per element instead of a structural comparison per probe.
const SMALL_MEMBER_SCAN: usize = 16;

/// Charged units between polls of the installed [`WorkControl`]. Every
/// charge counts at least one unit, so the checkpoint runs at least once per
/// this many charges and before any charge that would carry the units since
/// the last poll to this bound. The first charge after
/// [`Machine::set_work_control`], and every charge after a refused poll,
/// polls at once.
pub const WORK_POLL_UNITS: u64 = 32;

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
    deferred_print: Option<bool>,
    output_limits: Option<(usize, usize)>,
    output_used: (usize, usize),
    output_failure: Option<&'static str>,
    pub(crate) checker_access: CheckerAccess,
    /// The random stream draws come from: the startup stream or the
    /// expanded state's (`random.rs`); `None` outside a search.
    pub draw: Option<Box<dyn crate::random::DrawStream>>,
    /// How many draws were made, which the engine reads to refuse a draw
    /// in a check.
    pub draws: u64,
    /// Every value drawn since the engine last reset it, in order and
    /// interned, when the engine records draws (simulation records them in
    /// the trace document for the validator); `None` records nothing.
    pub random_log: Option<Vec<Handle>>,
    /// `ENABLED` queries being evaluated, innermost last: a simulation
    /// stream refuses to draw inside one.
    pub(crate) in_enabled: u32,
    /// Draws members of sets a generator cannot enumerate, in simulation
    /// only (`crate::unbounded`); `None` keeps the "not enumerable" error.
    pub unbounded: Option<crate::unbounded::Unbounded>,
    /// `TLCGet("level")`: 0 in `Init`, the expanded state's level in the
    /// next-state relation and action checks, the checked state's level in
    /// state checks (initial states are level 1). The engine sets it.
    pub level: i64,
    pub(crate) conts: Vec<crate::generate::ContNode>,
    pub(crate) cvals: Vec<Handle>,
    pub(crate) saved: Vec<Handle>,
    scratch: Vec<Vec<Handle>>,
    /// The absolute frame positions of thunk slots forced to a transient,
    /// in forcing order ([`Machine::thunk_mark`]). A position at or past
    /// the frame's length names a slot that has left the frame.
    transient_slots: Vec<usize>,
    depth: usize,
    /// Loop iterations left before evaluation gives up. Unbounded at run
    /// time; the compiler bounds it while folding constants, and an
    /// isolated optional analysis machine bounds its own speculative work.
    pub(crate) fuel: u64,
    /// Whether a site where the dive-eval evaluator prints a warning that
    /// Core does not record in [`Machine::warnings`] refuses instead
    /// ([`Machine::set_refuse_unrecorded_warnings`]).
    refuse_unrecorded_warnings: bool,
    /// Persistent handles the caller proved materialized
    /// ([`Machine::set_proven_materialized`]): the rows it loaded into
    /// `s0` and `s1`, position for position, and others.
    proven_s0: Vec<Handle>,
    proven_s1: Vec<Handle>,
    proven: Vec<Handle>,
    work_control: Option<Arc<dyn WorkControl>>,
    /// Charged units left before the next [`WorkControl`] poll, while fuel
    /// and the transient bound are unbounded ([`Machine::charge`]).
    work_countdown: u64,
    /// The same countdown while either is bounded.
    bounded_countdown: u64,
    pub(crate) debugger: Option<Arc<dyn crate::debug::Observer>>,
    /// Results of calls, kept only while the compiler evaluates start-up
    /// constant definitions (see [`CallCache`]).
    pub(crate) calls: Option<CallCache>,
    /// The most transient bytes an evaluation may hold
    /// ([`crate::compile::STARTUP_TRANSIENT_BYTES`] while the compiler
    /// evaluates start-up constant definitions); unbounded otherwise.
    pub(crate) transient_bound: usize,
}

/// What of a live checker `TLCGet`/`TLCSet` may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckerAccess {
    /// Registers and `TLCGet("level")` (model checking).
    Full,
    /// `TLCGet("level")` only, set by a finite mode's driver.
    Level,
    /// Nothing: a finite mode without a checker context.
    Denied,
}

/// The results of the calls made while the compiler evaluates the pure
/// constant definitions the binder left to the core, keyed by
/// the function and the exact argument handles.
///
/// A start-up constant definition reads no state and applies no impure
/// operator (`dive_model::bind_deferring_constants` checks it through every
/// definition it reaches), so a call's result is a function of its
/// arguments: captures are passed as arguments, and a frame starts empty.
/// Equal handles are equal values while no transient they name has been
/// freed, so the table is emptied whenever the arena's reset or rollback
/// count changes; within one count a kept transient result is still live.
#[derive(Debug, Default)]
pub(crate) struct CallCache {
    revision: (u64, u64),
    results: std::collections::HashMap<Box<[u64]>, Handle>,
    key: Vec<u64>,
    /// Lookups answered from the table.
    pub(crate) hits: u64,
}

/// The most results [`CallCache`] holds before it starts again.
const CALL_CACHE_ENTRIES: usize = 1 << 16;

fn eval_err(what: &str) -> Error {
    Error::eval(what)
}

/// An `Assert.fail(EC.GENERAL, ...)` error the core raises exactly where
/// TLC does. Messages that begin like TLC's keep TLC's beginning, which the
/// engine's replay rule reads (`dive_runtime::classify`).
fn general(what: &str) -> Error {
    Error::eval_class(crate::TlcClass::GENERAL, what)
}

/// An `EvalException` of TLC class `code`.
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
            deferred_print: None,
            output_limits: None,
            output_used: (0, 0),
            output_failure: None,
            checker_access: CheckerAccess::Full,
            draw: None,
            draws: 0,
            random_log: None,
            in_enabled: 0,
            unbounded: None,
            level: 0,
            conts: Vec::new(),
            cvals: Vec::new(),
            saved: Vec::new(),
            scratch: Vec::new(),
            transient_slots: Vec::new(),
            depth: 0,
            fuel: u64::MAX,
            refuse_unrecorded_warnings: false,
            proven_s0: Vec::new(),
            proven_s1: Vec::new(),
            proven: Vec::new(),
            work_control: None,
            work_countdown: 0,
            bounded_countdown: 0,
            debugger: None,
            calls: None,
            transient_bound: usize::MAX,
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
        self.disarm_inline_charge();
        self.fuel = units;
        self.memo.reserve_member_index(
            units == u64::MAX
                && self.opts.member_index
                && !self.heap.store.semantics().tlc_comparisons,
        );
    }

    /// Whether a bounded machine has spent all of its fuel
    /// ([`Machine::set_fuel`]), as the last charge that failed leaves it.
    ///
    /// ```
    /// # fn inspect(machine: &dive_core::Machine) {
    /// let _ = machine.fuel_exhausted();
    /// # }
    /// ```
    #[must_use]
    pub const fn fuel_exhausted(&self) -> bool {
        self.fuel == 0
    }

    /// Makes every site where the dive-eval evaluator prints a warning that
    /// this machine does not record in [`Machine::warnings`] fail as
    /// unsupported instead: an `EXCEPT` key outside the function's domain
    /// (TLC warning 2144). An isolated optional machine sets it so that its
    /// caller can fall back to the dive-eval evaluator, which prints the
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

    /// Records handles the caller proved materialized in this machine's
    /// store with [`Heap::proved_concrete`] or [`Heap::materialized_within`]:
    /// the rows `s0` and `s1` it is about to load, and `others`. It forgets
    /// the previous ones. Identity tests that require materialized operands
    /// (a `SameVars` subscript, set membership) then skip re-proving those
    /// exact handles. Only persistent compound handles are kept: persistent
    /// nodes never change, so the proof stays true. An isolated analysis
    /// machine passes the rows and constants it admitted before each
    /// evaluation.
    ///
    /// Precondition, which this method does not check: every recorded handle
    /// satisfies [`Heap::materialized_within`] in this machine's store.
    /// Recording any other handle (a lazy or symbolic set, say) makes
    /// membership binary-search a non-canonical node and can give a wrong
    /// answer. That walk has no depth limit, unlike the internal check it
    /// replaces, which declines values nested 64 deep; the skipped check
    /// would then have refused a value whose identity is nonetheless exact,
    /// so the recorded proof only answers more of the same questions.
    ///
    /// ```
    /// # fn configure(machine: &mut dive_core::Machine, row: &[dive_core::Handle]) {
    /// machine.set_proven_materialized(row, &[], []);
    /// # }
    /// ```
    pub fn set_proven_materialized(
        &mut self,
        s0: &[Handle],
        s1: &[Handle],
        others: impl IntoIterator<Item = Handle>,
    ) {
        let keep = |h: &Handle| {
            if h.is_persistent() && h.is_compound() {
                *h
            } else {
                Handle::EMPTY
            }
        };
        self.proven_s0.clear();
        self.proven_s0.extend(s0.iter().map(keep));
        self.proven_s1.clear();
        self.proven_s1.extend(s1.iter().map(keep));
        self.proven.clear();
        self.proven.extend(
            others
                .into_iter()
                .filter(|h| h.is_persistent() && h.is_compound()),
        );
    }

    /// Whether `h` is a handle the caller proved materialized
    /// ([`Machine::set_proven_materialized`]).
    fn proven(&self, h: Handle) -> bool {
        h.is_compound()
            && (self.proven_s0.contains(&h)
                || self.proven_s1.contains(&h)
                || self.proven.contains(&h))
    }

    /// Whether `h`, the value of variable `v` in `s0` (`primed`: `s1`), is
    /// the proven handle at that position, or otherwise proven.
    #[inline]
    fn proven_at(&self, v: usize, primed: bool, h: Handle) -> bool {
        let row = if primed {
            &self.proven_s1
        } else {
            &self.proven_s0
        };
        (h.is_compound() && row.get(v) == Some(&h)) || self.proven(h)
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
        self.work_countdown = 0;
        self.bounded_countdown = 0;
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
        self.checker_access = CheckerAccess::Denied;
    }

    /// Keeps `TLCGet("level")` available in a finite mode whose driver sets
    /// [`Machine::level`] the way TLC does for that mode. Register reads and
    /// writes still refuse.
    pub fn allow_finite_level(&mut self) {
        if self.checker_access == CheckerAccess::Denied {
            self.checker_access = CheckerAccess::Level;
        }
    }

    fn check_finite_access(&self, op: dive_ir::StdOp) -> Result<(), Error> {
        let allowed = match self.checker_access {
            CheckerAccess::Full => true,
            CheckerAccess::Level => op == dive_ir::StdOp::TlcTLCGet,
            CheckerAccess::Denied => false,
        };
        // A finite mode that installed a draw stream (simulation) draws.
        let drawn = self.draw.is_some() && crate::stdlib::random_op(op);
        if !allowed
            && !drawn
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

    /// Takes the retained Print output and the first limit it reached, and
    /// starts counting against the output limits afresh. A finite mode that
    /// buffers output per unit of work (a simulation trace) admits each
    /// unit's output in its own order.
    pub fn take_output(&mut self) -> (Vec<String>, Option<&'static str>) {
        self.output_used = (0, 0);
        (
            std::mem::take(&mut self.printed),
            self.output_failure.take(),
        )
    }

    /// Begins or ends speculative evaluation of a source with ordered output.
    /// Returns whether a print was demanded since speculation began. The caller
    /// must discard that source and run it again in commit order.
    pub fn defer_prints(&mut self, enabled: bool) -> bool {
        std::mem::replace(&mut self.deferred_print, enabled.then_some(false)) == Some(true)
    }

    pub(crate) fn print(&mut self, text: String) -> Result<(), Error> {
        if let Some(requested) = &mut self.deferred_print {
            *requested = true;
            return Err(Error::stop());
        }
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
    ///
    /// The inline path only counts down to the next [`WorkControl`] poll
    /// ([`WORK_POLL_UNITS`]). Its countdown is armed only while fuel and the
    /// start-up transient bound are unbounded. A machine with finite fuel and
    /// no transient bound spends its fuel inline while the poll countdown and
    /// the fuel both cover the charge, exactly as [`Machine::charge_slow`]
    /// would; every other bounded charge takes the slow path.
    #[inline]
    pub(crate) fn charge(&mut self, n: usize) -> Result<(), Error> {
        let units = (n as u64).max(1);
        if units < self.work_countdown {
            self.work_countdown -= units;
            return Ok(());
        }
        let n64 = n as u64;
        if units < self.bounded_countdown
            && self.fuel != u64::MAX
            && n64 <= self.fuel
            && self.transient_bound == usize::MAX
        {
            self.bounded_countdown -= units;
            self.fuel -= n64;
            return Ok(());
        }
        self.charge_slow(units, n)
    }

    /// The checks [`Machine::charge`] amortizes or skips: the work poll
    /// once the units since the last one reach [`WORK_POLL_UNITS`], and the
    /// start-up transient bound and finite fuel, exactly. The units left
    /// before the next poll live in `work_countdown` while the machine is
    /// unbounded and in `bounded_countdown` otherwise; at most one is
    /// nonzero. A refused poll leaves both at zero, so every later charge
    /// polls again.
    #[inline(never)]
    fn charge_slow(&mut self, units: u64, n: usize) -> Result<(), Error> {
        let left = self.work_countdown.max(self.bounded_countdown);
        self.work_countdown = 0;
        self.bounded_countdown = 0;
        if units < left {
            self.bounded_countdown = left - units;
        } else if self
            .work_control
            .as_ref()
            .is_some_and(|control| !control.proceed())
        {
            self.heap.mark_work_interrupted();
            return Err(Error::stop());
        } else {
            self.bounded_countdown = WORK_POLL_UNITS;
        }
        if self.transient_bound != usize::MAX && self.heap.arena.footprint() > self.transient_bound
        {
            return Err(Error::unsupported("the start-up evaluation memory bound"));
        }
        if self.fuel == u64::MAX {
            if self.transient_bound == usize::MAX {
                self.work_countdown = std::mem::take(&mut self.bounded_countdown);
            }
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

    /// Moves the poll countdown off the inline path of [`Machine::charge`]
    /// before fuel or the transient bound changes, keeping the units left.
    fn disarm_inline_charge(&mut self) {
        self.bounded_countdown = self
            .bounded_countdown
            .max(std::mem::take(&mut self.work_countdown));
    }

    /// Bounds the transient bytes an evaluation may hold; [`Machine::charge`]
    /// checks the bound exactly while it is finite.
    pub(crate) fn set_transient_bound(&mut self, bytes: usize) {
        self.disarm_inline_charge();
        self.transient_bound = bytes;
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
        if self.frame.is_empty() {
            // Every recorded thunk slot has left the frame.
            self.transient_slots.clear();
        }
    }

    /// The point from which [`Machine::thunks_released`] considers the thunk
    /// slots forced to a transient. Take it with the [`Heap::mark`] that a
    /// later rollback frees to.
    #[must_use]
    pub fn thunk_mark(&self) -> usize {
        self.transient_slots.len()
    }

    /// Whether no thunk slot still in the frame holds a transient forced
    /// since `since`, so that a rollback
    /// to the heap mark taken with it frees no value a live slot holds.
    /// A slot forced in a frame that has since been left is gone: entering
    /// a frame fills fresh slots with [`Handle::EMPTY`]. Forgets the
    /// positions of slots that have left the frame; a true result forgets
    /// every position recorded since `since`.
    pub fn thunks_released(&mut self, since: usize) -> bool {
        let len = self.transient_slots.len();
        if since >= len {
            return true;
        }
        let live = self.frame.len();
        let mut kept = since;
        let mut i = since;
        while i < len {
            let slot = self.transient_slots[i];
            if slot < live {
                self.transient_slots[kept] = slot;
                kept += 1;
            }
            i += 1;
        }
        self.transient_slots.truncate(kept);
        kept == since
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

    /// Whether a [`Node::SameVars`] may compare its leaves directly. Its
    /// `Eq` forces the thunks, whose bodies build both tuples from the
    /// variables (`eval_component`); `compare::equals` then requires every
    /// leaf materialized at its depth and, outside TLC comparisons, returns
    /// the tuples' `Heap::eq`, which is the conjunction of the leaves'
    /// `Heap::eq` (the same tree, so equal arities). A materialized leaf is
    /// neither symbolic nor a product, so `eval_component` returns it
    /// unchanged. Skipping an unforced thunk only leaves it for a later
    /// demand. TLC comparisons (whose shape guard may refuse), an observed
    /// debugger, a forced thunk (whose cached value `Eq` would read), an
    /// unassigned variable or an unmaterialized leaf evaluate the `Eq`.
    fn same_vars_direct(&self, leaves: &[(u16, u8)], thunks: &[u32]) -> Result<bool, Error> {
        if self.debugger.is_some()
            || self.heap.store().semantics().tlc_comparisons
            || thunks.iter().any(|&slot| !self.slot(slot).is_empty())
        {
            return Ok(false);
        }
        for &(v, depth) in leaves {
            let v = usize::from(v);
            let (Some(&a), Some(&b)) = (self.s0.get(v), self.s1.get(v)) else {
                return Ok(false);
            };
            if !(self.proven_at(v, false, a) || compare::materialized(&self.heap, a, depth)?)
                || (a != b
                    && !(self.proven_at(v, true, b)
                        || compare::materialized(&self.heap, b, depth)?))
            {
                return Ok(false);
            }
        }
        Ok(true)
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
                if v.is_transient() {
                    self.stats.transient_thunks += 1;
                    self.transient_slots.push(self.base + *slot as usize);
                }
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
            Node::SameVars { eq, leaves, thunks } => {
                if self.same_vars_direct(leaves, thunks)? {
                    let same = leaves.iter().all(|&(v, _)| {
                        let v = usize::from(v);
                        match (self.s0.get(v), self.s1.get(v)) {
                            (Some(&a), Some(&b)) => self.heap.eq(a, b),
                            _ => false,
                        }
                    });
                    return Ok(Handle::bool(same));
                }
                self.eval(code, *eq)
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
                // A bounded machine pays for a big power's digits before
                // they are allocated.
                if *op == Arith::Pow && self.fuel != u64::MAX {
                    let words = self.heap.pow_result_words(x, y);
                    self.charge(usize::try_from(words).unwrap_or(usize::MAX))?;
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
            Node::PrefixFcn {
                bind,
                function,
                body,
            } => self.prefix_fcn(code, bind, *function, *body),
            Node::Quant { kind, binds, body } => self.quant(code, *kind, binds, *body),
            Node::Apply(f, a) => {
                if self.opts.actor_rows
                    && self.fuel == u64::MAX
                    && self.debugger.is_none()
                    && let Some(value) = self.direct_cell(code, *f, *a)
                {
                    return Ok(value);
                }
                let fv = self.eval(code, *f)?;
                let av = self.eval_norm(code, *a)?;
                self.apply(fv, av)
            }
            Node::Select(r, field) => {
                if self.opts.actor_rows
                    && self.fuel == u64::MAX
                    && self.debugger.is_none()
                    && let Some(Node::Apply(table, key)) = code.nodes.get(*r as usize)
                    && let Some(value) = self.direct_cell(code, *table, *key)
                {
                    return self.select(value, *field);
                }
                let rv = self.eval(code, *r)?;
                self.select(rv, *field)
            }
            Node::Domain(f) => {
                let fv = self.eval(code, *f)?;
                self.domain(fv)
            }
            Node::Except {
                base,
                updates,
                row_fields,
            } => {
                let mut f = self.eval_norm(code, *base)?;
                if self.opts.actor_rows
                    && let Some(fields) = row_fields
                    && self.fuel == u64::MAX
                    && self.debugger.is_none()
                    && self.transient_bound == usize::MAX
                    && let Some(result) = self.except_row(code, f, updates, fields)?
                {
                    return Ok(result);
                }
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
                // A call the core does not reproduce refuses before its
                // arguments are evaluated: an argument may itself be such a
                // call, or have an effect TLC would order differently.
                if crate::stdlib::deferred(*op) {
                    return Err(crate::stdlib::deferred_refusal(*op));
                }
                // A value printed keeps its form: TLC prints an interval as
                // `1..3` (`crate::print`), and draws from a lazy `[S -> T]`
                // one value per component (`crate::random`).
                let prints = matches!(
                    op,
                    dive_ir::StdOp::TlcPrint
                        | dive_ir::StdOp::TlcPrintT
                        | dive_ir::StdOp::TlcToString
                        | dive_ir::StdOp::TlcRandomElement
                        | dive_ir::StdOp::RandomizationRandomSubset
                        | dive_ir::StdOp::RandomizationRandomSetOfSubsets
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
            Node::Composition {
                first, then, reset, ..
            } => self.composition_holds(code, *first, *then, reset),
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
    /// whose bound variables have no tuple pattern. The function's domains
    /// are evaluated first, in order (TLC builds the lazy function before
    /// evaluating the argument), then the argument. With one bound variable
    /// the argument must be in the domain; with several it is a tuple of
    /// their number whose components, in order, must be in their domains.
    /// The body is then evaluated for that argument alone.
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
                if let [bind] = &binds[..]
                    && let [slot] = bind.slots[..]
                {
                    return self.beta_one(code, bind.domain, slot, arg, *body);
                }
                self.beta_many(code, chain, binds, arg, *body)
            }
            _ => Err(Error::internal("a lazy application of a non-function node")),
        }
    }

    fn beta_one(
        &mut self,
        code: &Code,
        domain: P,
        slot: u32,
        arg: P,
        body: P,
    ) -> Result<Handle, Error> {
        let dom = self.eval(code, domain)?;
        let x = self.eval_norm(code, arg)?;
        if !self.member(x, dom)? {
            return Err(general(
                "In applying the function\na function, the argument is not in its domain",
            ));
        }
        let old = self.slot(slot);
        self.set_slot(slot, x);
        let out = self.eval(code, body);
        self.set_slot(slot, old);
        out
    }

    /// [`Machine::beta`] with several bound variables. An argument that is
    /// not a tuple of their number takes the constructor's speculative
    /// eager build and ordinary application instead, unchanged.
    fn beta_many(
        &mut self,
        code: &Code,
        fc: P,
        binds: &[Bind],
        arg: P,
        body: P,
    ) -> Result<Handle, Error> {
        if binds.iter().any(|b| b.tuple || b.slots.is_empty()) {
            return Err(Error::internal(
                "a lazy application with a tuple pattern or no bound variable",
            ));
        }
        let mut doms = Vec::with_capacity(binds.len());
        for b in binds {
            doms.push(self.eval(code, b.domain)?);
        }
        let x = self.eval_norm(code, arg)?;
        let arity: usize = binds.iter().map(|b| b.slots.len()).sum();
        let parts = if x.is_compound() {
            let v = self.heap.view(x);
            (v.kind == Kind::IntFcn && v.lo == 1 && v.data.len() == arity).then(|| v.data.to_vec())
        } else {
            None
        };
        let Some(parts) = parts else {
            let f = self.eval(code, fc).map_err(speculative)?;
            return self.apply(f, x);
        };
        let mut k = 0;
        for (b, &dom) in binds.iter().zip(&doms) {
            for _ in &b.slots {
                if !self.member(parts[k], dom)? {
                    return Err(general(
                        "In applying the function\na function, the argument is not in its domain",
                    ));
                }
                k += 1;
            }
        }
        let mark = self.saved.len();
        for &s in binds.iter().flat_map(|b| b.slots.iter()) {
            let v = self.slot(s);
            self.saved.push(v);
        }
        for (&s, &p) in binds.iter().flat_map(|b| b.slots.iter()).zip(&parts) {
            self.set_slot(s, p);
        }
        let out = self.eval(code, body);
        for (k, &s) in binds.iter().flat_map(|b| b.slots.iter()).enumerate() {
            let v = self.saved.get(mark + k).copied().unwrap_or(Handle::EMPTY);
            self.set_slot(s, v);
        }
        self.saved.truncate(mark);
        out
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
            // A primed definition's site is decided on its body, as the
            // inline body is: the test builds nothing to cache
            // (`Compiler::lazy_set`).
            Some(Node::Memo { site, body })
                if code.sites.get(*site as usize).is_some_and(|s| s.candidate) =>
            {
                self.member_lazy(code, x, *body)
            }
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
        // A candidate site's persistent copies are charged to a bounded
        // budget; once it is spent the site is off (`Memo::charge_kept`).
        if s.candidate && !self.memo.may_keep() {
            self.memo.charge_kept(site, 0);
            return self.eval(code, body);
        }
        let added = self.heap.cache.added;
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
                    if s.candidate {
                        self.charge_kept(site, added);
                    }
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
        if s.candidate {
            self.charge_kept(site, added);
        }
        if let Some(r) = self.memo.get(site, &key[..n]) {
            return Ok(r);
        }
        let r = self.eval(code, body)?;
        let added = self.heap.cache.added;
        // A step site keeps an exact persistent copy of its result; one
        // that cannot be kept (lazy) is returned uncached.
        let kept = if s.step && r.is_transient() {
            self.heap.intern_exact(r).unwrap_or(r)
        } else {
            r
        };
        if s.candidate {
            self.charge_kept(site, added);
        }
        if !kept.is_transient() {
            self.memo.put(site, &key[..n], kept);
        }
        Ok(kept)
    }

    /// Charges a candidate site for the store bytes this heap has added
    /// since `since` (`InternCache::added`, `Memo::charge_kept`).
    fn charge_kept(&mut self, site: u32, since: u64) {
        let bytes = self.heap.cache.added.saturating_sub(since);
        self.memo.charge_kept(site, bytes);
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
        let cached = match &mut self.calls {
            Some(cache) if !self.heap.speculative => {
                let revision = self.heap.arena.cache_revision();
                if cache.revision != revision {
                    cache.results.clear();
                    cache.revision = revision;
                }
                cache.key.clear();
                cache.key.push(u64::from(fun));
                cache.key.extend(vals.iter().map(|h| h.0));
                if let Some(&r) = cache.results.get(cache.key.as_slice()) {
                    cache.hits += 1;
                    self.give_scratch(vals);
                    return Ok(r);
                }
                Some(cache.key.clone().into_boxed_slice())
            }
            _ => None,
        };
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
        if let (Some(key), Ok(h), Some(cache)) = (cached, &r, &mut self.calls)
            && cache.revision == self.heap.arena.cache_revision()
        {
            if cache.results.len() >= CALL_CACHE_ENTRIES {
                cache.results.clear();
            }
            cache.results.insert(key, *h);
        }
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
        if !set.is_compound() {
            return Err(eval_err("membership in a value that is not a set"));
        }
        let kind = self.heap.view(set).kind;
        if self.fuel != u64::MAX && kind == Kind::SetEnum {
            self.domain_work(set, false, 0)?;
        }
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
                    for (h, flags) in [
                        (x, x.is_compound().then(|| self.heap.view(x).flags)),
                        (set, Some(v.flags)),
                    ] {
                        // A node carrying the cached proof is materialized
                        // (`compare::materialized` accepts it at once), and so
                        // is a recorded proven handle; only the others walk.
                        let proved = flags.is_some_and(|flags| {
                            flags & crate::node::flags::CONCRETE != 0 || self.proven(h)
                        });
                        if !proved {
                            compare::require_materialized(&self.heap, &[h])?;
                        }
                    }
                }
                let found = match indexed {
                    Some(found) => found,
                    // Both sides were proven materialized above, where equal
                    // values have equal hashes (the member index's
                    // admission). A small set is scanned by hash, and each
                    // hash match is confirmed by the canonical comparison.
                    None if x.is_compound()
                        && v.data.len() < SMALL_MEMBER_SCAN
                        && !self.heap.store.semantics().tlc_comparisons =>
                    {
                        let hash = self.heap.hash(x);
                        v.data
                            .iter()
                            .any(|&e| self.heap.hash(e) == hash && self.heap.cmp(e, x).is_eq())
                    }
                    None => v.data.binary_search_by(|&e| self.heap.cmp(e, x)).is_ok(),
                };
                if found {
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
                let keys = xv.keys().to_vec();
                if keys.len() != names.len()
                    || (!self.heap.store().semantics().tlc_comparisons && keys != names)
                {
                    return Ok(false);
                }
                let vals = xv.values().to_vec();
                for ((k, name), (e, s)) in
                    keys.into_iter().zip(names).zip(vals.into_iter().zip(sets))
                {
                    if k != name || !self.member(e, s)? {
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
            Kind::Union if v.lo == crate::convert::BOUND_RECORD_UNION => {
                match self.record_union_member(x, set) {
                    Err(e)
                        if e.kind() == crate::ErrorKind::Eval
                            && !self.heap.store().semantics().tlc_comparisons =>
                    {
                        // A field comparison may be undefined before another arm's
                        // label distinguishes it. Use the original concrete-set
                        // membership in that case, including its comparison policy.
                        let concrete = self.work_materialize_cached(set)?;
                        self.member(x, concrete)
                    }
                    result => result,
                }
            }
            Kind::Union => {
                let (a, b) = (v.data[0], v.data[1]);
                Ok(self.member(x, a)? || self.member(x, b)?)
            }
            Kind::SetPred => Err(Error::unsupported(
                "membership in a lazy set predicate (the evaluator side is not built)",
            )),
            // TLC's unnormalized `SetEnumValue.member` is a linear scan whose
            // errors differ from a sorted search; its compat-order
            // comparison is not built.
            Kind::SetEnum => Err(Error::unsupported(
                "membership in a set enumeration TLC has not normalized (--compat=tlc)",
            )),
        }
    }

    fn record_union_member(&mut self, x: Handle, domain: Handle) -> Result<bool, Error> {
        if self.heap.kind(domain) == Some(Kind::Union) {
            let v = self.heap.view(domain);
            if v.lo == crate::convert::BOUND_RECORD_UNION {
                let [a, b] = *v.data else {
                    return Err(Error::internal("invalid closed record union"));
                };
                return Ok(self.record_union_member(x, a)? || self.record_union_member(x, b)?);
            }
        }
        self.member(x, domain)
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
        // materializing it.
        let dom = self.eval(code, bind.domain)?;
        let mut cur = self.work_cursor(dom)?;
        let mut out = Vec::new();
        loop {
            // Free a rejected candidate, and the predicate's transients, by
            // arena rollback. Only when no thunk was forced to a
            // transient meanwhile: its slot may hold a value built after the
            // mark.
            let before = self.heap.mark();
            let Some(e) = self.work_next(&mut cur)? else {
                break;
            };
            self.charge(1)?;
            let after = self.heap.mark();
            let forced = self.thunk_mark();
            self.bind_one(bind, e)?;
            let keep = self.eval_bool(code, pred)?;
            if self.thunks_released(forced) {
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
        let forced = self.thunk_mark();
        match self.eval_norm(code, set) {
            Ok(f) => {
                let powerset = self.heap.symbolic(Kind::Subset, 0, &[f]);
                self.work_cursor(powerset).map(Some)
            }
            Err(e) if e.kind() == crate::ErrorKind::Eval => {
                if self.thunks_released(forced) {
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
            let d = match code.nodes.get(b.domain as usize) {
                // A plan constant's enumeration is built once (bosco's
                // `\E r \in SUBSET (P \X M)`).
                Some(Node::Const(h)) if self.fuel == u64::MAX => {
                    self.heap.enumerate_constant(*h)?
                }
                _ => {
                    let d = self.eval(code, b.domain)?;
                    self.work_norm(d)?
                }
            };
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
        self.each_unit_binding(binds, &units, f)
    }

    /// Runs `f` under each combination of `units`' elements, the first unit
    /// varying fastest; `f` returns `false` to stop.
    pub(crate) fn each_unit_binding(
        &mut self,
        binds: &[Bind],
        units: &[(Unit, Vec<Handle>)],
        f: &mut dyn FnMut(&mut Self) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
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

    pub(crate) fn existential_bindings(
        &mut self,
        code: &Code,
        binds: &[Bind],
        visit: &mut dyn FnMut(&mut Self) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
        if self.fuel == u64::MAX
            && let [bind] = binds
            && let Some(set) = bind.eligible
        {
            // The first guard is pure. If preparing its restricted domain
            // cannot be evaluated, let ordinary enumeration determine whether
            // and when the original action actually observes that failure.
            let cursor = match self.subset_cursor(code, Some(set)) {
                Err(e) if e.kind() == crate::ErrorKind::Unsupported => None,
                result => result?,
            };
            if let Some(mut cursor) = cursor {
                loop {
                    let before = self.heap.mark();
                    let Some(element) = self.work_next(&mut cursor)? else {
                        break;
                    };
                    let forced = self.thunk_mark();
                    self.bind_one(bind, element)?;
                    if !visit(self)? {
                        return Ok(false);
                    }
                    if self.thunks_released(forced) {
                        self.heap.rollback(before);
                    }
                }
                return Ok(true);
            }
        }
        self.for_each_binding(code, binds, visit)
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
                self.existential_bindings(code, binds, &mut |m: &mut Self| {
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
                let mut cur = self.choice_cursor(d)?;
                loop {
                    // Free a rejected candidate, and the predicate's
                    // transients, by arena rollback, as `filter` does.
                    let before = self.heap.mark();
                    let Some(e) = self.next_choice(&mut cur)? else {
                        break;
                    };
                    self.charge(1)?;
                    let forced = self.thunk_mark();
                    self.bind_one(&binds[0], e)?;
                    if self.eval_bool(code, body)? {
                        return Ok(e);
                    }
                    if self.thunks_released(forced) {
                        self.heap.rollback(before);
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

    fn prefix_fcn(
        &mut self,
        code: &Code,
        bind: &Bind,
        function: u32,
        body: P,
    ) -> Result<Handle, Error> {
        let binds = std::slice::from_ref(bind);
        let mut units = self.units(code, binds)?;
        let (unit, keys) = units
            .pop()
            .ok_or_else(|| Error::internal("prefix function without a unit"))?;
        if keys.iter().any(|key| key.as_int().is_none())
            || keys
                .windows(2)
                .any(|pair| pair[0].as_int() >= pair[1].as_int())
        {
            return Err(Error::unsupported(
                "finite function equation requires an ordered small-integer domain",
            ));
        }
        self.charge(keys.len())?;
        let mut values = Vec::with_capacity(keys.len());
        for (index, &key) in keys.iter().enumerate() {
            self.bind_unit(binds, unit, key)?;
            let start = index.saturating_sub(1);
            let prefix = self
                .heap
                .fcn_sorted(&keys[start..index], &values[start..index]);
            self.set_slot(function, prefix);
            values.push(self.eval_component(code, body)?);
        }
        Ok(self.heap.fcn_sorted(&keys, &values))
    }

    // ------------------------------------------------------------------
    // Functions and records
    // ------------------------------------------------------------------

    /// A direct table and already-ground scalar index are effect-free.
    /// Values behind unforced thunks and misses still run the original code.
    fn direct_cell(&self, code: &Code, table: P, key: P) -> Option<Handle> {
        let raw = |p: P| match *code.nodes.get(p as usize)? {
            Node::Var(i) => self.s0.get(usize::from(i)).copied(),
            Node::Const(h) => Some(h),
            Node::Slot(s) => Some(self.slot(s)),
            _ => None,
        };
        let table = raw(table)?;
        let key = raw(key)?;
        if !table.is_compound() || key.is_compound() || key.is_empty() {
            return None;
        }
        let view = self.heap.view(table);
        if !view.kind.is_fcn() {
            return None;
        }
        let index = self.fcn_index(view, key).ok()??;
        Some(view.values()[index])
    }

    /// Look up a key in an already borrowed compound.
    fn fcn_index(&self, v: View<'_>, key: Handle) -> Result<Option<usize>, Error> {
        match v.kind {
            Kind::IntFcn => Ok(key.as_int().and_then(|k| {
                let i = k.checked_sub(v.lo)?;
                usize::try_from(i).ok().filter(|&i| i < v.data.len())
            })),
            Kind::Fcn => {
                let keys = v.keys();
                if self.opts.actor_rows
                    && let Some(id) = key.as_str_id().or_else(|| key.as_model_id())
                {
                    Ok(self.heap.name_position(keys, key, id))
                } else if key.as_str_id().is_some() || keys.len() <= 8 {
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
        if !f.is_compound() {
            return Err(eval_err("application of a value that is not a function"));
        }
        let v = self.heap.view(f);
        if let Some(i) = self.fcn_index(v, a)? {
            return Ok(v.values()[i]);
        }
        let msg = "Attempted to apply function to an argument not in its domain";
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
        if v.kind == Kind::Fcn {
            let index = if self.opts.actor_rows
                && let Some(id) = field.as_str_id()
            {
                self.heap.name_position(v.keys(), field, id)
            } else {
                v.keys().iter().position(|&k| k == field)
            };
            if let Some(i) = index {
                return Ok(v.values()[i]);
            }
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

    /// Only complete persistent operands enter a bounded row site. The outer
    /// table must be a canonical identity so reusing an unchanged row cannot
    /// retain a literal's different representation bits. Every slot, including
    /// external actor and RHS inputs, participates in exact handle comparison.
    fn row_memo_key(
        &self,
        code: &Code,
        site: Option<u32>,
        table: Handle,
        row: Handle,
    ) -> Option<(u32, [Handle; MAX_KEY], usize)> {
        let site = site?;
        if self.heap.speculative
            || self.refuse_unrecorded_warnings
            || !self.memo.active(site)
            || !self.memo.may_keep()
            || !table.is_persistent()
            || table.is_lazy_persistent()
            || row.is_transient()
        {
            return None;
        }
        let meta = code.sites.get(site as usize)?;
        let len = meta.key_len();
        if !meta.row_input || len > MAX_KEY || !meta.primed.is_empty() {
            return None;
        }
        let mut key = [Handle::EMPTY; MAX_KEY];
        key[0] = row;
        for (i, &var) in meta.vars.iter().enumerate() {
            key[1 + i] = *self.s0.get(usize::from(var))?;
        }
        for (i, &slot) in meta.slots.iter().enumerate() {
            key[1 + meta.vars.len() + i] = self.slot(slot);
        }
        key[..len]
            .iter()
            .all(|h| !h.is_transient() && !h.is_empty())
            .then_some((site, key, len))
    }

    /// Resolve every selector against the actual carrier before evaluating
    /// any RHS. Missing keys, unsupported operand shapes and unbound slots
    /// then take the original path with its exact demands and diagnostics.
    fn except_row(
        &mut self,
        code: &Code,
        table: Handle,
        updates: &[Update],
        plan: &crate::plan::RowUpdate,
    ) -> Result<Option<Handle>, Error> {
        let Some(first) = updates
            .first()
            .and_then(|u| u.path.first())
            .and_then(|&p| code.nodes.get(p as usize))
        else {
            return Ok(None);
        };
        if updates.len() != plan.fields.len() || updates.len() > crate::plan::MAX_ROW_FIELDS {
            return Ok(None);
        }
        let key = match *first {
            Node::Const(h) => h,
            Node::Slot(s) => self.slot(s),
            _ => return Ok(None),
        };
        if key.is_empty() || key.is_compound() || !table.is_compound() {
            return Ok(None);
        }
        let outer = self.heap.view(table);
        if !outer.kind.is_fcn() {
            return Ok(None);
        }
        let Some(index) = self.fcn_index(outer, key)? else {
            return Ok(None);
        };
        let row = outer.values()[index];
        if !row.is_compound() {
            return Ok(None);
        }
        let record = self.heap.view(row);
        if record.kind != Kind::Fcn {
            return Ok(None);
        }
        let mut slots = [(0, Handle::EMPTY); crate::plan::MAX_ROW_FIELDS];
        for (slot, &field) in slots.iter_mut().zip(&plan.fields) {
            let Some(pos) = field
                .as_str_id()
                .and_then(|id| self.heap.name_position(record.keys(), field, id))
            else {
                return Ok(None);
            };
            *slot = (pos, record.values()[pos]);
        }
        let lookups = outer.len().saturating_add(record.len());
        let copies = outer.data.len().saturating_add(record.data.len());
        let memo = if outer.is_lazy() || record.is_lazy() {
            None
        } else {
            self.row_memo_key(code, plan.site, table, row)
        };
        if let Some((site, key, len)) = &memo
            && let Some(result) = self.memo.get(*site, &key[..*len])
        {
            self.charge(lookups)?;
            return Ok(Some(if result == row {
                table
            } else {
                self.heap.fcn_with(table, index, result)
            }));
        }

        let mut changed = 0;
        for (i, u) in updates.iter().enumerate() {
            self.charge(lookups)?;
            let (position, old) = slots[i];
            let new = if u.path.len() == 2 {
                self.set_slot(u.at, old);
                self.eval_component(code, u.value)?
            } else {
                self.except(code, old, u, 2)?
            };
            if new != old {
                self.charge(copies)?;
                slots[changed] = (position, new);
                changed += 1;
            }
        }
        let result = self
            .heap
            .replace_row_fields(table, index, row, &slots[..changed]);
        if let Some((site, key, len)) = memo
            && slots[..changed].iter().all(|&(_, v)| !self.heap.is_lazy(v))
        {
            let output = self.heap.view(result).values()[index];
            let added = self.heap.cache.added;
            if let Ok(value) = self.heap.intern_exact(output)
                && (value != row || result == table)
            {
                self.memo.put(site, &key[..len], value);
            }
            self.charge_kept(site, added);
        }
        Ok(Some(result))
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
        if !f.is_compound() {
            return Err(eval_err("EXCEPT on a value that is not a function"));
        }
        let view = self.heap.view(f);
        if !view.kind.is_fcn() {
            return Err(eval_err("EXCEPT on a value that is not a function"));
        }
        let data_len = view.data.len();
        let entries = view.len();
        // Copy the selected handle before a work checkpoint or recursive
        // evaluation can mutate the machine. A dense lookup has constant cost.
        let dense = if view.kind == Kind::IntFcn {
            Some(self.fcn_index(view, key)?.map(|i| (i, view.data[i])))
        } else {
            None
        };
        self.charge(entries)?;
        let entry = if let Some(entry) = dense {
            entry
        } else {
            let view = self.heap.view(f);
            self.fcn_index(view, key)?.map(|i| (i, view.values()[i]))
        };
        let Some((i, old)) = entry else {
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
            self.heap.store().note_missing_update();
            if self.refuse_unrecorded_warnings {
                return Err(Error::unsupported(
                    "an EXCEPT key outside the function's domain, where the reference prints warning 2144",
                ));
            }
            return Ok(f);
        };
        let new = if depth + 1 == u.path.len() {
            self.set_slot(u.at, old);
            self.eval_component(code, u.value)?
        } else {
            self.except(code, old, u, depth + 1)?
        };
        if new == old {
            return Ok(f);
        }
        self.charge(data_len)?;
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

    /// Bags (`tlc2.module.Bags`; as in `dive-eval`'s `community` module).
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

    /// A primed definition's site (`MemoSite::candidate`) sees a new
    /// primed key value for every enumeration candidate. Its persistent
    /// copies are bounded by the memo cap (#869 review), never one per
    /// candidate, and its results are those of the body throughout.
    #[test]
    fn candidate_site_copies_stay_within_the_memo_cap() {
        use crate::plan::{Code, MemoSite};
        let cap = 64 << 10;
        let store = Arc::new(Store::new(true));
        let opts = Opts {
            memo_cap: cap,
            ..Opts::default()
        };
        let mut m = Machine::new(Arc::clone(&store), 1, opts, Memo::new(&[1]));
        let code = Code {
            nodes: vec![
                Node::Primed(0),
                Node::Tuple(vec![0, 0].into()),
                Node::Memo { site: 0, body: 1 },
            ],
            sites: vec![MemoSite {
                row_input: false,
                name: "F (Primed, action)".into(),
                vars: vec![].into(),
                slots: vec![].into(),
                primed: vec![0].into(),
                step: true,
                candidate: true,
            }],
            ..Code::default()
        };
        let before = store.stats().nodes;
        let candidates: i64 = 20_000;
        for i in 0..candidates {
            let items: Vec<Handle> = (0..6).map(|k| Handle::int(i * 8 + k).unwrap()).collect();
            let x = m.heap.tuple(&items);
            m.s1[0] = x;
            let expected = m.heap.tuple(&[x, x]);
            for _ in 0..4 {
                let r = m.eval(&code, 2).unwrap();
                assert!(
                    compare::equals(&m.heap, r, expected).unwrap(),
                    "candidate {i}"
                );
            }
            m.s1[0] = Handle::EMPTY;
            m.heap.reset();
        }
        let added = store.stats().nodes - before;
        // Each candidate's key and result are two nodes of 6 and 2 handles:
        // copying every one would add `2 * candidates` nodes.
        assert!(
            added < 2 * candidates.unsigned_abs() / 4,
            "{added} persistent nodes"
        );
        assert!(!m.memo.may_keep());
        assert!(m.memo.stats()[0].off);
        assert!(m.memo.stats()[0].hits > 0);
    }

    #[test]
    fn a_transient_bound_stops_an_evaluation_by_name() {
        let mut m = Machine::new(
            Arc::new(Store::with_semantics(true, crate::Semantics::default())),
            0,
            Opts::default(),
            Memo::new(&[]),
        );
        let one = Handle::int(1).unwrap();
        assert_eq!(m.charge(1), Ok(()));
        m.set_transient_bound(4096);
        while m.heap.arena.footprint() <= 4096 {
            m.heap.tuple(&[one, one, one, one]);
        }
        let error = m.charge(1).unwrap_err();
        assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
        assert!(error.message().contains("start-up evaluation memory bound"));
        m.set_transient_bound(usize::MAX);
        assert_eq!(m.charge(1), Ok(()));
        m.heap.reset();
        m.heap.arena.release();
        assert_eq!(m.heap.arena.footprint(), 0);
    }

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

#[cfg(test)]
mod thunk_rollback {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    /// Slot 0's thunk is a tuple of `x`: a transient.
    fn code() -> Code {
        Code {
            nodes: vec![
                Node::Var(0),
                Node::Tuple(Box::new([0, 0])),
                Node::Lazy { slot: 0, body: 1 },
            ],
            ..Default::default()
        }
    }

    fn machine() -> Machine {
        let mut m = Machine::new(
            Arc::new(Store::default()),
            1,
            Opts::default(),
            Memo::default(),
        );
        m.s0[0] = Handle::int(1).unwrap();
        m
    }

    #[test]
    fn a_thunk_forced_in_a_left_frame_releases_the_rollback() {
        let code = code();
        let mut m = machine();
        let outer = m.enter(1);
        let before = m.heap.mark();
        let since = m.thunk_mark();
        let inner = m.enter(1);
        let v = m.eval(&code, 2).unwrap();
        assert!(v.is_transient());
        // While the check's frame is live, its slot holds the value.
        assert!(!m.thunks_released(since));
        m.leave(inner);
        assert!(m.thunks_released(since));
        m.heap.rollback(before);
        assert!(!m.heap.poisoned());
        // The outer frame's own thunk is unforced and evaluates afresh.
        let w = m.eval(&code, 2).unwrap();
        assert!(compare::equals(&m.heap, w, w).unwrap());
        assert!(!m.heap.poisoned());
        m.leave(outer);
    }

    #[test]
    fn a_thunk_forced_in_a_live_frame_keeps_the_values_it_holds() {
        let code = code();
        let mut m = machine();
        let outer = m.enter(1);
        let since = m.thunk_mark();
        let v = m.eval(&code, 2).unwrap();
        let inner = m.enter(1);
        m.leave(inner);
        // The live slot holds a value built after the mark: no rollback.
        assert!(!m.thunks_released(since));
        // An enclosing scope still sees it, and a later scope does not.
        assert!(!m.thunks_released(0));
        let later = m.thunk_mark();
        assert!(m.thunks_released(later));
        assert_eq!(m.eval(&code, 2).unwrap(), v);
        assert!(!m.heap.poisoned());
        m.leave(outer);
        // Leaving the last frame drops every slot.
        assert!(m.thunks_released(0));
    }

    #[test]
    fn a_released_inner_scope_keeps_the_enclosing_scopes_live_slots() {
        let code = code();
        let mut m = machine();
        let outer = m.enter(1);
        let since = m.thunk_mark();
        m.eval(&code, 2).unwrap();
        let inner_since = m.thunk_mark();
        let inner = m.enter(1);
        m.eval(&code, 2).unwrap();
        m.leave(inner);
        assert!(m.thunks_released(inner_since));
        assert!(!m.thunks_released(since));
        m.leave(outer);
    }
}

#[cfg(test)]
mod work_poll_bound {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Proceeds for the first `allow` polls, then refuses.
    #[derive(Debug)]
    struct Allow {
        allow: usize,
        polls: AtomicUsize,
    }
    impl WorkControl for Allow {
        fn proceed(&self) -> bool {
            self.polls.fetch_add(1, Ordering::Relaxed) < self.allow
        }
    }

    fn machine(allow: usize) -> (Machine, Arc<Allow>) {
        let mut m = Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            Memo::default(),
        );
        let control = Arc::new(Allow {
            allow,
            polls: AtomicUsize::new(0),
        });
        m.set_work_control(Some(control.clone()));
        (m, control)
    }

    fn polls(control: &Allow) -> usize {
        control.polls.load(Ordering::Relaxed)
    }

    #[test]
    fn the_first_charge_after_installation_polls() {
        let (mut m, control) = machine(0);
        let error = m.charge(1).err();
        assert!(error.is_some_and(|e| e.kind() == crate::ErrorKind::Stop));
        assert_eq!(polls(&control), 1);
        assert!(m.heap.work_interrupted());
    }

    #[test]
    fn a_stop_is_seen_within_the_documented_unit_bound() {
        let units = usize::try_from(WORK_POLL_UNITS).unwrap_or(usize::MAX);
        for n in [0, 1, 3] {
            let (mut m, control) = machine(1);
            let mut charged = 0usize;
            let mut calls = 0usize;
            let stopped = loop {
                let result = m.charge(n);
                calls += 1;
                if let Err(error) = result {
                    break error;
                }
                charged += n.max(1);
                assert!(calls <= units + 1, "no stop within the bound (n={n})");
            };
            assert_eq!(stopped.kind(), crate::ErrorKind::Stop);
            assert_eq!(polls(&control), 2);
            // The first charge polled; the refused poll came on the charge
            // that would carry the units since it to the bound.
            assert!(charged < units + n.max(1), "n={n} charged={charged}");
            assert!(calls <= units + 1, "n={n} calls={calls}");
            // Once refused, every later charge polls again.
            for _ in 0..3 {
                assert!(m.charge(1).is_err());
            }
            assert_eq!(polls(&control), 5);
        }
    }

    #[test]
    fn a_large_charge_polls_before_its_work() {
        let (mut m, control) = machine(1);
        assert_eq!(m.charge(1), Ok(()));
        let big = usize::try_from(WORK_POLL_UNITS).unwrap_or(usize::MAX);
        let error = m.charge(big).err();
        assert!(error.is_some_and(|e| e.kind() == crate::ErrorKind::Stop));
        assert_eq!(polls(&control), 2);
    }

    #[test]
    fn bounded_fuel_and_transients_are_checked_on_every_charge_and_polled_on_the_bound() {
        let units = usize::try_from(WORK_POLL_UNITS).unwrap_or(usize::MAX);
        let (mut m, control) = machine(usize::MAX);
        assert_eq!(m.charge(1), Ok(()));
        m.set_fuel(10);
        for spent in 1..=10 {
            assert_eq!(m.charge(1), Ok(()));
            assert_eq!(m.fuel, 10 - spent);
        }
        let error = m.charge(1).err();
        assert!(error.is_some_and(|e| e.kind() == crate::ErrorKind::Unsupported));
        assert_eq!(polls(&control), 1);
        m.set_fuel(u64::MAX);
        m.heap.tuple(&[Handle::TRUE]);
        m.set_transient_bound(0);
        let error = m.charge(1).err();
        assert!(error.is_some_and(|e| e.kind() == crate::ErrorKind::Unsupported));
        m.set_transient_bound(usize::MAX);
        // Twelve charges since the one that polled: the bound counts the
        // bounded and unbounded charges alike, so charge 33 polls.
        for _ in 13..units {
            assert_eq!(m.charge(1), Ok(()));
        }
        assert_eq!(polls(&control), 1);
        assert_eq!(m.charge(1), Ok(()));
        assert_eq!(polls(&control), 2);
        // A bounded machine still polls within the bound.
        m.set_fuel(u64::MAX - 1);
        for _ in 1..units {
            assert_eq!(m.charge(1), Ok(()));
        }
        assert_eq!(polls(&control), 2);
        assert_eq!(m.charge(1), Ok(()));
        assert_eq!(polls(&control), 3);
    }

    #[test]
    fn bounded_inline_charges_spend_fuel_and_poll_exactly_as_the_slow_path() {
        let units = WORK_POLL_UNITS;
        let (mut m, control) = machine(usize::MAX);
        m.set_fuel(100);
        // The first charge polls; each later poll comes after `units` poll
        // units, where a zero charge counts one unit and spends no fuel.
        let mut fuel = 100u64;
        let mut since_poll = 0u64;
        let mut expected_polls = 0usize;
        for n in [0usize, 1, 5, 0, 3, 7, 2, 0, 9, 4, 1, 6, 0, 8, 2, 5, 3] {
            let poll_units = (n as u64).max(1);
            if expected_polls == 0 || since_poll + poll_units >= units {
                expected_polls += 1;
                since_poll = 0;
            } else {
                since_poll += poll_units;
            }
            assert_eq!(m.charge(n), Ok(()));
            fuel -= n as u64;
            assert_eq!(m.fuel, fuel);
            assert_eq!(polls(&control), expected_polls);
        }
        // Fuel that does not cover a charge refuses it and is spent.
        let left = usize::try_from(fuel).unwrap_or(usize::MAX);
        let error = m.charge(left + 1).err();
        assert!(error.is_some_and(|e| e.kind() == crate::ErrorKind::Unsupported));
        assert!(m.fuel_exhausted());
    }

    #[test]
    fn reinstalling_a_control_polls_at_once() {
        let (mut m, control) = machine(usize::MAX);
        assert_eq!(m.charge(1), Ok(()));
        assert_eq!(polls(&control), 1);
        let deny = Arc::new(Allow {
            allow: 0,
            polls: AtomicUsize::new(0),
        });
        m.set_work_control(Some(deny.clone()));
        assert!(m.charge(1).is_err());
        assert_eq!(polls(&deny), 1);
    }
}

#[cfg(test)]
mod bounded_analysis_machine {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;
    use crate::plan::MemoSite;

    fn memo_code(step: bool) -> Code {
        Code {
            nodes: vec![
                Node::Const(Handle::int(7).unwrap()),
                Node::Memo { site: 0, body: 0 },
            ],
            sites: vec![MemoSite {
                row_input: false,
                name: "site".into(),
                vars: Box::new([0]),
                slots: Box::new([]),
                primed: Box::new([]),
                step,
                candidate: false,
            }],
            ..Default::default()
        }
    }

    fn machine(memo_on: bool, memo: Memo) -> Machine {
        let opts = Opts {
            memo: memo_on,
            step_memo: memo_on,
            ..Opts::default()
        };
        let mut m = Machine::new(Arc::new(Store::default()), 1, opts, memo);
        m.s0[0] = Handle::int(1).unwrap();
        m
    }

    #[test]
    fn memo_off_machines_evaluate_memo_bodies_without_reading_or_filling_a_site() {
        // Control: with memoization on, the site is looked up and filled.
        let mut on = machine(true, Memo::new(&[1]));
        assert_eq!(
            on.eval(&memo_code(false), 1).unwrap(),
            Handle::int(7).unwrap()
        );
        assert_eq!(on.memo.stats()[0].lookups, 1);
        assert!(on.memo.bytes() > 0);
        // memo = step_memo = false: even a site with a table is never used.
        let mut off = machine(false, Memo::new(&[1]));
        for _ in 0..2 {
            assert_eq!(
                off.eval(&memo_code(false), 1).unwrap(),
                Handle::int(7).unwrap()
            );
        }
        assert_eq!(off.memo.stats()[0].lookups, 0);
        assert_eq!(off.memo.bytes(), 0);
        // An analysis machine owns a table-less Memo, so not even a step
        // site (which ignores Opts::memo) can read or fill one.
        let mut analysis = machine(false, Memo::default());
        for step in [false, true] {
            assert_eq!(
                analysis.eval(&memo_code(step), 1).unwrap(),
                Handle::int(7).unwrap()
            );
        }
        assert_eq!(analysis.memo.stats(), Vec::new());
        assert_eq!(analysis.memo.bytes(), 0);
    }

    #[test]
    fn bounded_machines_precharge_big_powers_before_allocating() {
        let pow = |x: i64, y: i64| Code {
            nodes: vec![
                Node::Const(Handle::int(x).unwrap()),
                Node::Const(Handle::int(y).unwrap()),
                Node::Arith(Arith::Pow, 0, 1),
            ],
            ..Default::default()
        };
        let mut m = machine(false, Memo::default());
        // 3^100000 has about 158,497 bits (2,476 words): more than the fuel.
        m.set_fuel(1000);
        let nodes = m.heap.arena().peak().0;
        let error = m.eval(&pow(3, 100_000), 2).unwrap_err();
        assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
        assert!(m.fuel_exhausted());
        assert_eq!(m.heap.arena().peak().0, nodes, "no digits allocated");
        // Within the fuel the exact big result is built.
        m.set_fuel(1 << 20);
        let big = m.eval(&pow(3, 100_000), 2).unwrap();
        assert_eq!(
            m.heap.big(big).unwrap(),
            num_traits::Pow::pow(num_bigint::BigInt::from(3), 100_000_u32)
        );
        assert!(!m.fuel_exhausted());
        // Small and TLC-mode powers cost no digit fuel.
        m.set_fuel(4);
        assert_eq!(m.eval(&pow(2, 10), 2).unwrap(), Handle::int(1024).unwrap());
        m.heap.set_int_mode(crate::int::IntMode::Tlc);
        m.set_fuel(4);
        assert_eq!(m.eval(&pow(2, 10), 2).unwrap(), Handle::int(1024).unwrap());
    }
}

#[cfg(test)]
mod same_vars {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    /// `<<x, <<y, z>>>> = <<x', <<y', z'>>>>` behind a thunk (node 8), and
    /// its `SameVars` (node 15).
    fn code() -> Code {
        Code {
            nodes: vec![
                Node::Var(0),
                Node::Var(1),
                Node::Var(2),
                Node::Tuple(Box::new([1, 2])),
                Node::Tuple(Box::new([0, 3])),
                Node::Lazy { slot: 0, body: 4 },
                Node::Primed(0),
                Node::Primed(1),
                Node::Eq(5, 13),
                Node::Primed(2),
                Node::Tuple(Box::new([7, 9])),
                Node::Tuple(Box::new([6, 10])),
                Node::Lazy { slot: 1, body: 11 },
                Node::Lazy { slot: 2, body: 12 },
                Node::Const(Handle::TRUE),
                Node::SameVars {
                    eq: 8,
                    leaves: Box::new([(0, 1), (1, 2), (2, 2)]),
                    thunks: Box::new([0, 1, 2]),
                },
            ],
            ..Default::default()
        }
    }

    fn values(m: &mut Machine) -> Vec<Handle> {
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let text = m.heap.string("a").unwrap();
        let set = m.heap.set_enum(&[one, two]);
        let set = m.heap.intern(set).unwrap();
        let other = m.heap.set_enum(&[one]);
        let other = m.heap.intern(other).unwrap();
        let fcn = m.heap.int_fcn(1, &[set, text]);
        let fcn = m.heap.intern_exact(fcn).unwrap();
        // A symbolic interval is not materialized: `Eq` normalizes it.
        let interval = m.heap.interval(1, 2);
        let interval = m.heap.intern_exact(interval).unwrap();
        vec![one, two, text, set, other, fcn, interval, Handle::EMPTY]
    }

    /// The `SameVars` result equals the `Eq` it wraps on every pair of
    /// rows drawn from `values`, error for error.
    fn agrees(semantics: crate::store::Semantics) {
        let code = code();
        let store = Arc::new(Store::with_semantics(true, semantics));
        let mut m = Machine::new(store, 3, Opts::default(), Memo::default());
        let values = values(&mut m);
        let pick = |i: usize| values[i % values.len()];
        let mut checked = 0;
        for i in 0..values.len().pow(2) {
            let (a, b) = (i % values.len(), i / values.len());
            for row in [[a, a, b], [a, b, b], [b, a, a], [a, a, a]] {
                let before: Vec<_> = row.iter().map(|&k| pick(k)).collect();
                for after in [before.clone(), vec![pick(b), before[1], before[2]]] {
                    m.s0.copy_from_slice(&before);
                    m.s1.copy_from_slice(&after);
                    let frame = m.enter(3);
                    let eq = m.eval(&code, 8).map_err(|e| e.kind());
                    m.leave(frame);
                    let frame = m.enter(3);
                    let same = m.eval(&code, 15).map_err(|e| e.kind());
                    m.leave(frame);
                    assert_eq!(same, eq, "{before:?} {after:?}");
                    // With the handles a row proof admits recorded as
                    // proven, the result is the same.
                    let proven: Vec<_> = before
                        .iter()
                        .chain(&after)
                        .copied()
                        .filter(|&h| m.heap.materialized_within(&[h], 4096, None))
                        .collect();
                    let keep = |row: &[Handle]| -> Vec<Handle> {
                        row.iter()
                            .map(|&h| {
                                if proven.contains(&h) {
                                    h
                                } else {
                                    Handle::EMPTY
                                }
                            })
                            .collect()
                    };
                    m.set_proven_materialized(&keep(&before), &keep(&after), []);
                    let frame = m.enter(3);
                    let reused = m.eval(&code, 15).map_err(|e| e.kind());
                    m.leave(frame);
                    m.set_proven_materialized(&[], &[], []);
                    assert_eq!(reused, eq, "{before:?} {after:?} with proofs");
                    checked += 1;
                }
            }
        }
        assert!(checked > 100);
        assert!(!m.heap.poisoned());
    }

    /// Set membership with recorded proofs agrees with membership without
    /// them, and an operand nobody recorded still refuses when it is not
    /// materialized.
    #[test]
    fn membership_reuses_only_recorded_proofs() {
        let store = Arc::new(Store::default());
        let mut m = Machine::new(store, 2, Opts::default(), Memo::default());
        // Bounded fuel: membership takes the checked path, not the index.
        m.set_fuel(1 << 30);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        // Persistent copies without the cached proof, as snapshot restore
        // and owned exchange produce them.
        let inner = m.heap.set_from(&mut vec![one, two]);
        let inner = m.heap.unmarked_copy(inner);
        let single = m.heap.set_from(&mut vec![one]);
        let single = m.heap.unmarked_copy(single);
        let outer = m.heap.set_from(&mut vec![inner, single]);
        let outer = m.heap.unmarked_copy(outer);
        let other = m.heap.set_from(&mut vec![two]);
        let other = m.heap.unmarked_copy(other);
        let interval = m.heap.interval(1, 2);
        let interval = m.heap.intern_exact(interval).unwrap();
        let lazy = m.heap.set_from(&mut vec![interval]);
        let lazy = m.heap.unmarked_copy(lazy);
        for h in [inner, single, outer, other] {
            assert!(!m.heap.proved_concrete(h));
            assert!(m.heap.materialized_within(&[h], 64, None));
        }
        assert!(!m.heap.materialized_within(&[lazy], 64, None));
        let cases = [(inner, outer), (other, outer), (one, inner), (two, single)];
        for (x, set) in cases {
            m.set_proven_materialized(&[], &[], []);
            let plain = m.member(x, set).map_err(|e| e.kind());
            assert!(plain.is_ok(), "{x:?} {set:?} {plain:?}");
            m.set_proven_materialized(&[x], &[set], []);
            assert_eq!(m.member(x, set).map_err(|e| e.kind()), plain);
            m.set_proven_materialized(&[], &[], [x, set]);
            assert_eq!(m.member(x, set).map_err(|e| e.kind()), plain);
        }
        assert_eq!(m.member(inner, outer).ok(), Some(true));
        assert_eq!(m.member(other, outer).ok(), Some(false));
        assert_eq!(m.member(two, single).ok(), Some(false));
        // An unmaterialized operand that is not recorded still refuses,
        // with or without proofs recorded for the other operand.
        m.set_proven_materialized(&[], &[], []);
        let refused = m.member(lazy, outer).map_err(|e| e.kind());
        assert!(refused.is_err());
        m.set_proven_materialized(&[outer], &[], []);
        assert_eq!(m.member(lazy, outer).map_err(|e| e.kind()), refused);
        m.set_proven_materialized(&[], &[], []);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn same_vars_agrees_with_the_tuple_equality_it_wraps() {
        agrees(crate::store::Semantics::default());
        agrees(crate::store::Semantics::tlc());
    }

    #[test]
    fn a_forced_thunk_is_read_as_the_tuple_equality_reads_it() {
        let code = code();
        let mut m = Machine::new(
            Arc::new(Store::default()),
            3,
            Opts::default(),
            Memo::default(),
        );
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        m.s0.copy_from_slice(&[one, one, one]);
        m.s1.copy_from_slice(&[one, one, one]);
        let frame = m.enter(3);
        // A cached value the variables no longer give is what `Eq` reads.
        let stale = m.heap.tuple(&[two, two]);
        m.set_slot(0, stale);
        assert_eq!(m.eval(&code, 15).unwrap(), Handle::FALSE);
        for slot in 0..3 {
            m.set_slot(slot, Handle::EMPTY);
        }
        assert_eq!(m.eval(&code, 15).unwrap(), Handle::TRUE);
        // The direct comparison forces no thunk.
        assert!((0..3).all(|slot| m.slot(slot).is_empty()));
        m.s1[2] = two;
        assert_eq!(m.eval(&code, 15).unwrap(), Handle::FALSE);
        assert!((0..3).all(|slot| m.slot(slot).is_empty()));
        m.leave(frame);
    }
}
