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
//! `dive_eval::next` (behind the `enumerate` feature, on by default) ports
//! TLC's `getInitStates`, `getNextStates`, `getActions` and `enabled`: it
//! generates initial states and successors in TLC's order, duplicates
//! included, evaluating the non-action parts of an action through this
//! interpreter against the partial successor it is building, and uses
//! [`EvalError`] for the error classes. `ENABLED` goes through it too, so a
//! core build without the feature refuses `ENABLED`; the independent trace
//! validator uses its separate bounded finite-cover procedure.
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
mod lazy;
#[cfg(feature = "enumerate")]
pub mod next;
mod options;
mod scope;
mod std;
mod tla_cmp;
mod tlc_enum;

/// Without the successor enumerator, `ENABLED` (which searches for a
/// successor with TLC's `enabled`) is refused.
#[cfg(not(feature = "enumerate"))]
impl Core {
    pub(crate) fn eval_enabled(
        self: &Arc<Self>,
        _env: &mut Env,
        _st: &St,
        _a: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        Err(EvalError::unsupported(format!(
            "ENABLED without the successor enumerator (the `enumerate` feature) ({})",
            self.loc(e)
        )))
    }
}

use ::std::collections::HashMap;
use ::std::sync::Arc;

use dive_ir::{DefId, DefKind, ExprId, Ir};
use dive_value::{Interner, Value};

pub use crate::core::{CollectSink, PrintSink};
pub use crate::env::{St, StateRef};
pub use crate::error::{Ec, ErrorClass, EvalError, Result};
pub use crate::host::{Counter, Host, HostError, RunConfig, TLCGET_NAMES};
pub use crate::lazy::LazyFcn;
pub use crate::options::{
    Comparisons, DEFAULT_MAX_DEPTH, ExplorationOrder, Options, Profile, RECOMMENDED_STACK,
};
pub use crate::scope::{Scope, SlotView};

use crate::core::Core;
use crate::env::{Dyn, Env, Ev};

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

    /// Installs the run hosting the evaluator (see [`Host`]): the model
    /// checker's live state, its random streams and `TLCSet("exit")`.
    /// Replaces any host installed before.
    pub fn set_host(&self, host: Arc<dyn Host>) {
        let _ = self.replace_host(Some(host));
    }

    /// Installs `host` (or none) and returns the host installed before.
    pub fn replace_host(&self, host: Option<Arc<dyn Host>>) -> Option<Arc<dyn Host>> {
        ::std::mem::replace(
            &mut *self
                .core
                .host
                .host
                .lock()
                .unwrap_or_else(::std::sync::PoisonError::into_inner),
            host,
        )
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
    pub fn intern_checker_names(&self) {
        for n in TLCGET_NAMES {
            self.core.names.intern(n);
        }
    }

    /// Interns the actual configuration strings constructed eagerly by
    /// TLC's `AbstractChecker.createConfig`, after [`Self::intern_checker_names`]
    /// and before assumptions. Even a spec that never reads the config can
    /// observe these tokens through runtime `CHOOSE` ordering.
    pub fn intern_bfs_config(&self, config: &RunConfig) {
        self.core.names.intern("bfs");
        self.core.names.intern(&config.seed.to_string());
        self.core.names.intern(&config.fingerprint.to_string());
        self.core.names.intern(&config.install);
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
