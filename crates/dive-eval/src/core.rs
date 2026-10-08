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
    pub(crate) lits: Vec<Lit>,
    /// Interned field names of record constructors, record sets and record
    /// selections, by node.
    pub(crate) fields: Vec<Option<Box<[Atom]>>>,
    /// Values of zero-arity constant-level definitions, cached once computed
    /// (TLC precomputes them at start-up; only successes are kept, so a
    /// failing definition stays lazy, as in `SpecProcessor.processConstantDefns`).
    pub(crate) def_cache: Vec<OnceLock<Value>>,
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
    pub(crate) sink: Arc<dyn PrintSink>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("opts", &self.opts)
            .finish_non_exhaustive()
    }
}

impl Core {
    pub(crate) fn new(model: Model, opts: Options, sink: Arc<dyn PrintSink>) -> Self {
        let ir = model.ir;
        let names = model.names;
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
        let std_redirects = model
            .redirects
            .iter()
            .filter_map(|(d, t)| ir.def(*d).std.map(|op| (op, *t)))
            .collect();
        Self {
            ir,
            ctx: opts.ctx,
            opts,
            names,
            consts: model.constants,
            redirects: model.redirects,
            def_values: model.def_values,
            std_redirects,
            lits,
            fields,
            def_cache,
            open_ok,
            warned: Mutex::default(),
            startup,
            sink,
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
        let node = self.ir.node(e);
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
        for (_, c) in set.components() {
            if is_user_value(&c) {
                return Err(EvalError::general(format!(
                    "Attempted to compute the number of elements in the overridden value {}.",
                    self.ppr(&c)
                )));
            }
            if let Value::Set(cs) = &c {
                self.size_guard(cs)?;
            }
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

    /// Prints a line through the sink.
    pub(crate) fn print(&self, text: String) {
        self.sink.print(text);
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
