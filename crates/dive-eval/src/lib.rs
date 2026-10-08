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
//! let ev = Evaluator::new(Model::new(Arc::new(ir)), Options::tlc());
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
//! build without the feature (the independent trace validator) refuses
//! `ENABLED`.
#![forbid(unsafe_code)]

mod core;
mod env;
mod error;
mod eval;
mod lazy;
#[cfg(feature = "enumerate")]
pub mod next;
mod options;
mod scope;
mod std;
mod tla_cmp;
#[cfg(feature = "enumerate")]
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
    #[must_use]
    pub fn new(model: Model, opts: Options) -> Self {
        let sink = Arc::new(CollectSink::default());
        let mut ev = Self::with_sink(model, opts, sink.clone());
        ev.collected = Some(sink);
        ev
    }

    /// Takes the `Print` output collected so far (empty with a custom sink).
    #[must_use]
    pub fn take_output(&self) -> Vec<String> {
        self.collected
            .as_ref()
            .map(|s| s.take())
            .unwrap_or_default()
    }

    /// An evaluator with its own `Print` sink.
    #[must_use]
    pub fn with_sink(model: Model, opts: Options, sink: Arc<dyn PrintSink>) -> Self {
        Self {
            core: Arc::new(Core::new(model, opts, sink)),
            collected: None,
        }
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
        let d = self.core.ir.def(def);
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
        let d = self.core.ir.def(def);
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
        let ir = &self.core.ir;
        let def = ir.def(a);
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
