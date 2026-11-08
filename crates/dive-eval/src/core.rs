//! The evaluator's shared state: the IR, the model's bindings, the options,
//! the plan (literal values and interned names prepared once per
//! specification), and the caches TLC keeps per run.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use dive_ir::{DefId, Expr, ExprId, Ir};
use dive_value::{Atom, Ctx, DeferredKind, Int, Interner, SetKind, Value, ValueOrder};

use crate::error::{EvalError, Result};
use crate::options::Options;
use crate::{ConstBinding, Model};

/// A literal prepared by the plan.
#[derive(Clone, Debug)]
pub(crate) enum Lit {
    /// Not a literal.
    None,
    /// The literal's value.
    Val(Value),
    /// The literal cannot be represented (a numeral too big for
    /// `--int=tlc`, a decimal).
    Err(EvalError),
}

/// Where `Print` and `PrintT` output goes.
pub trait PrintSink: Send + Sync {
    /// One printed line (TLC prints each `Print` as one `println`; the text
    /// may contain newlines from the pretty-printer).
    fn print(&self, text: String);
}

/// The default sink: collects the output in order.
#[derive(Default, Debug)]
pub struct CollectSink {
    lines: Mutex<Vec<String>>,
}

impl CollectSink {
    /// Takes everything printed so far.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lines.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl PrintSink for CollectSink {
    fn print(&self, text: String) {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(text);
    }
}

/// The shared evaluator state.
pub(crate) struct Core {
    pub(crate) input_refusal: Mutex<Option<String>>,
    /// Only the first TXT request has a proved native result representation.
    pub(crate) text_input_used: std::sync::atomic::AtomicBool,
    /// TLC initializes its static environment before any text-file result.
    pub(crate) environment_initialized: std::sync::atomic::AtomicBool,
    /// Names prepared from source before any startup evaluation can create more.
    pub(crate) input_name_baseline: usize,
    pub(crate) receiver_domain: Option<dive_value::ReceiverDomain>,
    pub(crate) ir: Arc<Ir>,
    pub(crate) opts: Options,
    pub(crate) ctx: Ctx,
    pub(crate) names: Arc<Interner>,
    pub(crate) consts: Vec<Option<ConstBinding>>,
    pub(crate) redirects: HashMap<DefId, DefId>,
    pub(crate) def_values: HashMap<DefId, Value>,
    /// Configuration overrides of definitions TLC implements in Java
    /// (`Nat <- NatOverride`): the override replaces the Java method.
    pub(crate) std_redirects: HashMap<dive_ir::StdOp, DefId>,
    /// [`Self::std_redirects`] by `StdOp` discriminant, for the lookup every
    /// standard-operator call makes.
    pub(crate) std_redirect_table: Box<[Option<DefId>]>,
    /// [`Self::redirects`] by definition index, for the lookup every call
    /// makes ([`Core::redirect`]).
    pub(crate) redirect_table: Box<[Option<DefId>]>,
    pub(crate) lits: Vec<Lit>,
    /// Interned field names of record constructors, record sets and record
    /// selections, by node.
    pub(crate) fields: Vec<Option<Box<[Atom]>>>,
    pub(crate) frame_owners: Vec<DefId>,
    pub(crate) frame_required: Vec<bool>,
    /// Values of zero-arity constant-level definitions, cached once computed
    /// (TLC precomputes them at start-up; only successes are kept, so a
    /// failing definition stays lazy, as in `SpecProcessor.processConstantDefns`).
    pub(crate) def_cache: Vec<OnceLock<Value>>,
    /// Additional state-independence proof for opportunistic constant caching.
    /// SANY levels can ignore native arguments (`Naturals` defines `a*b` as
    /// `TRUE`); explicit binding-stage startup values do not use this guard.
    pub(crate) constant_cache_safe: Vec<OnceLock<bool>>,
    /// Whether a call of each definition depends only on its arguments and
    /// the substituted constants it reads, which are listed
    /// (`Core::state_free` with its parameters and substitutions as inputs).
    pub(crate) call_cache_safe: Vec<OnceLock<Option<Arc<[dive_ir::DeclId]>>>>,
    /// The pure calls kept during the current start-up evaluation
    /// (`crate::startup_calls`); `None` outside one.
    pub(crate) startup_calls: Mutex<Option<crate::startup_calls::StartupCalls>>,
    /// How many calls start-up evaluations answered from kept results.
    pub(crate) startup_call_hits: std::sync::atomic::AtomicU64,
    /// Whether the configuration's `<-` bindings keep constant definitions
    /// constant: every definition a constant or definition is bound to is
    /// constant-level and pure. Then a definition that reads a constant
    /// (`Effects::OPEN`) is still computed once, as TLC does at start-up.
    pub(crate) open_ok: bool,
    /// The warnings printed so far (`MP.warningHistory`).
    pub(crate) warned: Mutex<std::collections::HashSet<String>>,
    /// Values the binding stage computed at start-up
    /// (`Evaluator::precompute`, TLC's `processConstantDefns`): every later
    /// use of the definition sees this value.
    pub(crate) startup: Vec<OnceLock<Value>>,
    /// Whether each [`Self::startup`] value, once set, is eager: it holds no
    /// deferred set anywhere inside it (`Evaluator::startup_fixed`).
    pub(crate) startup_eager: Vec<OnceLock<bool>>,
    pub(crate) sink: Arc<dyn PrintSink>,
    /// Actual Print/PrintT events, including quiet captures and empty text.
    pub(crate) printed: std::sync::atomic::AtomicBool,
    /// Exclusive simulation entry on this shared core, including evaluator clones.
    pub(crate) simulation_active: std::sync::atomic::AtomicBool,
    /// The run hosting the evaluator, and TLC's register file.
    pub(crate) host: crate::host::HostSlot,
    /// How many names were interned at start-up
    /// ([`crate::Evaluator::seal_startup_names`]).
    pub(crate) startup_names: std::sync::atomic::AtomicUsize,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("opts", &self.opts)
            .finish_non_exhaustive()
    }
}

thread_local! {
    // One counter across nested short loops, not one counter per invocation.
    // It contains no run state; the active core owns each actual control check.
    static LOOP_WORK: Cell<u16> = const { Cell::new(1024) };
}

impl Core {
    /// Amortized cooperative checkpoint for evaluator-owned loop work.
    #[inline]
    pub(crate) fn loop_work(&self) -> Result<()> {
        let due = LOOP_WORK.with(|counter| {
            let remaining = counter.get() - 1;
            counter.set(if remaining == 0 { 1024 } else { remaining });
            remaining == 0
        });
        // Infallible value equality/order/hash can force a deferred provider.
        // Those existing quiet units cannot transport interruption; do not
        // invoke the host there or let a transient error poison cached truth.
        if due && !dive_value::quiet_operation_active() {
            self.host.check_control()
        } else {
            Ok(())
        }
    }

    pub(crate) fn new(model: Model, opts: Options, sink: Arc<dyn PrintSink>) -> Self {
        let ir = model.ir;
        let names = model.names;
        let (frame_owners, frame_required) = crate::context::frame_metadata(&ir);
        let n = ir.nodes().len();
        let mut lits = Vec::with_capacity(n);
        let mut fields = Vec::with_capacity(n);
        for node in ir.nodes() {
            lits.push(match &node.expr {
                Expr::Bool(b) => Lit::Val(Value::Bool(*b)),
                Expr::Num(text) => match Int::from_literal(text, opts.ctx.int_mode) {
                    Ok(i) => Lit::Val(Value::int(i)),
                    Err(e) => Lit::Err(e.into()),
                },
                Expr::Decimal { int, frac } => Lit::Err(EvalError::coded(
                    crate::error::Ec::CantHandleReal,
                    format!("TLC can't handle real numbers.\n{int}.{frac}"),
                )),
                Expr::Str(s) => Lit::Val(Value::string(&names, s)),
                _ => Lit::None,
            });
            fields.push(match &node.expr {
                Expr::Record(fs) | Expr::RecordSet(fs) => {
                    Some(fs.iter().map(|f| names.intern(&f.name)).collect())
                }
                Expr::RecordSelect { field, .. } => {
                    Some(Box::new([names.intern(field)]) as Box<[Atom]>)
                }
                _ => None,
            });
        }
        let def_cache = (0..ir.defs().len()).map(|_| OnceLock::new()).collect();
        let constant_cache_safe = (0..ir.defs().len()).map(|_| OnceLock::new()).collect();
        let call_cache_safe = (0..ir.defs().len()).map(|_| OnceLock::new()).collect();
        let constant_pure = |d: &DefId| {
            let def = ir.def(*d);
            def.level == dive_ir::Level::Constant
                && !def.effects.intersects(dive_ir::Effects::IMPURE)
        };
        let open_ok = model.constants.iter().flatten().all(|b| match b {
            ConstBinding::Def(d) => constant_pure(d),
            ConstBinding::Value(_) => true,
        }) && model.redirects.values().all(constant_pure);
        let startup = (0..ir.defs().len()).map(|_| OnceLock::new()).collect();
        let startup_eager = (0..ir.defs().len()).map(|_| OnceLock::new()).collect();
        let std_redirects = model
            .redirects
            .iter()
            .filter_map(|(d, t)| ir.def(*d).std.map(|op| (op, *t)))
            .collect::<HashMap<_, _>>();
        let mut std_redirect_table = vec![None; dive_ir::StdOp::ALL.len()].into_boxed_slice();
        for (&op, &target) in &std_redirects {
            if let Some(slot) = std_redirect_table.get_mut(op as usize) {
                *slot = Some(target);
            }
        }
        let mut redirect_table = vec![None; ir.defs().len()].into_boxed_slice();
        for (&source, &target) in &model.redirects {
            if let Some(slot) = redirect_table.get_mut(source.index()) {
                *slot = Some(target);
            }
        }
        Self {
            input_refusal: Mutex::new(None),
            text_input_used: std::sync::atomic::AtomicBool::new(false),
            environment_initialized: std::sync::atomic::AtomicBool::new(false),
            input_name_baseline: names.len(),
            receiver_domain: dive_value::ReceiverDomain::new(),
            ir,
            ctx: opts.ctx,
            opts,
            names,
            consts: model.constants,
            redirects: model.redirects,
            def_values: model.def_values,
            std_redirects,
            std_redirect_table,
            redirect_table,
            lits,
            fields,
            frame_owners,
            frame_required,
            def_cache,
            constant_cache_safe,
            call_cache_safe,
            startup_calls: Mutex::default(),
            startup_call_hits: std::sync::atomic::AtomicU64::new(0),
            open_ok,
            warned: Mutex::default(),
            startup,
            startup_eager,
            sink,
            printed: std::sync::atomic::AtomicBool::new(false),
            startup_names: std::sync::atomic::AtomicUsize::new(0),
            host: crate::host::HostSlot::default(),
            simulation_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The error TLC reports when it processes the specification, before
    /// evaluating anything: the first numeral it cannot represent
    /// (`TLC_INTEGER_TOO_BIG`) or decimal (`TLC_CANT_HANDLE_REAL_NUMBERS`)
    /// in a module that is not a standard module.
    pub(crate) fn startup_error(&self) -> Option<EvalError> {
        for (i, lit) in self.lits.iter().enumerate() {
            let Lit::Err(e) = lit else { continue };
            let node = &self.ir.nodes()[i];
            let standard = node
                .origin
                .file
                .and_then(|f| self.ir.files().get(f.index()))
                .is_some_and(|f| f.standard);
            if !standard {
                return Some(e.clone());
            }
        }
        None
    }

    /// TLC's `SemanticNode.toString()` for an expression: the image of a
    /// numeral or decimal, otherwise the location string.
    pub(crate) fn loc(&self, e: ExprId) -> String {
        let Some(node) = self.ir.nodes().get(e.index()) else {
            return format!("unknown expression #{}", e.0);
        };
        match &node.expr {
            Expr::Num(text) => text.to_string(),
            Expr::Decimal { int, frac } => format!("{int}.{frac}"),
            _ => self.ir.location(&node.origin),
        }
    }

    /// `Values.ppr(v.toString())`.
    pub(crate) fn ppr(&self, v: &Value) -> String {
        dive_value::print::print_value(v, self.ctx)
            .unwrap_or_else(|_| dive_value::print::to_tlc_string_lenient(v, self.ctx))
    }

    /// The elements of a set in exploration order (open question 9): TLC's
    /// normalized order unless the options say to follow the value order.
    pub(crate) fn explore(&self, set: &dive_value::Set) -> Result<Arc<[Value]>> {
        self.enum_guard(set)?;
        let elems = set.elements(self.ctx)?;
        let order = self.opts.explore_order();
        if order == set.order() || elems.len() < 2 || !needs_resort(&elems) {
            return Ok(elems);
        }
        let mut v = elems.to_vec();
        v.sort_by(|a, b| a.cmp_with(b, order));
        Ok(v.into())
    }

    /// TLC's errors for enumerating a lazy set with a component it cannot
    /// enumerate (`SubsetValue`, `SetOfFcnsValue`); a refusal where dive does
    /// not reproduce TLC's message.
    pub(crate) fn enum_guard(&self, set: &dive_value::Set) -> Result<()> {
        let comps = set.components();
        match set.kind() {
            SetKind::Subset => {
                if let Some((_, b)) = comps.first() {
                    if is_user_value(b) {
                        return Err(EvalError::general(format!(
                            "Attempted to compute the number of elements in the overridden value {}.",
                            self.ppr(b)
                        )));
                    }
                    if let Value::Set(bs) = b {
                        self.enum_guard(bs)?;
                    }
                }
            }
            SetKind::FcnSet => {
                if let (Some((_, d)), Some((_, r))) = (comps.first(), comps.get(1)) {
                    // Pure structural emptiness can short-circuit before a
                    // symbolic domain/range is enumerated. Do not evaluate a
                    // deferred predicate an extra time during preflight.
                    if structural_set_operand(d)
                        && structural_set_operand(r)
                        && (dive_value::tlc::is_empty(d, self.ctx)?
                            || dive_value::tlc::is_empty(r, self.ctx)?)
                    {
                        return Ok(());
                    }
                    if is_user_value(d) {
                        return Err(EvalError::general(format!(
                            "Attempted to enumerate a set of the form [D -> R],but the domain D:\n{}\ncannot be enumerated.",
                            self.ppr(d)
                        )));
                    }
                    if let Value::Set(ds) = d {
                        self.enum_guard(ds)?;
                    }
                    if is_user_value(r) && !dive_value::tlc::is_empty(d, self.ctx)? {
                        return Err(EvalError::general(format!(
                            "Attempted to enumerate a set of the form [D -> R],but the range R:\n{}\ncannot be enumerated.",
                            self.ppr(r)
                        )));
                    }
                    if let Value::Set(rs) = r {
                        self.enum_guard(rs)?;
                    }
                }
            }
            SetKind::Product | SetKind::RcdSet => {
                for (_, c) in &comps {
                    if is_user_value(c) {
                        return Err(EvalError::unsupported(format!(
                            "enumerating {} with the infinite component {}",
                            self.ppr(&Value::Set(set.clone())),
                            self.ppr(c)
                        )));
                    }
                    if let Value::Set(cs) = c {
                        self.enum_guard(cs)?;
                    }
                }
            }
            SetKind::Seq | SetKind::Nat | SetKind::Int | SetKind::Strings => {
                return Err(EvalError::unsupported(format!(
                    "enumerating the infinite set {}",
                    self.ppr(&Value::Set(set.clone()))
                )));
            }
            _ => {}
        }
        Ok(())
    }

    /// TLC's error for the size of a lazy set with a component that is a
    /// special set constant (`UserValue.size`).
    pub(crate) fn size_guard(&self, set: &dive_value::Set) -> Result<()> {
        if let Some((d, r)) = set.as_fcn_set()
            && structural_set_operand(d)
            && structural_set_operand(r)
        {
            if dive_value::tlc::is_empty(d, self.ctx)? || dive_value::tlc::is_empty(r, self.ctx)? {
                return Ok(());
            }
            // SetOfFcnsValue.size asks for the range before the domain,
            // and a singleton range never asks for the domain's cardinality.
            self.component_size_guard(r)?;
            if let Value::Set(rs) = r
                && rs.size(self.ctx)? == 1
            {
                return Ok(());
            }
            return self.component_size_guard(d);
        }
        for (_, c) in set.components() {
            self.component_size_guard(&c)?;
        }
        Ok(())
    }

    fn component_size_guard(&self, c: &Value) -> Result<()> {
        if is_user_value(c) {
            return Err(EvalError::general(format!(
                "Attempted to compute the number of elements in the overridden value {}.",
                self.ppr(c)
            )));
        }
        if let Value::Set(cs) = c {
            self.size_guard(cs)?;
        }
        Ok(())
    }

    /// The elements of a set enumerated the way TLC's lazy sets enumerate
    /// their operands (`elements()` without an ordering), which never sizes
    /// the set first. dive's enumeration does, so a set too large to size is
    /// refused here rather than reported with an error TLC does not raise.
    pub(crate) fn explore_unsized(&self, set: &dive_value::Set) -> Result<Arc<[Value]>> {
        match self.explore(set) {
            Err(e)
                if e.is_eval()
                    && (e
                        .message()
                        .starts_with("Overflow when computing the number of elements")
                        || e.message().contains("more elements than dive enumerates")) =>
            {
                Err(EvalError::unsupported(format!(
                    "enumerating, without materializing it, a set too large to size: {}",
                    e.message().lines().last().unwrap_or("")
                )))
            }
            r => r,
        }
    }

    /// Prints a line through the sink, or into the thread's
    /// [`crate::capture::PrintCapture`].
    pub(crate) fn print(&self, text: String) {
        self.printed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(text) = crate::capture::push(text, false) {
            self.sink.print(text);
        }
    }

    /// Prints a once-per-run warning (`MP.printWarning`'s history): into the
    /// thread's capture, deduplicated when it is released, or through the
    /// sink if it is the first with this text.
    pub(crate) fn print_warning(&self, msg: String) {
        if crate::capture::active() {
            let _ = crate::capture::push(msg, true);
        } else if self.first_warning(&msg) {
            self.sink.print(msg);
        }
    }

    /// Records a warning in the run's history; whether it is new.
    pub(crate) fn first_warning(&self, msg: &str) -> bool {
        let mut warned = self.warned.lock().unwrap_or_else(PoisonError::into_inner);
        if warned.contains(msg) {
            false
        } else {
            warned.insert(msg.to_owned());
            true
        }
    }

    /// Read-only current evaluator depth; does not enter or waive a guard.
    pub(crate) fn evaluation_depth() -> u32 {
        DEPTH.with(Cell::get)
    }

    /// Enters one level of evaluation depth.
    pub(crate) fn enter(&self, what: ExprId) -> Result<DepthGuard> {
        DEPTH.with(|d| {
            let n = d.get();
            if n >= self.opts.max_depth {
                return Err(EvalError::resource_limit(format!(
                    "evaluation nested more than {} levels deep (recursion at {})",
                    self.opts.max_depth,
                    self.loc(what)
                ))
                .at(what));
            }
            d.set(n + 1);
            Ok(DepthGuard)
        })
    }
}

/// Whether the order of names matters for these elements: any string or
/// model value at any depth.
fn needs_resort(elems: &[Value]) -> bool {
    fn has_name(v: &Value, depth: u32) -> bool {
        if depth > 64 {
            return true;
        }
        match v {
            Value::Str(_) | Value::Model(_) => true,
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => false,
            Value::Fcn(f) => {
                f.as_record().is_some()
                    || f.iter()
                        .any(|(d, x)| has_name(&d, depth + 1) || has_name(x, depth + 1))
            }
            Value::Set(s) => !matches!(s.kind(), SetKind::Interval | SetKind::Nat | SetKind::Int),
        }
    }
    elems.iter().any(|v| has_name(v, 0))
}

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Leaves a level of evaluation depth when dropped.
pub(crate) struct DepthGuard;

impl Drop for DepthGuard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// TLC's `Value.getKindString()` for a value.
pub(crate) fn kind_string(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "a Boolean value",
        Value::Int(_) | Value::BigInt(_) => "an integer",
        Value::Str(_) => "a string",
        Value::Model(_) => "a model value",
        Value::Fcn(f) => {
            if f.is_record_literal() {
                "a record"
            } else if f.is_tuple_literal() {
                "a tuple"
            } else {
                "a function  of the form (d1 :> e1 @@ ... @@ dN :> eN)"
            }
        }
        Value::Set(s) => set_kind_string(s),
    }
}

fn set_kind_string(s: &dive_value::Set) -> &'static str {
    match s.kind() {
        SetKind::Enum | SetKind::Raw => "a set of the form {e1, ... ,eN}",
        SetKind::Interval => "a set of the form i..j",
        SetKind::Subset => "a set of the form SUBSET S",
        SetKind::FcnSet => "a set of the form [S -> T]",
        SetKind::RcdSet => "a set of the form [d1 : S1, ... , dN : SN]",
        SetKind::Product => "a cartesian product",
        SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq => "a special set constant",
        SetKind::Deferred => deferred_kind_string(s),
    }
}

fn deferred_kind_string(s: &dive_value::Set) -> &'static str {
    match s.as_deferred().map(|d| d.kind()) {
        Some(DeferredKind::Cup) => "a set of the form S \\cup T",
        Some(DeferredKind::Cap) => "a set of the form S \\cap T",
        Some(DeferredKind::Diff) => "a set of the form S \\ T",
        Some(DeferredKind::Union) => "a set of the form UNION  S",
        Some(DeferredKind::Map) => "a set of the form {e1, ... ,eN}",
        _ => "a set of the form {x \\in S : expr}",
    }
}

/// The value class TLC's `Value` subclass name (`tlc2.value.impl.<Name>`),
/// for messages that print Java class names.
pub(crate) fn java_class(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "BoolValue",
        Value::Int(_) | Value::BigInt(_) => "IntValue",
        Value::Str(_) => "StringValue",
        Value::Model(_) => "ModelValue",
        Value::Fcn(f) => {
            if f.is_record_literal() {
                "RecordValue"
            } else if f.is_tuple_literal() {
                "TupleValue"
            } else {
                "FcnRcdValue"
            }
        }
        Value::Set(s) => match s.kind() {
            SetKind::Enum | SetKind::Raw => "SetEnumValue",
            SetKind::Interval => "IntervalValue",
            SetKind::Subset => "SubsetValue",
            SetKind::FcnSet => "SetOfFcnsValue",
            SetKind::RcdSet => "SetOfRcdsValue",
            SetKind::Product => "SetOfTuplesValue",
            SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq => "UserValue",
            SetKind::Deferred => match s.as_deferred().map(|d| d.kind()) {
                Some(DeferredKind::Cup) => "SetCupValue",
                Some(DeferredKind::Cap) => "SetCapValue",
                Some(DeferredKind::Diff) => "SetDiffValue",
                Some(DeferredKind::Union) => "UnionValue",
                Some(DeferredKind::Map) => "SetEnumValue",
                _ => "SetPredValue",
            },
        },
    }
}

/// Whether TLC holds the value as a `UserValue` (`Nat`, `Int`, `STRING`,
/// `Seq(S)`).
pub(crate) fn is_user_value(v: &Value) -> bool {
    matches!(v, Value::Set(s) if matches!(s.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq))
}

/// Whether TLC's value is `Reducible` (`SetEnumValue` or `IntervalValue`).
pub(crate) fn is_reducible(v: &Value) -> bool {
    matches!(v, Value::Set(s) if matches!(s.kind(), SetKind::Enum | SetKind::Raw | SetKind::Interval))
}

/// Whether TLC's value is `Enumerable`: every set except the special set
/// constants `Nat`, `Int`, `STRING` and `Seq(S)` (`UserValue`).
pub(crate) fn is_enumerable(v: &Value) -> bool {
    matches!(v, Value::Set(s) if !matches!(s.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq))
}

/// The value order in which `TLC` enumerates normalized sets.
pub(crate) const TLC_ORDER: ValueOrder = ValueOrder::Tlc;

// Structural isEmpty/size checks cannot execute a lazy predicate. Keep existing
// guarded refusals for deferred operands until their evaluation order is ported.
fn structural_set_operand(value: &Value) -> bool {
    let mut todo = vec![value.clone()];
    while let Some(v) = todo.pop() {
        if let Value::Fcn(f) = &v {
            todo.extend(f.iter().flat_map(|(d, v)| [d, v.clone()]));
            continue;
        }
        let Value::Set(s) = v else { continue };
        if s.kind() == SetKind::Deferred {
            return false;
        }
        if let Some(elements) = s.raw_elements() {
            todo.extend(elements.iter().cloned());
        }
        todo.extend(s.components().into_iter().map(|(_, v)| v));
    }
    true
}

#[cfg(test)]
mod print_events {
    #![allow(clippy::expect_used, reason = "test setup")]
    use super::*;

    #[test]
    fn actual_empty_print_event_survives_drain_and_quiet_capture_without_sink_calls() {
        let analysis = dive_sema::analyze_source(
            "PrintFacts.tla",
            "---- MODULE PrintFacts ----\nEXTENDS TLC\nUnused == PrintT(\"unused\")\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let ev = crate::Evaluator::new(crate::Model::new(ir), Options::tlc()).expect("context");
        assert!(
            !ev.has_print_event(),
            "unused Print definition is not execution"
        );
        ev.core.print_warning("warning".into());
        assert!(!ev.has_print_event(), "warnings are separate events");
        assert_eq!(ev.take_output(), ["warning"]);
        let cap = crate::capture::PrintCapture::start();
        ev.core.print(String::new());
        assert!(ev.has_print_event());
        assert!(ev.take_output().is_empty(), "quiet event did not call sink");
        assert_eq!(
            cap.take(),
            [crate::capture::Captured {
                text: String::new(),
                warning: false
            }]
        );
        drop(cap);
        assert!(
            ev.has_print_event(),
            "draining a quiet capture does not erase execution"
        );
    }
}
