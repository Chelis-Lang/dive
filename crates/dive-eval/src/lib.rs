//! `dive-eval`: the evaluator for expressions and actions over states (ADR
//! 0014).
//!
//! It evaluates the IR ([`dive_ir`], ADR 0013) over `dive-value` values with
//! TLC's semantics: TLC's evaluation order, its laziness (operator arguments,
//! zero-arity `LET` definitions and instance substitutions are thunks forced
//! under TLC's caching rule), its lazy sets and functions, `CHOOSE` in the
//! run's value order, and TLC's errors with TLC's messages. Anything it does
//! not implement exactly is refused as [`ErrorClass::Unsupported`], naming
//! the feature.
//!
//! # Using it
//!
//! A [`Model`] binds the IR's constants (normally built by `dive-model` from
//! the configuration); an [`Evaluator`] evaluates definitions, expressions
//! and assumptions under [`Options`]:
//!
//! ```
//! use std::sync::Arc;
//! use dive_eval::{Evaluator, Model, Options, St};
//! use dive_sema::{Options as SemaOptions, analyze_source};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nTwice(n) == n + n\nX == Twice(21)\n====\n";
//! let ir = dive_ir::lower(&analyze_source("M.tla", src, &SemaOptions::default())).expect("lowers");
//! let x = ir.root_def("X").expect("defined");
//! let ev = Evaluator::new(Model::new(Arc::new(ir)), Options::tlc()).expect("valid context");
//! assert_eq!(ev.eval_def(x, &St::constant()).expect("evaluates"), 42.into());
//! ```
//!
//! # How the successor generator plugs in
//!
//! `dive_eval::next` ports TLC's `getInitStates`, `getNextStates`,
//! `getActions` and `enabled`: it generates initial states and successors in
//! TLC's order, duplicates included, evaluating the non-action parts of an
//! action through this interpreter against the partial successor it is
//! building, and uses [`EvalError`] for the error classes. `ENABLED` goes
//! through it too, for the checker and the trace validator alike (ADR 0103).
#![forbid(unsafe_code)]

mod bounded_domain;
pub mod capture;
mod community;
mod context;
mod core;
pub use context::{ContextError, ContextErrorKind, validate_ir};
mod env;
mod error;
mod eval;
mod finite;
mod host;
mod json;
mod lazy;
mod named_state;
pub mod next;
mod options;
mod random_relation;
mod scope;
mod std;
mod svg;
mod tla_cmp;
mod tlc_enum;

use ::std::collections::HashMap;
use ::std::sync::Arc;

use dive_ir::{DefId, DefKind, ExprId, Ir};
use dive_value::{Interner, Value};

pub use crate::core::{CollectSink, PrintSink};
pub use crate::env::{St, StateRef};
pub use crate::error::{Ec, ErrorClass, EvalError, Result};
pub use crate::host::{Counter, Host, HostError, RunConfig, TLCGET_NAMES};
pub use crate::lazy::{LazyFcn, deferred_filter_normalization_operand};
pub use crate::named_state::NamedState;
pub use crate::options::{
    ActionComposition, Comparisons, DEFAULT_MAX_DEPTH, ExplorationOrder, Options, Profile,
    RECOMMENDED_STACK,
};
pub use crate::scope::{Scope, SlotView};

use crate::core::Core;
use crate::env::{Dyn, Env, Ev};

/// Exclusive simulation entry on a shared evaluator core. Dropping the lease
/// permits the next simulation. The driver must restore its host before drop.
/// Ordinary evaluator operations and host setters keep their existing behavior;
/// callers must not mix foreign evaluator/host operations with an active simulation.
#[derive(Debug)]
pub struct SimulationLease<'a> {
    active: &'a ::std::sync::atomic::AtomicBool,
}

impl Drop for SimulationLease<'_> {
    fn drop(&mut self) {
        self.active
            .store(false, ::std::sync::atomic::Ordering::Release);
    }
}

/// What the configuration binds a constant to.
#[derive(Clone, Debug)]
pub enum ConstBinding {
    /// A value (`C = v` in the `.cfg`, including model values).
    Value(Value),
    /// A definition (`C <- D`), also for constant operators (`F(_)`).
    Def(DefId),
}

/// The bindings evaluation needs beyond the IR: the run's names and the
/// constants. `dive-model` builds it from the configuration (ADR 0013 §8).
#[derive(Clone, Debug)]
pub struct Model {
    /// The lowered specification.
    pub ir: Arc<Ir>,
    /// The run's names. To reproduce TLC's order of strings and model values
    /// under `--choose-order=tlc`, intern them in SANY/TLC's encounter order
    /// before building the evaluator (ADR 0012).
    pub names: Arc<Interner>,
    /// Constant bindings, by [`dive_ir::ConstId`].
    pub constants: Vec<Option<ConstBinding>>,
    /// Definition overrides from the configuration (`Op <- Other`).
    pub redirects: HashMap<DefId, DefId>,
    /// Definitions the configuration assigns a value (`Op = v`).
    pub def_values: HashMap<DefId, Value>,
}

impl Model {
    /// A model with no constants bound and a fresh name table.
    #[must_use]
    pub fn new(ir: Arc<Ir>) -> Self {
        let n = ir.consts().len();
        Self {
            ir,
            names: Arc::new(Interner::new()),
            constants: vec![None; n],
            redirects: HashMap::new(),
            def_values: HashMap::new(),
        }
    }
}

/// The outcome of checking the assumptions, as TLC's
/// `Tool.checkAssumptions` reports it.
#[derive(Clone, Debug)]
pub enum AssumptionOutcome {
    /// Every assumption is `TRUE`.
    AllTrue,
    /// An assumption is `FALSE`: TLC's `TLC_ASSUMPTION_FALSE` (2104).
    False {
        /// The assumption.
        def: DefId,
        /// TLC's message.
        message: String,
    },
    /// Evaluating an assumption failed: TLC's
    /// `TLC_ASSUMPTION_EVALUATION_ERROR` (2105), or a refusal or resource
    /// limit.
    Error {
        /// The assumption.
        def: DefId,
        /// The error.
        error: EvalError,
        /// TLC's message (for an evaluation error).
        message: String,
    },
}

/// An evaluator for one specification and model.
#[derive(Clone, Debug)]
pub struct Evaluator {
    core: Arc<Core>,
    collected: Option<Arc<CollectSink>>,
}

impl Evaluator {
    /// Whether both handles share the exact mutable evaluation core. This
    /// callback-free identity check binds deferred execution receipts to their
    /// original run; it is not semantic equality, graph or host authority.
    #[must_use]
    pub fn shares_run_state(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.core, &other.core)
    }
    /// An evaluator that collects `Print` output (see [`Evaluator::take_output`]).
    /// Admission checks IR references, override signatures/cycles and captured
    /// values before interning literals or invoking caller code. Bare numeric IDs
    /// are relative to `model.ir`; coincidentally equal foreign indices are not
    /// provenance certificates.
    ///
    /// # Errors
    /// A typed malformed context or bounded admission refusal.
    pub fn new(model: Model, opts: Options) -> ::std::result::Result<Self, ContextError> {
        let sink = Arc::new(CollectSink::default());
        let mut ev = Self::with_sink(model, opts, sink.clone())?;
        ev.collected = Some(sink);
        Ok(ev)
    }

    /// Takes the `Print` output collected so far (empty with a custom sink).
    #[must_use]
    pub fn take_output(&self) -> Vec<String> {
        self.collected
            .as_ref()
            .map(|s| s.take())
            .unwrap_or_default()
    }

    /// Whether this evaluator has executed a `Print` or `PrintT` event.
    ///
    /// This monotonic execution fact includes empty text, custom sinks and
    /// nested quiet captures. Warnings and unexecuted definitions do not set it;
    /// draining output does not reset it. It carries no replay certificate.
    #[must_use]
    pub fn has_print_event(&self) -> bool {
        self.core
            .printed
            .load(::std::sync::atomic::Ordering::Relaxed)
    }

    /// Releases captured output ([`capture::PrintCapture`]) through the
    /// sink, in the order given. A captured warning is kept only if no
    /// warning with its text was printed before in the run.
    pub fn release(&self, lines: Vec<capture::Captured>) {
        for line in lines {
            if !line.warning || self.core.first_warning(&line.text) {
                self.core.sink.print(line.text);
            }
        }
    }

    /// Applies the run's warning history to captured output without calling
    /// any sink. Print events are retained even when their text repeats.
    /// This is an alternative to [`Self::release`] for drivers that return
    /// captured observations themselves; do not release the filtered warnings
    /// a second time, since they are already recorded in the run's history.
    ///
    /// ```
    /// use dive_eval::{Evaluator, capture::{Captured, PrintCapture}};
    /// fn retained(ev: &Evaluator, cap: &PrintCapture) -> Vec<Captured> {
    ///     ev.filter_captured_warnings(cap.take())
    /// }
    /// ```
    #[must_use]
    pub fn filter_captured_warnings(
        &self,
        lines: Vec<capture::Captured>,
    ) -> Vec<capture::Captured> {
        lines
            .into_iter()
            .filter(|line| !line.warning || self.core.first_warning(&line.text))
            .collect()
    }

    /// Whether evaluating in `scope` can run an expression with side effects
    /// (`Print`, `TLCSet`, randomness, ...) through a lazily evaluated value
    /// that every evaluation shares: a cached `LazyValue` or a lazy function
    /// bound when the model was bound. Which evaluation forces such a value
    /// first, and so where its side effects happen, depends on the order of
    /// evaluation, so a parallel checker must not reorder those evaluations.
    #[must_use]
    pub fn scope_shares_side_effects(&self, scope: &Scope) -> bool {
        self.validate_scope(scope).is_err() || env::shares_side_effects(&self.core.ir, &scope.env)
    }

    /// Whether retained lazy expressions or actual captured operators may print,
    /// draw randomness, perform I/O or read live checker state. Foreign scopes
    /// and unresolved callable targets conservatively return true. Pure captured
    /// operators remain usable; the caller-supplied argument level is not authority
    /// for a callable's effects.
    ///
    /// This inspects retained contexts without executing expressions, forcing
    /// callbacks or scanning arbitrary stored values. It complements static IR
    /// effects when public Rust callers supply a predicate's scope themselves.
    #[must_use]
    pub fn scope_has_effects(&self, scope: &Scope) -> bool {
        use dive_ir::{Effects, StdSupport};
        if self.validate_scope(scope).is_err() {
            return true;
        }
        let native = |op: dive_ir::StdOp| {
            op.impure() || matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
        };
        let definition = |id: dive_ir::DefId| {
            let Some(def) = self.core.ir.defs().get(self.core.redirect(id).index()) else {
                return true;
            };
            def.std.map_or_else(
                || {
                    self.core
                        .ir
                        .nodes()
                        .get(def.body.index())
                        .is_none_or(|node| {
                            node.effects
                                .intersects(Effects::IMPURE | Effects::LIVE_TLCGET)
                        })
                },
                native,
            )
        };
        env::scope_has_effects(&self.core.ir, &scope.env, &|operator| match operator {
            env::OpVal::Std(op) => native(*op),
            env::OpVal::Def { def, .. } => definition(*def),
            env::OpVal::Const(id) => {
                match self.core.consts.get(id.index()).and_then(Option::as_ref) {
                    Some(ConstBinding::Def(def)) => definition(*def),
                    Some(ConstBinding::Value(_)) | None => true,
                }
            }
            env::OpVal::Builtin(_) => false,
            env::OpVal::Unbound(_) => true,
        })
    }

    /// An evaluator with its own `Print` sink, admitted before any sink call.
    ///
    /// # Errors
    /// The same context admission failures as [`Self::new`].
    pub fn with_sink(
        mut model: Model,
        opts: Options,
        sink: Arc<dyn PrintSink>,
    ) -> ::std::result::Result<Self, ContextError> {
        context::validate(&mut model)?;
        Ok(Self {
            core: Arc::new(Core::new(model, opts, sink)),
            collected: None,
        })
    }

    /// Claims exclusive simulation entry without waiting. Evaluator clones share
    /// the same core, so overlapping and reentrant claims return `None`.
    /// Hold this lease through host restoration. Independent cores remain free
    /// to simulate concurrently. No host-registration lock is held by the lease.
    ///
    /// ```
    /// use dive_eval::Evaluator;
    /// fn shared_core(ev: &Evaluator) {
    ///     let lease = ev.try_simulation_lease();
    ///     assert!(lease.is_some());
    ///     assert!(ev.clone().try_simulation_lease().is_none());
    ///     drop(lease);
    ///     assert!(ev.try_simulation_lease().is_some());
    /// }
    /// ```
    #[must_use]
    pub fn try_simulation_lease(&self) -> Option<SimulationLease<'_>> {
        self.core
            .simulation_active
            .compare_exchange(
                false,
                true,
                ::std::sync::atomic::Ordering::AcqRel,
                ::std::sync::atomic::Ordering::Acquire,
            )
            .ok()
            .map(|_| SimulationLease {
                active: &self.core.simulation_active,
            })
    }

    /// Installs the run hosting the evaluator (see [`Host`]): the model
    /// checker's live state, its random streams and `TLCSet("exit")`.
    /// Replaces any host installed before.
    pub fn set_host(&self, host: Arc<dyn Host>) {
        let _ = self.replace_host(Some(host));
    }

    /// Installs `host` (or none) and returns the host installed before.
    #[must_use = "the previous host must be restored or dropped deliberately"]
    pub fn replace_host(&self, host: Option<Arc<dyn Host>>) -> Option<Arc<dyn Host>> {
        self.core.host.replace(host)
    }

    /// The installed host, if any.
    #[must_use]
    pub fn host(&self) -> Option<Arc<dyn Host>> {
        self.core.host.get()
    }

    /// Interns the names TLC interns when it creates its model checker
    /// ([`TLCGET_NAMES`]), in TLC's order. A driver that reproduces TLC's
    /// name order calls it after the binding stage and before checking the
    /// assumptions, where TLC creates its `ModelChecker`.
    /// They are start-up names ([`Self::startup_names`]) if every name
    /// interned before them was.
    pub fn intern_checker_names(&self) {
        self.extend_startup_names(|| {
            for n in TLCGET_NAMES {
                self.core.names.intern(n);
            }
        });
    }

    /// Interns the actual configuration strings constructed eagerly by
    /// TLC's `AbstractChecker.createConfig`, after [`Self::intern_checker_names`]
    /// and before assumptions. Even a spec that never reads the config can
    /// observe these tokens through runtime `CHOOSE` ordering.
    /// They are start-up names ([`Self::startup_names`]) if every name
    /// interned before them was.
    pub fn intern_bfs_config(&self, config: &RunConfig) {
        self.extend_startup_names(|| {
            self.core.names.intern("bfs");
            self.core.names.intern(&config.seed.to_string());
            self.core.names.intern(&config.fingerprint.to_string());
            self.core.names.intern(&config.install);
        });
    }

    /// Records every name interned so far as a start-up name: one TLC
    /// interns, in the same order, before it evaluates an `ASSUME` or a
    /// state. The binding stage calls it when it finishes.
    pub fn seal_startup_names(&self) {
        self.core
            .startup_names
            .store(self.core.names.len(), ::std::sync::atomic::Ordering::SeqCst);
    }

    /// The number of start-up names ([`Self::seal_startup_names`]). A name
    /// with a higher token was created at run time: its position in TLC's
    /// encounter order depends on what was evaluated before it.
    #[must_use]
    pub fn startup_names(&self) -> usize {
        self.core
            .startup_names
            .load(::std::sync::atomic::Ordering::SeqCst)
    }

    /// Runs `f`, which interns start-up names, and counts them as start-up
    /// names only if no run-time name was interned before them.
    fn extend_startup_names(&self, f: impl FnOnce()) {
        let contiguous = self.core.names.len() == self.startup_names();
        f();
        if contiguous {
            self.seal_startup_names();
        }
    }

    /// Compares two given values with this evaluator's comparison profile.
    /// This is the equality used by the interpreter, including TLC's errors
    /// under `Comparisons::Tlc` and the approved TLA+ comparison semantics.
    /// It does not evaluate an expression or generate successor states.
    ///
    /// # Errors
    /// A comparison the profile cannot decide, or a value/resource error.
    pub fn equal_values(&self, a: &Value, b: &Value) -> Result<bool> {
        self.core.equals(a, b)
    }

    /// The configured binding of a root constant. This exposes the actual
    /// value or definition for conservative scoped preflight; it evaluates
    /// nothing and does not resolve instance substitutions.
    #[must_use]
    pub fn constant_binding(&self, constant: dive_ir::ConstId) -> Option<ConstBinding> {
        self.core.consts.get(constant.index()).cloned().flatten()
    }

    /// The options.
    #[must_use]
    pub fn options(&self) -> &Options {
        &self.core.opts
    }

    /// The IR.
    #[must_use]
    pub fn ir(&self) -> &Arc<Ir> {
        &self.core.ir
    }

    /// Evaluates a zero-arity definition.
    ///
    /// # Errors
    /// Any evaluation error, refusal or resource limit.
    pub fn eval_def(&self, def: DefId, st: &St) -> Result<Value> {
        self.validate_state_context(st)?;
        let d = self.admit_def(def)?;
        if !d.params.is_empty() {
            return Err(EvalError::bug(format!(
                "{} takes {} arguments",
                d.name,
                d.params.len()
            )));
        }
        let mut env = Env::new(def, d.depth, d.frame.len(), None, Dyn::default());
        let ev = self.core.eval(&mut env, st, d.body)?;
        self.core.force_fcn(ev)
    }

    /// Evaluates the body of a top-level definition as a predicate or value
    /// in its own frame: `expr` must belong to `def`'s frame.
    ///
    /// # Errors
    /// Any evaluation error, refusal or resource limit.
    pub fn eval_in(&self, def: DefId, expr: ExprId, st: &St) -> Result<Value> {
        self.validate_state_context(st)?;
        let scope = self.scope(def)?;
        self.admit_scoped_expr(&scope, expr)?;
        let d = self.admit_def(def)?;
        let mut env = Env::new(def, d.depth, d.frame.len(), None, Dyn::default());
        let ev = self.core.eval(&mut env, st, expr)?;
        self.core.force_fcn(ev)
    }

    /// TLC's `Tool.checkAssumptions`: evaluates the root module's
    /// assumptions in order (skipping those `is_axiom` selects), stopping at
    /// the first that is not `TRUE`.
    pub fn check_assumptions(&self, is_axiom: impl Fn(DefId) -> bool) -> AssumptionOutcome {
        let ir = &self.core.ir;
        let root = ir.module(ir.root());
        for &a in &root.assumptions {
            if is_axiom(a) {
                continue;
            }
            match self.check_assumption(a) {
                AssumptionOutcome::AllTrue => {}
                other => return other,
            }
        }
        AssumptionOutcome::AllTrue
    }

    /// Evaluates one assumption as TLC does (`Tool.isValid(expr)`: states
    /// `Empty`, control `Const`).
    #[must_use]
    pub fn check_assumption(&self, a: DefId) -> AssumptionOutcome {
        let def = match self.admit_def(a) {
            Ok(def) => def,
            Err(error) => {
                return AssumptionOutcome::Error {
                    def: a,
                    message: error.to_string(),
                    error,
                };
            }
        };
        let loc = self.core.loc(def.body);
        let mut env = Env::new(a, def.depth, def.frame.len(), None, Dyn::default());
        match self.core.eval(&mut env, &St::constant(), def.body) {
            Ok(Ev::V(Value::Bool(true))) => AssumptionOutcome::AllTrue,
            Ok(Ev::V(Value::Bool(false))) => AssumptionOutcome::False {
                def: a,
                message: format!("Assumption {loc} is false."),
            },
            Ok(_) => {
                let error = EvalError::coded(
                    Ec::ExpectedValue,
                    format!("TLC expected a boolean value, but did not find one. {loc}"),
                );
                let message = format!("Evaluating assumption {loc} failed.\n{}", error.tool_text());
                AssumptionOutcome::Error {
                    def: a,
                    error,
                    message,
                }
            }
            Err(error) => {
                let message = format!("Evaluating assumption {loc} failed.\n{}", error.tool_text());
                AssumptionOutcome::Error {
                    def: a,
                    error,
                    message,
                }
            }
        }
    }

    /// The definition the configuration makes `def` stand for, following
    /// `Op <- Other` overrides; `def` itself when it is not overridden.
    #[must_use]
    pub fn redirected(&self, def: DefId) -> DefId {
        self.core.redirect(def)
    }

    /// Whether the configuration assigns `def` a value (`Op = v`).
    #[must_use]
    pub fn is_value_override(&self, def: DefId) -> bool {
        self.core.def_values.contains_key(&def)
    }

    /// The error TLC raises while processing the specification, before
    /// checking assumptions: a numeral too big for `--int=tlc`
    /// (`TLC_INTEGER_TOO_BIG`) or a decimal (`TLC_CANT_HANDLE_REAL_NUMBERS`).
    /// The binding stage reports it as TLC does, as a top-level error.
    #[must_use]
    pub fn startup_error(&self) -> Option<EvalError> {
        self.core.startup_error()
    }

    /// TLC's location string for an expression (`SemanticNode.toString()`).
    #[must_use]
    pub fn location(&self, e: ExprId) -> String {
        self.core.loc(e)
    }

    /// The assumptions of the root module, in order, with whether each is a
    /// theorem-like definition kind TLC never evaluates.
    #[must_use]
    pub fn assumptions(&self) -> Vec<DefId> {
        let ir = &self.core.ir;
        ir.module(ir.root())
            .assumptions
            .iter()
            .copied()
            .filter(|&d| ir.def(d).kind == DefKind::Assumption)
            .collect()
    }
}
