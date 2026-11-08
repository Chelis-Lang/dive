//! The successor generator: ports of TLC's `Tool.getInitStates`,
//! `Tool.getNextStates`, `processUnchanged`, `enabled` and `getActions`
//! (`tlc2/tool/impl/Tool.java` at tlaplus `94d0c50`), rule by rule
//! (ADR 0014 §3).
//!
//! **What it reproduces.** For each state and action, the exact sequence of
//! successors TLC's enumerator hands to its functor, duplicates included,
//! since "states generated" counts every one (probe `DuplicateSuccessors`).
//! The enumeration works in continuation-passing style over an explicit
//! continuation list, TLC's `ActionItemList`, with one successor buffer that
//! is bound and unbound as TLC binds and unbinds its `TLCStateMut`:
//!
//! | Construct | Rule (Java) |
//! |---|---|
//! | `A /\ B /\ C` | `A` with the continuation `B, C` consed in order (`:1176-1183`) |
//! | `A \/ B` | each disjunct with the same continuation, left to right (`:1184-1207`) |
//! | `\E x \in S, y \in T : A` | one enumeration of `A` per binding, normalized, **first** variable fastest (`ContextEnumerator`, `:1208-1229`; probes `ExistsOrder`, `ExistsSetOrder`) |
//! | `\A x \in S : A` | the first binding's `A`, with the other bindings consed onto the continuation, last first (`:1230-1246`) |
//! | `x' = e` | binds `x'` if unassigned, otherwise checks equality (`:1353-1378`) |
//! | `x' \in S` | binds each element of `S` in TLC's `elements()` order if unassigned, otherwise checks membership (`:1422-1468`; probes `InSetOrder`, `FilterSetOrder`, `MapSetOrder`); for lazily represented sets TLC's unnormalized `elements()` order, refused where it depends on history (`tlc_enum.rs`) |
//! | `x' \subseteq S` | the same over `SUBSET S` (`:1379-1421`) |
//! | `UNCHANGED e` | a variable, a tuple (items consed), a zero-arity definition or argument (followed), anything else compared (`processUnchanged`, `:1551-1661`) |
//! | `[A]_e` | `A`, then the `UNCHANGED e` successor (`:1284-1295`; probe `StutterInNext`) |
//! | `<A>_e` | `A` with a `CHANGED e` check consed (`:1279-1283`) |
//! | `IF`, `CASE`, `=>` | guard evaluated, then the chosen branch (`:1296-1352`, `:1469-1482`) |
//! | `LET`, labels, `INSTANCE` substitutions, `Op!1` | transparent; substitutions are bound as uncached lazy values (`:960-990`) |
//! | user operator | its body, its arguments bound lazily; an argument naming an action is followed (`LazyValue`, `:1112-1158`) |
//! | `f[a]` | the body of a lazy function, or the function's boolean value (`:1247-1278`) |
//! | anything else | evaluated as a boolean (`:1536-1546`) |
//!
//! Once every variable of the successor is assigned, the rest of the
//! continuation is evaluated as booleans instead (`getNextStatesAllAssigned`,
//! `:1036-1082`), unless warnings are off ([`crate::Options::warnings`]).
//! Initial states follow `getInitStates` (`:427-890`), which has the same
//! rules over unprimed variables, its own error texts and an unconditional
//! all-assigned path.
//!
//! **What it does not do.** It never deduplicates, filters or reorders: the
//! checker counts, constrains and stores (ADR 0015). A successor that leaves a
//! variable unassigned is still emitted ([`Successor::complete`]); TLC reports
//! it in its functor (`TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT`).
//!
//! # Driving it
//!
//! A checker (WP-D2) gets the initial predicates and the split actions from
//! the binding stage ([`Evaluator::init_predicate`], [`Evaluator::actions`],
//! or [`Evaluator::scope`] and its relatives for formulas found inside
//! definitions and instances), generates the initial states, then expands
//! each state with [`Evaluator::successors`]. Every state handed to the sink
//! counts as generated. Run it on a thread with
//! [`crate::RECOMMENDED_STACK`].
//!
//! ```
//! use std::sync::Arc;
//! use dive_eval::next::{Flow, StateVals};
//! use dive_eval::{Evaluator, Model, Options};
//! use dive_ir::actions::{ActionName, split};
//! use dive_sema::{Options as SemaOptions, analyze_source};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\n\
//!            Init == x \\in {0, 1}\nNext == x' = x + 1 \\/ x' = x + 1\n====\n";
//! let ir = dive_ir::lower(&analyze_source("M.tla", src, &SemaOptions::default())).expect("lowers");
//! let ir = Arc::new(ir);
//! let (init, next) = (ir.root_def("Init").expect("Init"), ir.root_def("Next").expect("Next"));
//! let ev = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("valid context");
//! let inits = vec![ev.init_predicate(init, 0).expect("no arguments")];
//! let tree = split(&ir, ir.def(next).body, ActionName::Def(next), next, &Default::default());
//! let actions = ev.actions(&tree, next, inits.len()).expect("splits");
//! assert_eq!(actions.len(), 2);
//!
//! let mut states: Vec<StateVals> = Vec::new();
//! ev.init_states(&inits, &mut |s| {
//!     states.push(s.state.into());
//!     Flow::Continue
//! })
//! .expect("initial states");
//! assert_eq!(states.len(), 2);
//!
//! // Both disjuncts generate x = 1 from x = 0: two successors, one state.
//! let mut generated = 0;
//! ev.successors(&actions, &states[0], &mut |s| {
//!     assert_eq!(s.ordinal, generated);
//!     generated += 1;
//!     Flow::Continue
//! })
//! .expect("successors");
//! assert_eq!(generated, 2);
//! ```

#![allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "each rule ports one TLC method and is kept together for comparison, with the short names of the TLC code"
)]

use std::sync::Arc;

use dive_ir::actions::{ActionName, ActionTree, Split, label};
use dive_ir::{Arg, BuiltinOp, DefId, DefScope, Expr, ExprId, JunctionKind, OpRef, QuantKind};
use dive_value::Value;

use crate::core::{Core, is_enumerable};
use crate::env::{Env, Ev, OpVal, St, StateRef, Sv, Thunk, control};
use crate::error::{Ec, ErrorClass, EvalError, Result};
pub use crate::scope::Scope;
use crate::{ConstBinding, Evaluator};

mod cdot;

/// A state's variable values by [`dive_ir::VarId`], `None` where unassigned.
pub type StateVals = Arc<[Option<Value>]>;

/// Whether to go on enumerating after a successor or initial state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Go on.
    Continue,
    /// Stop now: no further successor is generated or evaluated (TLC's
    /// functor throwing, for example at an invariant violation).
    Stop,
}

/// One initial predicate or next-state action with its context (TLC's
/// `Action`): an expression, the bindings it is evaluated under, its name
/// and its label.
#[derive(Clone, Debug)]
pub struct Action {
    pub(crate) expr: ExprId,
    pub(crate) env: Env,
    owner: Arc<dive_ir::Ir>,
    names: Arc<dive_value::Interner>,
    context: std::sync::Weak<crate::core::Core>,
    name: ActionName,
    params: Vec<(String, Value)>,
    label: String,
    id: usize,
}

impl Action {
    /// The predicate.
    #[must_use]
    pub fn expr(&self) -> ExprId {
        self.expr
    }

    /// The captured evaluation context, for conservative backend preflight.
    /// Reading the context does not evaluate or force any captured value.
    #[must_use]
    pub fn scope(&self) -> Scope {
        Scope {
            env: self.env.clone(),
            owner: self.owner.clone(),
            names: self.names.clone(),
            context: self.context.clone(),
            wrapper: None,
        }
    }

    /// What the action is named after.
    #[must_use]
    pub fn name(&self) -> ActionName {
        self.name
    }

    /// The values of the named definition's parameters bound by splitting,
    /// in parameter order (TLC's `Action.getParameters`).
    #[must_use]
    pub fn params(&self) -> &[(String, Value)] {
        &self.params
    }

    /// TLC's label, `<Name(v1,v2) line L, col C to line L2, col C2 of
    /// module M>` (`Action.getLocation`), as printed in counterexamples.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The action's id: initial predicates first, then the next-state
    /// actions, as TLC numbers them (`Tool.java:225-241`).
    #[must_use]
    pub fn id(&self) -> usize {
        self.id
    }
}

/// A successor or initial state handed to the sink.
#[derive(Debug)]
pub struct Successor<'a> {
    /// The state, by [`dive_ir::VarId`]; `None` where the action left a
    /// variable unassigned.
    pub state: &'a [Option<Value>],
    /// The action (or initial predicate) that produced it.
    pub action: &'a Action,
    /// Its position among the states generated from the same expanded state
    /// (or among the initial states), from 0.
    pub ordinal: u64,
}

impl Successor<'_> {
    /// Whether every variable is assigned (`TLCState.allAssigned`). TLC's
    /// functor reports an incomplete state as
    /// `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT` (or `_INITIAL`).
    #[must_use]
    pub fn complete(&self) -> bool {
        self.state.iter().all(Option::is_some)
    }

    /// The variables left unassigned, by [`dive_ir::VarId`] index.
    #[must_use]
    pub fn unassigned(&self) -> Vec<usize> {
        self.state
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.is_none().then_some(i))
            .collect()
    }
}

/// How an enumeration ended without an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// States handed to the sink.
    pub emitted: u64,
    /// Intermediate states of action compositions that count as generated
    /// without being handed to the sink (TLC's
    /// `nss.incrementStatesGenerated`; [`crate::ActionComposition::Tlc`]
    /// only, 0 otherwise). A checker adds them to its generated count, and
    /// they keep the expanded state from being a deadlock, as in TLC.
    pub intermediate: u64,
    /// Whether the sink stopped the enumeration.
    pub stopped: bool,
}

/// An error that ended an enumeration, with the number of states already
/// handed to the sink (they count as generated, probe `EvalErrorMidLevel`).
#[derive(Clone, Debug)]
pub struct NextError {
    /// The error.
    pub error: EvalError,
    /// States handed to the sink before it.
    pub emitted: u64,
    /// Intermediate states counted as generated before it (see
    /// [`Outcome::intermediate`]).
    pub intermediate: u64,
    /// The action being enumerated (`None` for initial states).
    pub action: Option<usize>,
}

impl std::fmt::Display for NextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (after {} states)", self.error, self.emitted)
    }
}

impl std::error::Error for NextError {}

// -----------------------------------------------------------------------------
// The public entry points
// -----------------------------------------------------------------------------

impl Evaluator {
    /// An initial predicate named by `INIT` (or a level-1 conjunct of a
    /// `SPECIFICATION` that is a use of `def`): `def`'s body in its own frame
    /// (`SpecProcessor.processConfigInitAndNext`).
    ///
    /// # Errors
    /// A bug if `def` has parameters.
    pub fn init_predicate(&self, def: DefId, id: usize) -> Result<Action> {
        let d = self.admit_def(def)?;
        if !d.params.is_empty() {
            return Err(EvalError::bug(format!("{} takes arguments", d.name)));
        }
        self.predicate(def, d.body, ActionName::Def(def), id)
    }

    /// An initial predicate or action that is the expression `expr` of the
    /// top-level definition `frame`, evaluated in a fresh frame of it (for
    /// example the `Init` conjunct of a `SPECIFICATION`, named `Unnamed`).
    ///
    /// # Errors
    /// An unknown ID, a foreign context or an incompatible lexical frame.
    pub fn predicate(
        &self,
        frame: DefId,
        expr: ExprId,
        name: ActionName,
        id: usize,
    ) -> Result<Action> {
        self.predicate_in(&self.scope(frame)?, expr, name, id)
    }

    /// An initial predicate or action `expr` evaluated in `scope`.
    ///
    /// # Errors
    /// An unknown ID, a foreign context or an incompatible lexical frame.
    pub fn predicate_in(
        &self,
        scope: &Scope,
        expr: ExprId,
        name: ActionName,
        id: usize,
    ) -> Result<Action> {
        self.validate_scope(scope)?;
        self.admit_scoped_expr(scope, expr)?;
        if let ActionName::Def(d) = name {
            self.admit_def(d)?;
        }
        Ok(Action {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            expr,
            env: scope.env.clone(),
            name,
            params: Vec::new(),
            label: label(&self.core.ir, name, &[], scope.wrapper.unwrap_or(expr)),
            id,
        })
    }

    /// Splits a next-state relation into TLC's action list (`Tool.getActions`,
    /// `Tool.java:250-419`), expanding the tree `dive_ir::actions::split`
    /// built: each split `\E` is enumerated in the empty state and each
    /// inlined operator's constant arguments are evaluated. `frame` is the
    /// top-level definition the relation is evaluated in; the actions are
    /// numbered from `first_id`.
    ///
    /// # Errors
    /// A refusal or bug while expanding, or TLC's failure to split (an
    /// operator TLC cannot cast at the top of the relation), which is refused.
    pub fn actions(&self, split: &Split, frame: DefId, first_id: usize) -> Result<Vec<Action>> {
        self.actions_in(&self.scope(frame)?, split, first_id)
    }

    /// [`Evaluator::actions`] for a relation found in `scope` (inside a
    /// parameterized definition's body, for example). A relation under a
    /// substitution is not split by TLC: build its one action with
    /// [`Evaluator::predicate_in`] instead.
    ///
    /// # Errors
    /// As [`Evaluator::actions`].
    pub fn actions_in(&self, scope: &Scope, split: &Split, first_id: usize) -> Result<Vec<Action>> {
        self.validate_scope(scope)?;
        self.admit_split(split)?;
        let env = scope.env.clone();
        let mut x = Expander {
            core: &self.core,
            split,
            out: Vec::new(),
            stack: Vec::new(),
            first_id,
        };
        // Leaves, `\E`s and constants carry their own names.
        let r = on_stack(move || x.expand(&split.root, &env).map(|()| x.out));
        match r {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(Split1::Throw(e))) => Err(EvalError::unsupported(format!(
                "a next-state relation TLC cannot split ({})",
                e.message().lines().next().unwrap_or("")
            ))),
            Ok(Err(Split1::Fatal(e))) | Err(e) => Err(e),
        }
    }

    fn admit_split(&self, split: &Split) -> Result<()> {
        let mut work = vec![&split.root];
        for (&def, tree) in &split.defs {
            self.admit_def(def)?;
            work.push(tree);
        }
        let mut remaining = 65_536_usize;
        while let Some(tree) = work.pop() {
            remaining = remaining
                .checked_sub(1)
                .ok_or_else(|| EvalError::resource_limit("action split admission"))?;
            let (expr, name) = match tree {
                ActionTree::Disj { expr, children } => {
                    work.extend(children);
                    (*expr, None)
                }
                ActionTree::Exists { expr, name, body } => {
                    work.push(body);
                    (*expr, Some(*name))
                }
                ActionTree::Scope { expr, body } => {
                    work.push(body);
                    (*expr, None)
                }
                ActionTree::Inline { expr, def, .. } => {
                    self.admit_def(*def)?;
                    (*expr, None)
                }
                ActionTree::Leaf { expr, name } | ActionTree::Configured { expr, name } => {
                    (*expr, Some(*name))
                }
                ActionTree::Throws { expr } => (*expr, None),
            };
            self.admit_expr(expr)?;
            if let Some(ActionName::Def(def)) = name {
                self.admit_def(def)?;
            }
            if work.len() > remaining {
                return Err(EvalError::resource_limit("action split admission"));
            }
        }
        Ok(())
    }

    /// Evaluates an action's predicate as an expression (TLC's
    /// `Tool.eval(pred, con, s0, s1, EvalControl.Clear)`), for a checker's
    /// invariants, constraints and implied actions found by a binding stage.
    ///
    /// # Errors
    /// Any evaluation error, refusal or resource limit.
    pub fn eval_action(&self, action: &Action, st: &St) -> Result<Value> {
        self.validate_scope(&action.scope())?;
        self.validate_state_context(st)?;
        let ev = self.core.eval(&mut action.env.clone(), st, action.expr)?;
        self.core.force_fcn(ev)
    }

    /// [`Evaluator::scope_shares_side_effects`] for an initial predicate or
    /// action.
    #[must_use]
    pub fn action_shares_side_effects(&self, action: &Action) -> bool {
        self.scope_shares_side_effects(&action.scope())
    }

    /// Generates the initial states (`Tool.getInitStates`): every initial
    /// predicate in order, the first one enumerated and the others consed as
    /// its continuation. Each state, duplicates included, goes to `sink`
    /// (TLC's `DoInitFunctor.addElement` counts every one).
    ///
    /// # Errors
    /// The evaluation error, refusal or resource limit that ended the
    /// enumeration, with the number of states emitted before it.
    pub fn init_states(
        &self,
        inits: &[Action],
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        self.init_states_mode(inits, sink, false)
    }

    /// Runs the same initial generator with the diagnostic tool's source hook.
    /// Cached/stored values are not relabelled; only actual fresh constructors
    /// acquire diagnostic sources. The current host owner remains installed.
    ///
    /// # Errors
    /// The actual generation error, refusal or owned interruption.
    pub fn init_states_diagnostic(
        &self,
        inits: &[Action],
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        self.init_states_mode(inits, sink, true)
    }

    fn init_states_mode(
        &self,
        inits: &[Action],
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
        diagnostic: bool,
    ) -> std::result::Result<Outcome, NextError> {
        for action in inits {
            self.validate_scope(&action.scope())
                .map_err(|error| NextError {
                    error,
                    emitted: 0,
                    intermediate: 0,
                    action: None,
                })?;
        }
        let Some(first) = inits.first() else {
            return Ok(Outcome {
                emitted: 0,
                intermediate: 0,
                stopped: false,
            });
        };
        let nvars = self.core.ir.vars().len();
        let mut acts: Acts = None;
        for a in inits[1..].iter().rev() {
            acts = cons(&acts, Kind::Pred, a.expr, &a.env);
        }
        let mut g = Gen {
            core: &self.core,
            init: true,
            diagnostic,
            s0: StateRef::Empty,
            s1: vec![None; nvars].into(),
            action: first,
            ordinal: 0,
            emitted: 0,
            intermediate: 0,
            sink,
            choices: None,
        };
        let r = g.step(first.expr, &acts, &first.env);
        g.finish(r, None)
    }

    /// Generates the successors of `s0` under one action
    /// (`Tool.getNextStates(functor, state, action)`), in TLC's order,
    /// duplicates included. `ordinal` is the number of successors of `s0`
    /// already generated by earlier actions.
    ///
    /// # Errors
    /// The evaluation error, refusal or resource limit that ended the
    /// enumeration, with the number of successors emitted before it.
    pub fn next_states(
        &self,
        action: &Action,
        s0: &StateVals,
        ordinal: u64,
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        self.next_states_mode(action, s0, ordinal, sink, false, None)
    }

    /// Traverses Next's disjunctions and bounded existentials in a seeded
    /// constructive order. The sink stops after its first admitted complete
    /// state; returning [`Flow::Continue`] searches the remaining alternatives.
    /// Init and predicate evaluation, including `ENABLED`, retain their ordinary
    /// semantics. `choose(n)` must return an index below the positive `n`.
    ///
    /// # Errors
    /// The ordinary generation errors, an unsupported indexed domain, or an
    /// out-of-range index returned by the caller.
    pub fn next_states_generation(
        &self,
        action: &Action,
        s0: &StateVals,
        ordinal: u64,
        choose: &mut dyn FnMut(u64) -> u64,
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        self.next_states_mode(action, s0, ordinal, sink, false, Some(choose))
    }

    /// Generates Next with the diagnostic tool's source hook. Only receivers
    /// actually constructed during this execution acquire diagnostic sources;
    /// cached or stored values retain their original lifetime and provenance.
    /// The current host owner remains installed.
    ///
    /// ```no_run
    /// # fn replay(ev: &dive_eval::Evaluator, action: &dive_eval::next::Action, source: &dive_eval::next::StateVals) {
    /// let result = ev.next_states_diagnostic(action, source, 0, &mut |_| {
    ///     dive_eval::next::Flow::Continue
    /// });
    /// // Inspect the actual error's typed receiver source, never its text.
    /// # let _ = result;
    /// # }
    /// ```
    ///
    /// # Errors
    /// The actual generation error, refusal or owned interruption.
    pub fn next_states_diagnostic(
        &self,
        action: &Action,
        s0: &StateVals,
        ordinal: u64,
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        self.next_states_mode(action, s0, ordinal, sink, true, None)
    }

    fn next_states_mode(
        &self,
        action: &Action,
        s0: &StateVals,
        ordinal: u64,
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
        diagnostic: bool,
        choices: Option<&mut dyn FnMut(u64) -> u64>,
    ) -> std::result::Result<Outcome, NextError> {
        self.validate_scope(&action.scope())
            .map_err(|error| NextError {
                error,
                emitted: 0,
                intermediate: 0,
                action: Some(action.id),
            })?;
        let mut g = Gen {
            core: &self.core,
            init: false,
            diagnostic,
            s0: StateRef::Vals(s0.clone()),
            s1: vec![None; s0.len()].into(),
            action,
            ordinal,
            emitted: 0,
            intermediate: 0,
            sink,
            choices,
        };
        let r = g.step(action.expr, &None, &action.env);
        g.finish(r, Some(action.id))
    }

    /// Generates the successors of `s0` under every action in order
    /// (`Tool.getNextStates(functor, state)`).
    ///
    /// # Errors
    /// As [`Evaluator::next_states`]; `emitted` counts all actions' states.
    pub fn successors(
        &self,
        actions: &[Action],
        s0: &StateVals,
        sink: &mut dyn FnMut(&Successor<'_>) -> Flow,
    ) -> std::result::Result<Outcome, NextError> {
        let mut total = 0u64;
        let mut intermediate = 0u64;
        for a in actions {
            match self.next_states(a, s0, total, sink) {
                Ok(o) => {
                    total += o.emitted;
                    intermediate += o.intermediate;
                    if o.stopped {
                        return Ok(Outcome {
                            emitted: total,
                            intermediate,
                            stopped: true,
                        });
                    }
                }
                Err(mut e) => {
                    e.emitted += total;
                    e.intermediate += intermediate;
                    return Err(e);
                }
            }
        }
        Ok(Outcome {
            emitted: total,
            intermediate,
            stopped: false,
        })
    }
}

/// Runs `f` on a thread with [`crate::RECOMMENDED_STACK`] (splitting can
/// evaluate deeply recursive operators).
fn on_stack<T: Send>(f: impl FnOnce() -> T + Send) -> Result<T> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(crate::RECOMMENDED_STACK)
            .spawn_scoped(s, f)
            .map_err(|e| EvalError::resource_limit(format!("cannot start a thread: {e}")))?
            .join()
            .map_err(|_| EvalError::bug("action expansion panicked"))
    })
}

// -----------------------------------------------------------------------------
// The continuation list
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `IActionItemList.PRED` and conjuncts.
    Pred,
    /// `IActionItemList.UNCHANGED`.
    Unchanged,
    /// `IActionItemList.CHANGED`.
    Changed,
}

#[derive(Debug)]
struct Item {
    kind: Kind,
    expr: ExprId,
    env: Env,
    next: Acts,
}

/// TLC's `ActionItemList`: a persistent list.
type Acts = Option<Arc<Item>>;

#[allow(
    clippy::unnecessary_wraps,
    reason = "a cons cell is a non-empty list, used where lists are expected"
)]
fn cons(acts: &Acts, kind: Kind, expr: ExprId, env: &Env) -> Acts {
    Some(Arc::new(Item {
        kind,
        expr,
        env: env.clone(),
        next: acts.clone(),
    }))
}

// -----------------------------------------------------------------------------
// The generator
// -----------------------------------------------------------------------------

/// `getInitStates` (with `init`) or `getNextStates` over one action.
struct Gen<'a, 's, 'c> {
    core: &'a Arc<Core>,
    init: bool,
    diagnostic: bool,
    /// The current state (`Empty` for initial states).
    s0: StateRef,
    /// The successor being built (`ps`, the initial state, with `init`).
    s1: StateVals,
    action: &'a Action,
    ordinal: u64,
    emitted: u64,
    /// Intermediate states of `\cdot` counted as generated (`cdot.rs`).
    intermediate: u64,
    sink: &'s mut dyn FnMut(&Successor<'_>) -> Flow,
    /// Only this actual Next generator may draw; ordinary evaluator calls
    /// and separately constructed ENABLED generators never inherit it.
    choices: Option<&'c mut dyn FnMut(u64) -> u64>,
}

impl Gen<'_, '_, '_> {
    fn finish(
        &self,
        r: Result<Flow>,
        action: Option<usize>,
    ) -> std::result::Result<Outcome, NextError> {
        match r {
            Ok(flow) => Ok(Outcome {
                emitted: self.emitted,
                intermediate: self.intermediate,
                stopped: flow == Flow::Stop,
            }),
            Err(error) => Err(NextError {
                error,
                emitted: self.emitted,
                intermediate: self.intermediate,
                action,
            }),
        }
    }

    /// The states and control TLC evaluates expressions under.
    fn st(&self) -> St {
        if self.init {
            // `eval(e, c, ps, TLCState.Empty, EvalControl.Init)`
            St {
                s0: StateRef::Vals(self.s1.clone()),
                s1: StateRef::Empty,
                control: control::INIT
                    | if self.diagnostic {
                        control::DIAGNOSTIC
                    } else {
                        0
                    },
            }
        } else {
            St {
                s0: self.s0.clone(),
                s1: StateRef::Vals(self.s1.clone()),
                control: if self.diagnostic {
                    control::DIAGNOSTIC
                } else {
                    control::CLEAR
                },
            }
        }
    }

    /// The states a `LazyValue`'s cache is consulted under
    /// (`getCachedValue(this, ps, null, Clear)` for initial states).
    fn st_cache(&self) -> St {
        if self.init {
            St {
                s0: StateRef::Vals(self.s1.clone()),
                s1: StateRef::Null,
                control: control::CLEAR,
            }
        } else {
            self.st()
        }
    }

    fn what(&self) -> &'static str {
        if self.init {
            "initial states"
        } else {
            "next states"
        }
    }

    fn bind(&mut self, var: usize, v: Value) {
        if let Some(slot) = Arc::make_mut(&mut self.s1).get_mut(var) {
            *slot = Some(v);
        }
    }

    fn unbind(&mut self, var: usize) {
        if let Some(slot) = Arc::make_mut(&mut self.s1).get_mut(var) {
            *slot = None;
        }
    }

    fn all_assigned(&self) -> bool {
        self.s1.iter().all(Option::is_some)
    }

    fn emit(&mut self) -> Flow {
        let succ = Successor {
            state: &self.s1,
            action: self.action,
            ordinal: self.ordinal,
        };
        let flow = (self.sink)(&succ);
        self.ordinal += 1;
        self.emitted += 1;
        flow
    }

    fn eval(&self, env: &Env, e: ExprId) -> Result<Ev> {
        self.core.eval(&mut env.clone(), &self.st(), e)
    }

    fn eval_v(&self, env: &Env, e: ExprId) -> Result<Value> {
        self.core.eval_v(&mut env.clone(), &self.st(), e)
    }

    /// `getNextStates(action, acts, s0, s1, ...)` / `getInitStates(acts, ps,
    /// ...)`: the continuation.
    fn cont(&mut self, acts: &Acts) -> Result<Flow> {
        let Some(item) = acts else {
            return Ok(self.emit());
        };
        if self.init {
            if self.all_assigned() {
                return self.init_all_assigned(acts);
            }
            return self.step(item.expr, &item.next, &item.env);
        }
        if self.core.opts.warnings && self.all_assigned() {
            return self.next_all_assigned(acts);
        }
        match item.kind {
            Kind::Pred => self.step(item.expr, &item.next, &item.env),
            Kind::Unchanged => self.unchanged(item.expr, &item.next, &item.env),
            Kind::Changed => {
                if self.changed(item)? {
                    self.cont(&item.next)
                } else {
                    Ok(Flow::Continue)
                }
            }
        }
    }

    /// The `CHANGED e` item of `<A>_e`: `e` differs between the states.
    fn changed(&self, item: &Item) -> Result<bool> {
        let s0 = St {
            s0: self.s0.clone(),
            s1: StateRef::Empty,
            control: control::CLEAR,
        };
        let s1 = St {
            s0: StateRef::Vals(self.s1.clone()),
            s1: StateRef::Empty,
            control: control::CLEAR,
        };
        let v1 = self.core.eval_v(&mut item.env.clone(), &s0, item.expr)?;
        let v2 = self.core.eval_v(&mut item.env.clone(), &s1, item.expr)?;
        Ok(!self.core.equals(&v1, &v2)?)
    }

    /// `getInitStates(acts, ps, ...)` once every variable is assigned
    /// (`Tool.java:540-572`).
    fn init_all_assigned(&mut self, acts: &Acts) -> Result<Flow> {
        let mut cur = acts.clone();
        while let Some(item) = cur.take() {
            let v = self.eval(&item.env, item.expr)?;
            match v {
                Ev::V(Value::Bool(true)) => {}
                Ev::V(Value::Bool(false)) => {
                    return Ok(Flow::Continue);
                }
                other => return Err(self.expected_bool(&other, item.expr)),
            }
            cur.clone_from(&item.next);
        }
        Ok(self.emit())
    }

    /// `getNextStatesAllAssigned` (`Tool.java:1047-1082`).
    fn next_all_assigned(&mut self, acts: &Acts) -> Result<Flow> {
        let mut cur = acts.clone();
        while let Some(item) = cur.take() {
            match item.kind {
                Kind::Pred => {
                    let v = self.eval(&item.env, item.expr)?;
                    match v {
                        Ev::V(Value::Bool(true)) => {}
                        Ev::V(Value::Bool(false)) => return Ok(Flow::Continue),
                        other => return Err(self.expected_bool(&other, item.expr)),
                    }
                }
                Kind::Unchanged => return self.unchanged(item.expr, &item.next, &item.env),
                Kind::Changed => {
                    if !self.changed(&item)? {
                        return Ok(Flow::Continue);
                    }
                }
            }
            cur.clone_from(&item.next);
        }
        Ok(self.emit())
    }

    /// `TLC_EXPECTED_EXPRESSION_IN_COMPUTING` for a non-boolean.
    fn expected_bool(&self, v: &Ev, at: ExprId) -> EvalError {
        EvalError::coded(
            Ec::ExpectedExpressionInComputing,
            format!(
                "In computing {}, TLC expected a boolean expression,\nbut instead found {}.\n{}",
                self.what(),
                self.core.ev_string(v),
                self.core.loc(at)
            ),
        )
    }

    /// Continues after a predicate that evaluated to `v`.
    fn after_bool(&mut self, v: &Ev, acts: &Acts, at: ExprId) -> Result<Flow> {
        match v {
            Ev::V(Value::Bool(true)) => self.cont(acts),
            Ev::V(Value::Bool(false)) => Ok(Flow::Continue),
            other => Err(self.expected_bool(other, at)),
        }
    }

    /// The default rule: evaluate `e` as a boolean.
    fn default_rule(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        let v = self.eval(env, e)?;
        match v {
            Ev::V(Value::Bool(true)) => self.cont(acts),
            Ev::V(Value::Bool(false)) => Ok(Flow::Continue),
            other if self.init => Err(EvalError::general(format!(
                "In computing initial states, TLC expected a boolean expression,\nbut instead found {}.\n{}",
                self.core.ev_string(&other),
                self.core.loc(e)
            ))),
            _ if matches!(self.core.ir.expr(e), Expr::Num(_) | Expr::Str(_)) => {
                Err(EvalError::general(format!(
                    "The next state relation is not a boolean expression.\n{}",
                    self.core.loc(e)
                )))
            }
            other => Err(self.expected_bool(&other, e)),
        }
    }

    /// `getNextStates(action, pred, acts, c, ...)` / `getInitStates(init,
    /// acts, c, ...)`.
    fn step(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        self.step_inner(e, acts, env).map_err(|error| error.at(e))
    }

    fn step_inner(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        let core = self.core;
        let _g = core.enter(e)?;
        let node = core.ir.node(e);
        match &node.expr {
            Expr::Junction { kind, items, .. } => match kind {
                JunctionKind::And => {
                    // OPCODE_cl / OPCODE_land
                    let mut acts1 = acts.clone();
                    for &i in items.iter().skip(1).rev() {
                        acts1 = cons(&acts1, Kind::Pred, i, env);
                    }
                    match items.first() {
                        Some(&first) => self.step(first, &acts1, env),
                        None => self.cont(acts),
                    }
                }
                JunctionKind::Or => {
                    // OPCODE_dl / OPCODE_lor
                    if let Some(choose) = self.choices.as_deref_mut() {
                        let len = u64::try_from(items.len()).map_err(|_| {
                            EvalError::resource_limit("generation disjunction size")
                        })?;
                        let mut order = crate::generation::IndexOrder::new(len, choose)?;
                        while let Some(index) = order.current() {
                            let index = usize::try_from(index).map_err(|_| {
                                EvalError::resource_limit("generation branch index")
                            })?;
                            if self.step(items[index], acts, env)? == Flow::Stop {
                                return Ok(Flow::Stop);
                            }
                            if !order.advance() {
                                break;
                            }
                        }
                        return Ok(Flow::Continue);
                    }
                    for &i in items {
                        if self.step(i, acts, env)? == Flow::Stop {
                            return Ok(Flow::Stop);
                        }
                    }
                    Ok(Flow::Continue)
                }
            },
            Expr::Quant {
                kind: QuantKind::Exists,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                // OPCODE_be
                let st = self.st();
                if let Some(choose) = self.choices.as_deref_mut() {
                    let mut inner = env.clone();
                    let mut contexts =
                        core.generation_contexts(&mut inner, &st, bounds, e, choose)?;
                    while contexts.bind(core, &mut inner)? {
                        if self.step(*body, acts, &inner)? == Flow::Stop {
                            return Ok(Flow::Stop);
                        }
                        contexts.advance();
                    }
                    return Ok(Flow::Continue);
                }
                let mut flow = Flow::Continue;
                let mut err: Option<EvalError> = None;
                let body = *body;
                core.for_each_context(&mut env.clone(), &st, bounds, e, &mut |_, env1| match self
                    .step(body, acts, env1)
                {
                    Ok(Flow::Continue) => Ok(true),
                    Ok(Flow::Stop) => {
                        flow = Flow::Stop;
                        Ok(false)
                    }
                    Err(x) => {
                        err = Some(x);
                        Ok(false)
                    }
                })?;
                match err {
                    Some(x) => Err(x),
                    None => Ok(flow),
                }
            }
            Expr::Quant {
                kind: QuantKind::Forall,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                // OPCODE_bf
                let st = self.st();
                let mut envs: Vec<Env> = Vec::new();
                core.for_each_context(&mut env.clone(), &st, bounds, e, &mut |_, env1| {
                    envs.push(env1.clone());
                    Ok(true)
                })?;
                let Some((first, rest)) = envs.split_first() else {
                    return self.cont(acts);
                };
                let mut acts1 = acts.clone();
                for c2 in rest {
                    acts1 = cons(&acts1, Kind::Pred, *body, c2);
                }
                self.step(*body, &acts1, first)
            }
            Expr::If { cond, then, other } => {
                // OPCODE_ite
                let g = self.eval(env, *cond)?;
                let b = match g {
                    Ev::V(Value::Bool(b)) => b,
                    other => {
                        let kind = core.kind_of(&other);
                        return Err(EvalError::general(if self.init {
                            format!(
                                "In computing initial states, a non-boolean expression ({kind}) was used as the condition of an IF.\n{}",
                                core.loc(e)
                            )
                        } else {
                            format!(
                                "In computing next states, a non-boolean expression ({kind}) was used as the condition of an IF.{}",
                                core.loc(e)
                            )
                        }));
                    }
                };
                self.step(if b { *then } else { *other }, acts, env)
            }
            Expr::Case { arms, other } => {
                if self.choices.is_some() {
                    return Err(EvalError::unsupported(
                        "constructive generation of CASE actions",
                    ));
                }
                // OPCODE_case
                for arm in arms {
                    match self.eval(env, arm.guard)? {
                        Ev::V(Value::Bool(true)) => return self.step(arm.value, acts, env),
                        Ev::V(Value::Bool(false)) => {}
                        other => {
                            return Err(EvalError::general(format!(
                                "In computing {}, a non-boolean expression ({}) was used as a guard condition of a CASE.\n{}",
                                self.what(),
                                core.kind_of(&other),
                                core.loc(arm.value)
                            )));
                        }
                    }
                }
                match other {
                    Some(o) => self.step(*o, acts, env),
                    None => Err(EvalError::general(format!(
                        "In computing {}, TLC encountered a CASE with no conditions true.\n{}",
                        self.what(),
                        core.loc(e)
                    ))),
                }
            }
            Expr::Implies(a, b) => {
                // OPCODE_implies
                match self.eval(env, *a)? {
                    Ev::V(Value::Bool(true)) => self.step(*b, acts, env),
                    Ev::V(Value::Bool(false)) => self.cont(acts),
                    other => {
                        let kind = core.kind_of(&other);
                        Err(EvalError::general(if self.init {
                            format!(
                                "In computing initial states of a predicate of form P => Q, P was {kind}\n.{}",
                                core.loc(e)
                            )
                        } else {
                            format!(
                                "In computing next states of a predicate of the form P => Q, P was\n{kind}.\n{}",
                                core.loc(e)
                            )
                        }))
                    }
                }
            }
            Expr::Builtin {
                op: op @ (BuiltinOp::Eq | BuiltinOp::In | BuiltinOp::Subseteq),
                args,
            } if args.len() == 2 => self.assign(*op, args[0], args[1], e, acts, env),
            Expr::Unchanged(a) if !self.init => self.unchanged(*a, acts, env),
            Expr::Cdot(a, b) if !self.init => self.cdot(*a, *b, acts, env, e),
            Expr::StepBox { action, sub } if !self.init => {
                // OPCODE_sa
                if self.step(*action, acts, env)? == Flow::Stop {
                    return Ok(Flow::Stop);
                }
                self.unchanged(*sub, acts, env)
            }
            Expr::StepAngle { action, sub } if !self.init => {
                // OPCODE_aa
                let acts1 = cons(acts, Kind::Changed, *sub, env);
                self.step(*action, &acts1, env)
            }
            Expr::Let { defs, body } => {
                // LetInKind: the definitions are not bound; a use finds the
                // definition and evaluates its body afresh each time.
                let env1 = core.bind_let_uncached(env, defs);
                self.step(*body, acts, &env1)
            }
            Expr::Subst { bindings, body } => {
                // SubstInKind: uncached lazy values over the outer context.
                let env1 = core.bind_subst(env, &self.st(), bindings, false)?;
                self.step(*body, acts, &env1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.step(*body, acts, env),
            Expr::FcnApply { fcn, arg } => self.fcn_apply(*fcn, *arg, e, acts, env),
            Expr::Def(_)
            | Expr::Apply { .. }
            | Expr::Local(_)
            | Expr::Const(_)
            | Expr::SubstRef { primed: false, .. } => self.user_op(e, acts, env),
            _ => self.default_rule(e, acts, env),
        }
    }

    /// `x' = e`, `x' \in S`, `x' \subseteq S` (`x = e`, ... in an initial
    /// predicate).
    fn assign(
        &mut self,
        op: BuiltinOp,
        lhs: ExprId,
        rhs: ExprId,
        e: ExprId,
        acts: &Acts,
        env: &Env,
    ) -> Result<Flow> {
        let core = self.core;
        let var = if self.init {
            core.var_of(env, lhs, false)?
        } else {
            core.primed_var(env, lhs, false)?
        };
        let Some(var) = var else {
            let v = self.eval(env, e)?;
            return match v {
                Ev::V(Value::Bool(true)) => self.cont(acts),
                Ev::V(Value::Bool(false)) => Ok(Flow::Continue),
                _ => Err(EvalError::bug("=, \\in or \\subseteq not boolean")),
            };
        };
        let lval = self.s1.get(var).cloned().flatten();
        match op {
            BuiltinOp::Eq => {
                let rval = core.embed(self.eval(env, rhs)?)?;
                match lval {
                    None => {
                        self.bind(var, rval);
                        let flow = self.cont(acts);
                        self.unbind(var);
                        flow
                    }
                    Some(l) => {
                        if core.equals(&l, &rval)? {
                            self.cont(acts)
                        } else {
                            Ok(Flow::Continue)
                        }
                    }
                }
            }
            BuiltinOp::In => {
                let rval = self.eval_v(env, rhs)?;
                match lval {
                    None => {
                        let Value::Set(s) = &rval else {
                            return Err(self.not_enumerable_in(e));
                        };
                        if !is_enumerable(&rval) {
                            return Err(self.not_enumerable_in(e));
                        }
                        if let Some((lo, hi)) = s.as_interval() {
                            // IntervalValue.elements() walks ascending integers;
                            // no normalized copy or history-sensitive lazy-set
                            // decision is needed. Reach the sink before building
                            // an entire domain so its stop can interrupt Init
                            // and successor enumeration (ADR 0107, issue #349).
                            return self.bind_interval(var, lo, hi, acts);
                        }
                        if let Some(elements) = core.in_element_stream(s)? {
                            for value in elements.with_owner(rval) {
                                let value = value?;
                                if self.bind_value(var, value, acts)? == Flow::Stop {
                                    return Ok(Flow::Stop);
                                }
                            }
                            return Ok(Flow::Continue);
                        }
                        let elems = core.in_elements(s)?;
                        self.bind_each(var, &elems, acts)
                    }
                    Some(l) => {
                        if core.member(&l, &rval)? {
                            self.cont(acts)
                        } else {
                            Ok(Flow::Continue)
                        }
                    }
                }
            }
            _ => {
                let rset = self.eval_v(env, rhs)?;
                if !matches!(rset, Value::Set(_)) {
                    return Err(EvalError::unsupported(format!(
                        "x' \\subseteq S where S is not a set ({})",
                        core.loc(e)
                    )));
                }
                let rval = Value::subset(rset, core.ctx);
                match lval {
                    None => {
                        let Value::Set(s) = &rval else {
                            return Err(EvalError::bug("SUBSET is not a set"));
                        };
                        let elems = core.explore(s)?;
                        self.bind_each(var, &elems, acts)
                    }
                    Some(l) => {
                        if core.member(&l, &rval)? {
                            self.cont(acts)
                        } else {
                            Ok(Flow::Continue)
                        }
                    }
                }
            }
        }
    }

    fn not_enumerable_in(&self, e: ExprId) -> EvalError {
        EvalError::general(format!(
            "In computing {}, the right side of \\IN is not enumerable.\n{}",
            self.what(),
            self.core.loc(e)
        ))
    }

    fn bind_each(&mut self, var: usize, elems: &[Value], acts: &Acts) -> Result<Flow> {
        for x in elems {
            if self.bind_value(var, x.clone(), acts)? == Flow::Stop {
                return Ok(Flow::Stop);
            }
        }
        Ok(Flow::Continue)
    }

    fn bind_value(&mut self, var: usize, value: Value, acts: &Acts) -> Result<Flow> {
        // A candidate is real work even when its continuation rejects it and
        // no successor reaches the sink, including private composition sinks.
        self.core.loop_work()?;
        self.bind(var, value);
        let flow = self.cont(acts);
        self.unbind(var);
        flow
    }

    fn bind_interval(
        &mut self,
        var: usize,
        lo: &dive_value::Int,
        hi: &dive_value::Int,
        acts: &Acts,
    ) -> Result<Flow> {
        if let (Some(lo), Some(hi)) = (lo.to_i64(), hi.to_i64()) {
            for value in lo..=hi {
                if self.bind_value(var, Value::Int(value), acts)? == Flow::Stop {
                    return Ok(Flow::Stop);
                }
            }
        } else {
            let mut value = lo.clone();
            while value <= *hi {
                if self.bind_value(var, Value::int(value.clone()), acts)? == Flow::Stop {
                    return Ok(Flow::Stop);
                }
                if value == *hi {
                    break;
                }
                value = value.add(&dive_value::Int::ONE, dive_value::IntMode::Big)?;
            }
        }
        Ok(Flow::Continue)
    }

    /// `processUnchanged` (`Tool.java:1557-1661`).
    fn unchanged(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        let core = self.core;
        let _g = core.enter(e)?;
        if let Some(var) = core.var_of(env, e, false)? {
            let name = &core.ir.decl(core.ir.vars()[var]).name;
            let Some(val0) = core.state_variable(&self.s0, name, Some(var))?.cloned() else {
                return Err(EvalError::bug(
                    "UNCHANGED of a variable the current state lacks",
                ));
            };
            return match self.s1.get(var).cloned().flatten() {
                None => {
                    self.bind(var, val0);
                    let flow = self.cont(acts);
                    self.unbind(var);
                    flow
                }
                Some(val1) => {
                    if core.equals(&val0, &val1)? {
                        self.cont(acts)
                    } else {
                        core.unchanged_warning(var, e);
                        Ok(Flow::Continue)
                    }
                }
            };
        }
        match &core.ir.node(e).expr {
            Expr::Tuple(items) => {
                let Some((&first, rest)) = items.split_first() else {
                    return self.cont(acts);
                };
                let mut acts1 = acts.clone();
                for &i in rest.iter().rev() {
                    acts1 = cons(&acts1, Kind::Unchanged, i, env);
                }
                self.unchanged(first, &acts1, env)
            }
            _ => match core.zero_arity_target(env, e)? {
                Target::Body(env1, body) => self.unchanged(body, acts, &env1),
                Target::Value => self.verify_unchanged(e, acts, env),
            },
        }
    }

    /// `verifyUnchanged`: `e` has the same value in both states.
    fn verify_unchanged(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        let st0 = St {
            s0: self.s0.clone(),
            s1: StateRef::Empty,
            control: control::CLEAR,
        };
        let st1 = St {
            s0: StateRef::Vals(self.s1.clone()),
            s1: StateRef::Null,
            control: control::CLEAR,
        };
        let v0 = self.core.eval_v(&mut env.clone(), &st0, e)?;
        let v1 = self.core.eval_v(&mut env.clone(), &st1, e)?;
        if self.core.equals(&v0, &v1)? {
            self.cont(acts)
        } else {
            Ok(Flow::Continue)
        }
    }

    /// `OPCODE_fa` in an action.
    fn fcn_apply(
        &mut self,
        fcn: ExprId,
        arg: ExprId,
        e: ExprId,
        acts: &Acts,
        env: &Env,
    ) -> Result<Flow> {
        let core = self.core;
        let st = if self.init {
            self.st()
        } else {
            self.st().keep_lazy()
        };
        let f = core.eval(&mut env.clone(), &st, fcn)?;
        let fv = match f {
            Ev::F(lf) => {
                if let Some(v) = lf.materialized.get(core.host.cache_domain()) {
                    v
                } else {
                    // `getFcnContext`: the lambda's body, its parameters bound.
                    let a = self.eval_v(env, arg)?;
                    let env1 = core.fcn_context(&lf, &a, e)?;
                    return self.step(lf.body, acts, &env1);
                }
            }
            Ev::V(v) => v,
        };
        let Value::Fcn(fv) = fv else {
            return Err(EvalError::general(format!(
                "In computing {}, a non-function ({}) was applied as a function.\n{}",
                self.what(),
                crate::core::kind_string(&fv),
                core.loc(e)
            )));
        };
        let a = self.eval_v(env, arg)?;
        match core.apply_fcn(&fv, &a)? {
            Value::Bool(true) => self.cont(acts),
            Value::Bool(false) => Ok(Flow::Continue),
            _ => Err(EvalError::coded(
                Ec::ExpectedExpressionInComputing2,
                format!(
                    "In computing {}, TLC expected a boolean expression,\nbut didn't find one.\n{}",
                    self.what(),
                    core.loc(e)
                ),
            )),
        }
    }

    /// `opcode == 0`: a user-defined operator, an argument or `LET` value, a
    /// constant or a Java override, looked up and followed or evaluated
    /// (`Tool.java:1112-1158` and `getNextStatesApplUsrDefOp`).
    fn user_op(&mut self, e: ExprId, acts: &Acts, env: &Env) -> Result<Flow> {
        let core = self.core;
        let st = self.st();
        match core.op_target(env, &st, e)? {
            OpTarget::Body(env1, body) => self.step(body, acts, &env1),
            OpTarget::Thunk(t) => {
                if let Some(v) = t.cached(&self.st_cache(), core.host.cache_domain()) {
                    return self.user_bool(&v, acts, e);
                }
                self.step(t.expr, acts, &t.env)
            }
            OpTarget::Value(v) => self.user_bool(&v, acts, e),
        }
    }

    fn user_bool(&mut self, v: &Ev, acts: &Acts, e: ExprId) -> Result<Flow> {
        self.after_bool(v, acts, e)
    }
}

// -----------------------------------------------------------------------------
// Lookups shared by the generator and ENABLED
// -----------------------------------------------------------------------------

/// What a zero-arity name in `UNCHANGED` leads to.
pub(crate) enum Target {
    /// Follow the expression in this environment.
    Body(Env, ExprId),
    /// Compare its value.
    Value,
}

/// What an operator application in an action leads to.
pub(crate) enum OpTarget {
    /// A definition's body, with its parameters bound.
    Body(Env, ExprId),
    /// A `LazyValue` (an argument, `LET` value or substitution).
    Thunk(Arc<Thunk>),
    /// A value: a constant, a Java override's result, a bound variable.
    Value(Ev),
}

impl Core {
    /// `Tool.lookup` for an operator in an action, and the step TLC takes
    /// with the result: a definition is followed, a `LazyValue` is consulted,
    /// anything else is evaluated (`OpValue.eval`, with the control the
    /// generator uses).
    pub(crate) fn op_target(self: &Arc<Self>, env: &Env, st: &St, e: ExprId) -> Result<OpTarget> {
        let node = self.ir.node(e);
        let mut env = env.clone();
        match &node.expr {
            Expr::Def(d) => self.def_target(&mut env, st, *d, &[], e),
            Expr::Apply { callee, args } => match callee {
                OpRef::Def(d) => self.def_target(&mut env, st, *d, args, e),
                OpRef::Std(op) => Ok(OpTarget::Value(self.eval_std(&mut env, st, *op, args, e)?)),
                _ => {
                    let op = self.op_value(&env, callee)?;
                    self.opval_target(&mut env, st, &op, args, e)
                }
            },
            Expr::Local(lr) => match self.slot(&env, *lr)?.clone() {
                Sv::Thunk(t) => Ok(OpTarget::Thunk(t)),
                Sv::Op(op) => self.opval_target(&mut env, st, &op, &[], e),
                Sv::Unset => Err(EvalError::bug("read of an unbound slot")),
                sv => Ok(OpTarget::Value(self.force(&mut env, &sv, st, e)?)),
            },
            Expr::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(d)) => self.def_target(&mut env, st, *d, &[], e),
                _ => Ok(OpTarget::Value(self.eval(&mut env, st, e)?)),
            },
            Expr::SubstRef {
                decl,
                primed: false,
            } => match env.dynamic.lookup(*decl).cloned() {
                Some(Sv::Thunk(t)) => Ok(OpTarget::Thunk(t)),
                Some(Sv::Op(op)) => self.opval_target(&mut env, st, &op, &[], e),
                Some(_) => Ok(OpTarget::Value(self.eval(&mut env, st, e)?)),
                None => {
                    let bound = self
                        .ir
                        .decl(*decl)
                        .constant
                        .and_then(|c| self.consts.get(c.index()).and_then(Option::as_ref));
                    match bound {
                        Some(ConstBinding::Def(d)) => self.def_target(&mut env, st, *d, &[], e),
                        _ => Ok(OpTarget::Value(self.eval(&mut env, st, e)?)),
                    }
                }
            },
            _ => Err(EvalError::bug("op_target on a non-operator")),
        }
    }

    fn def_target(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        d: DefId,
        args: &[Arg],
        e: ExprId,
    ) -> Result<OpTarget> {
        let d = self.redirect(d);
        if let Some(v) = self.def_values.get(&d) {
            return Ok(OpTarget::Value(Ev::V(v.clone())));
        }
        let def = self.ir.def(d);
        if let Some(op) = def.std {
            return Ok(OpTarget::Value(self.eval_std(env, st, op, args, e)?));
        }
        if args.is_empty() && self.cacheable(d) {
            // TLC precomputed it at start-up (`processConstantDefns`) and
            // looks the value up; a failure there left the definition lazy.
            if let Some(v) = self.def_cache[d.index()].get() {
                return Ok(OpTarget::Value(Ev::V(v.clone())));
            }
            let mut fresh = Env::new(
                d,
                def.depth,
                def.frame.len(),
                None,
                crate::env::Dyn::default(),
            );
            match self.eval(&mut fresh, &St::constant(), def.body) {
                Ok(Ev::V(v)) => {
                    let _ = self.def_cache[d.index()].set(v.clone());
                    return Ok(OpTarget::Value(Ev::V(v)));
                }
                Ok(ev) => return Ok(OpTarget::Value(ev)),
                Err(err) if err.class() == ErrorClass::Eval => {}
                Err(err) => return Err(err),
            }
        }
        let env1 = self.callee_env(env, st, d, None, args)?;
        Ok(OpTarget::Body(env1, def.body))
    }

    fn opval_target(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: &OpVal,
        args: &[Arg],
        e: ExprId,
    ) -> Result<OpTarget> {
        match op {
            OpVal::Def {
                def,
                parent,
                dynamic,
            } => {
                if self.ir.def(*def).std.is_some() {
                    return Ok(OpTarget::Value(self.apply_op(op, env, st, args, e)?));
                }
                // As `apply_op`: an operator passed with its context is
                // applied in that context.
                let mut caller = env.clone();
                if let Some(d) = dynamic {
                    caller.dynamic = d.clone();
                }
                let env1 = self.callee_env(&mut caller, st, *def, parent.clone(), args)?;
                Ok(OpTarget::Body(env1, self.ir.def(*def).body))
            }
            OpVal::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(d)) => self.def_target(env, st, *d, args, e),
                _ => Ok(OpTarget::Value(self.apply_op(op, env, st, args, e)?)),
            },
            OpVal::Builtin(b) => match b {
                BuiltinOp::Eq
                | BuiltinOp::In
                | BuiltinOp::Subseteq
                | BuiltinOp::And
                | BuiltinOp::Or
                | BuiltinOp::Implies
                | BuiltinOp::Unchanged
                | BuiltinOp::Prime
                | BuiltinOp::Enabled
                | BuiltinOp::Cdot => Err(EvalError::unsupported(format!(
                    "the built-in operator {b:?} passed as an operator argument and applied in an action ({})",
                    self.loc(e)
                ))),
                _ => Ok(OpTarget::Value(self.apply_op(op, env, st, args, e)?)),
            },
            OpVal::Std(_) | OpVal::Unbound(_) => {
                Ok(OpTarget::Value(self.apply_op(op, env, st, args, e)?))
            }
        }
    }

    /// For `UNCHANGED` and `ENABLED`'s `UNCHANGED`: a zero-arity operator is
    /// followed when its lookup is a definition or a `LazyValue`
    /// (`processUnchangedImpl0Arity`).
    pub(crate) fn zero_arity_target(self: &Arc<Self>, env: &Env, e: ExprId) -> Result<Target> {
        match &self.ir.node(e).expr {
            Expr::Def(d) => Ok(self
                .def_follow(env, *d)
                .map_or(Target::Value, |(env1, body)| Target::Body(env1, body))),
            Expr::Local(lr) => match self.slot(env, *lr)? {
                Sv::Thunk(t) => Ok(Target::Body(t.env.clone(), t.expr)),
                _ => Ok(Target::Value),
            },
            Expr::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(d)) => Ok(self
                    .def_follow(env, *d)
                    .map_or(Target::Value, |(env1, body)| Target::Body(env1, body))),
                _ => Ok(Target::Value),
            },
            Expr::SubstRef {
                decl,
                primed: false,
            } => match env.dynamic.lookup(*decl) {
                Some(Sv::Thunk(t)) => Ok(Target::Body(t.env.clone(), t.expr)),
                _ => Ok(Target::Value),
            },
            _ => Ok(Target::Value),
        }
    }

    /// A zero-arity definition TLC's lookup finds as an `OpDefNode`: its body
    /// in a fresh frame (`None` when the lookup gives a value instead).
    fn def_follow(&self, env: &Env, d: DefId) -> Option<(Env, ExprId)> {
        let d = self.redirect(d);
        let def = self.ir.def(d);
        if self.def_values.contains_key(&d)
            || def.std.is_some()
            || (self.cacheable(d) && self.def_cache[d.index()].get().is_some())
            || !def.params.is_empty()
        {
            return None;
        }
        let parent = match def.scope {
            DefScope::Local { .. } | DefScope::Inline { .. } => {
                Some(env.frame_at_depth(def.depth.saturating_sub(1))?.clone())
            }
            _ => None,
        };
        Some((
            Env::new(d, def.depth, def.frame.len(), parent, env.dynamic.clone()),
            def.body,
        ))
    }

    /// `getVar`: the root variable `e` denotes, if it is one, seeing through
    /// substitutions, `LET`s, labels, zero-arity definitions and lazy
    /// arguments (`SymbolNodeValueLookupProvider.getVar`).
    pub(crate) fn var_of(
        self: &Arc<Self>,
        env: &Env,
        e: ExprId,
        cutoff: bool,
    ) -> Result<Option<usize>> {
        Ok(self
            .var_lookup(env, e, VariableLookup::Root(cutoff), false, 0)?
            .and_then(|decl| self.ir.decl(decl).var)
            .map(dive_ir::VarId::index))
    }

    /// `getPrimedVar`: the root variable `e` is the primed form of.
    pub(crate) fn primed_var(
        self: &Arc<Self>,
        env: &Env,
        e: ExprId,
        cutoff: bool,
    ) -> Result<Option<usize>> {
        Ok(self
            .var_lookup(env, e, VariableLookup::Root(cutoff), true, 0)?
            .and_then(|decl| self.ir.decl(decl).var)
            .map(dive_ir::VarId::index))
    }

    fn var_lookup(
        self: &Arc<Self>,
        env: &Env,
        e: ExprId,
        mode: VariableLookup,
        primed: bool,
        depth: u32,
    ) -> Result<Option<dive_ir::DeclId>> {
        if depth > 256 {
            return Err(EvalError::resource_limit(format!(
                "following definitions to a variable more than 256 levels deep ({})",
                self.loc(e)
            )));
        }
        let next = |core: &Arc<Self>, env: &Env, e: ExprId| {
            core.var_lookup(env, e, mode, primed, depth + 1)
        };
        match &self.ir.node(e).expr {
            Expr::Subst { bindings, body } => {
                let env1 = self.bind_subst(env, &St::default(), bindings, false)?;
                next(self, &env1, *body)
            }
            Expr::Let { defs, body } => {
                let env1 = self.bind_let_uncached(env, defs);
                next(self, &env1, *body)
            }
            Expr::Label { body, .. } => next(self, env, *body),
            Expr::Var { var, primed: p } => {
                Ok((*p == primed).then_some(self.ir.vars()[var.index()]))
            }
            Expr::Prime(a) if primed => self.var_lookup(env, *a, mode, false, depth + 1),
            Expr::SubstRef { decl, primed: p } => {
                if *p && !primed {
                    return Ok(None);
                }
                // `x'` with `x` substituted is `getVar(x)`.
                let as_primed = primed && !*p;
                let d = self.ir.decl(*decl);
                let cut = mode.cutoff() && d.variable;
                match env.dynamic.lookup_cut(*decl, cut) {
                    Some(Sv::Thunk(t)) => {
                        self.var_lookup(&t.env, t.expr, mode, as_primed, depth + 1)
                    }
                    Some(_) => Ok(None),
                    None if d.variable && !as_primed => {
                        if d.var.is_some() || matches!(mode, VariableLookup::Enabled) {
                            return Ok(Some(*decl));
                        }
                        match self.root_var_named(&d.name) {
                            Some(v) => Ok(Some(self.ir.vars()[v])),
                            None => Err(EvalError::unsupported(format!(
                                "a variable of an instantiated module, hidden by ENABLED, with no root variable of its name ({})",
                                self.loc(e)
                            ))),
                        }
                    }
                    None => match d.constant {
                        Some(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                            Some(ConstBinding::Def(t)) => match self.def_follow(env, *t) {
                                Some((env1, body)) => {
                                    self.var_lookup(&env1, body, mode, as_primed, depth + 1)
                                }
                                None => Ok(None),
                            },
                            _ => Ok(None),
                        },
                        None => Ok(None),
                    },
                }
            }
            Expr::Def(d) => match self.def_follow(env, *d) {
                Some((env1, body)) => next(self, &env1, body),
                None => Ok(None),
            },
            Expr::Local(lr) => match self.slot(env, *lr)? {
                Sv::Thunk(t) => self.var_lookup(&t.env, t.expr, mode, primed, depth + 1),
                _ => Ok(None),
            },
            Expr::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(t)) => match self.def_follow(env, *t) {
                    Some((env1, body)) => next(self, &env1, body),
                    None => Ok(None),
                },
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Binds a `LET`'s zero-arity definitions as uncached lazy values: the
    /// enumerator does not bind them, and each use evaluates the definition
    /// afresh (`getNextStates`, `LetInKind`).
    pub(crate) fn bind_let_uncached(self: &Arc<Self>, env: &Env, defs: &[DefId]) -> Env {
        self.bind_let(env, defs, false)
    }

    fn bind_let(self: &Arc<Self>, env: &Env, defs: &[DefId], cachable: bool) -> Env {
        let mut env1 = env.clone();
        for &d in defs {
            let def = self.ir.def(d);
            if let DefScope::LetValue { slot, .. } = def.scope {
                let t = Thunk::new(def.body, env1.clone(), cachable);
                env1.bind(slot.index(), Sv::Thunk(t));
            }
        }
        env1
    }

    /// Conses a `Subst`'s bindings onto the dynamic context, as lazy values
    /// over the outer context (cached or not, as the caller's TLC code does).
    /// `Tool.getFcnContext`: the lambda's context with its parameters bound
    /// to `arg`, for applying a lazy function as an action. TLC ignores the
    /// lambda's pending `EXCEPT`s here, so one with any is refused, as is an
    /// argument TLC would reject (its messages are not reproduced).
    pub(crate) fn fcn_context(
        self: &Arc<Self>,
        f: &Arc<crate::lazy::LazyFcn>,
        arg: &Value,
        e: ExprId,
    ) -> Result<Env> {
        let refuse = |what: &str| {
            Err(EvalError::unsupported(format!(
                "applying a lazily evaluated function (TLC's FcnLambdaValue) as an action, {what} ({})",
                self.loc(e)
            )))
        };
        if !f.excepts.is_empty() {
            return refuse("with EXCEPTs pending");
        }
        let mut env = f.body_env();
        if f.bounds.len() == 1 {
            let b = &f.bounds[0];
            if !self.member(arg, &b.domain)? {
                return refuse("to an argument outside its domain");
            }
            if b.tuple {
                match arg {
                    Value::Fcn(t) if t.as_tuple().is_some_and(|t| t.len() == b.slots.len()) => {
                        self.bind_one(&mut env, &b.slots, true, arg, e)?;
                    }
                    _ => return refuse("to an argument that does not match its tuple parameter"),
                }
            } else if let Some(&s) = b.slots.first() {
                env.bind(s, Sv::Val(arg.clone()));
            }
            return Ok(env);
        }
        let Some(elems) = (match arg {
            Value::Fcn(t) => t.as_tuple(),
            _ => None,
        }) else {
            return refuse("to an argument that is not a tuple");
        };
        let mut k = 0;
        for b in &f.bounds {
            let n = if b.tuple { 1 } else { b.slots.len() };
            for j in 0..n {
                let Some(x) = elems.get(k) else {
                    return refuse("to too few arguments");
                };
                if !self.member(x, &b.domain)? {
                    return refuse("to an argument outside its domain");
                }
                if b.tuple {
                    match x {
                        Value::Fcn(t) if t.as_tuple().is_some_and(|t| t.len() == b.slots.len()) => {
                            self.bind_one(&mut env, &b.slots, true, x, e)?;
                        }
                        _ => {
                            return refuse(
                                "to an argument that does not match its tuple parameter",
                            );
                        }
                    }
                } else {
                    env.bind(b.slots[j], Sv::Val(x.clone()));
                }
                k += 1;
            }
        }
        Ok(env)
    }

    /// TLC's `toString` of a value in a message.
    pub(crate) fn ev_string(self: &Arc<Self>, v: &Ev) -> String {
        match v {
            Ev::V(v) => dive_value::print::to_tlc_string_lenient(v, self.ctx),
            Ev::F(f) => match self.materialize(f) {
                Ok(v) => dive_value::print::to_tlc_string_lenient(&v, self.ctx),
                Err(_) => "a function".to_owned(),
            },
        }
    }

    /// `TLC_UNCHANGED_VARIABLE_CHANGED` (2143), a warning.
    pub(crate) fn unchanged_warning(&self, var: usize, e: ExprId) {
        let name = self
            .ir
            .vars()
            .get(var)
            .map_or("?", |&d| self.ir.decl(d).name.as_str());
        self.unchanged_named_warning(name, e);
    }

    fn unchanged_named_warning(&self, name: &str, e: ExprId) {
        self.warn(
            2143,
            &format!(
                "The variable {name} was changed while it is specified as UNCHANGED at\n{}",
                self.loc(e)
            ),
        );
    }
}

// -----------------------------------------------------------------------------
// Splitting the next-state relation (`Tool.getActions`)
// -----------------------------------------------------------------------------

/// Why a subtree could not be split.
enum Split1 {
    /// TLC throws here (`ClassCastException`, a failed evaluation): the
    /// nearest enclosing inlining or `\E` catches it.
    Throw(EvalError),
    /// dive cannot tell what TLC does (a refusal, a resource limit, a bug).
    Fatal(EvalError),
}

impl From<EvalError> for Split1 {
    fn from(e: EvalError) -> Self {
        if e.class() == ErrorClass::Eval {
            Self::Throw(e)
        } else {
            Self::Fatal(e)
        }
    }
}

struct Expander<'a> {
    core: &'a Arc<Core>,
    split: &'a Split,
    out: Vec<Action>,
    /// The definitions inlined around the current node, with their
    /// parameters' values.
    stack: Vec<(DefId, Vec<(String, Value)>)>,
    first_id: usize,
}

impl Expander<'_> {
    fn push(&mut self, expr: ExprId, env: &Env, name: ActionName) {
        let params = match name {
            ActionName::Def(d) => self
                .stack
                .iter()
                .rev()
                .find(|(s, _)| *s == d)
                .map(|(_, p)| p.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let values: Vec<String> = params
            .iter()
            .map(|(_, v)| dive_value::print::to_tlc_string_lenient(v, self.core.ctx))
            .collect();
        let id = self.first_id + self.out.len();
        self.out.push(Action {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(self.core),
            expr,
            env: env.clone(),
            name,
            label: label(&self.core.ir, name, &values, expr),
            params,
            id,
        });
    }

    fn expand(&mut self, t: &ActionTree, env: &Env) -> std::result::Result<(), Split1> {
        let core = self.core;
        let st = St {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CLEAR,
        };
        match t {
            ActionTree::Leaf { expr, name } => {
                self.push(*expr, env, *name);
                Ok(())
            }
            ActionTree::Throws { expr } => Err(Split1::Throw(EvalError::general(format!(
                "TLC cannot split the next-state relation at {}",
                core.loc(*expr)
            )))),
            ActionTree::Disj { children, .. } => {
                for c in children {
                    self.expand(c, env)?;
                }
                Ok(())
            }
            ActionTree::Scope { expr, body } => {
                let env1 = match &core.ir.node(*expr).expr {
                    Expr::Let { defs, .. } => core.bind_let_uncached(env, defs),
                    _ => env.clone(),
                };
                self.expand(body, &env1)
            }
            ActionTree::Exists {
                expr,
                name: n,
                body,
            } => {
                let cnt = self.out.len();
                let Expr::Quant { bounds, .. } = &core.ir.node(*expr).expr else {
                    return Err(Split1::Fatal(EvalError::bug(
                        "Exists node is not a quantifier",
                    )));
                };
                let mut calls = 0usize;
                let mut failed: Option<Split1> = None;
                let r =
                    core.for_each_context(&mut env.clone(), &st, bounds, *expr, &mut |_, env1| {
                        calls += 1;
                        match self.expand(body, env1) {
                            Ok(()) => Ok(true),
                            Err(x) => {
                                failed = Some(x);
                                Ok(false)
                            }
                        }
                    });
                let outcome = match (r, failed) {
                    (Err(e), _) => Split1::from(e),
                    (Ok(()), Some(x)) => x,
                    (Ok(()), None) => {
                        if calls == 0 {
                            // `Enum.isDone()`: an empty enumeration is one action.
                            self.push(*expr, env, *n);
                        }
                        return Ok(());
                    }
                };
                match outcome {
                    Split1::Throw(_) => {
                        self.out.truncate(cnt);
                        self.push(*expr, env, *n);
                        Ok(())
                    }
                    fatal @ Split1::Fatal(_) => Err(fatal),
                }
            }
            ActionTree::Inline {
                expr,
                def,
                constant_args,
            } => self.inline(*expr, *def, *constant_args, env),
            ActionTree::Configured { expr, name: n } => {
                let node = &core.ir.node(*expr).expr;
                let (c, args): (Option<dive_ir::ConstId>, &[Arg]) = match node {
                    Expr::Const(c) => (Some(*c), &[]),
                    Expr::Apply {
                        callee: OpRef::Const(c),
                        args,
                    } => (Some(*c), args),
                    _ => (None, &[]),
                };
                let Some(c) = c else {
                    return Err(Split1::Fatal(EvalError::unsupported(format!(
                        "an instance substitution used as an action of the next-state relation ({})",
                        core.loc(*expr)
                    ))));
                };
                match core.consts.get(c.index()).and_then(Option::as_ref) {
                    Some(ConstBinding::Def(d)) => {
                        let constant = args.iter().all(|a| match a {
                            Arg::Expr { expr, .. } => {
                                core.ir.node(*expr).level == dive_ir::Level::Constant
                            }
                            Arg::Op { level, .. } => *level == dive_ir::Level::Constant,
                        });
                        let _ = n;
                        self.inline(*expr, *d, constant, env)
                    }
                    _ => Err(Split1::Throw(EvalError::general(format!(
                        "TLC cannot split the next-state relation at {}",
                        core.loc(*expr)
                    )))),
                }
            }
        }
    }

    /// `getActionsAppl` for a user definition: inlined with its constant
    /// arguments evaluated, or one action named after it.
    fn inline(
        &mut self,
        expr: ExprId,
        def: DefId,
        constant_args: bool,
        env: &Env,
    ) -> std::result::Result<(), Split1> {
        let core = self.core;
        let name = ActionName::Def(def);
        if !constant_args {
            self.push(expr, env, name);
            return Ok(());
        }
        let Some(body) = self.split.defs.get(&def) else {
            return Err(Split1::Fatal(EvalError::bug(
                "inlined definition without a tree",
            )));
        };
        let args: Vec<Arg> = match &core.ir.node(expr).expr {
            Expr::Apply { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        let st = St {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CLEAR,
        };
        let attempt = (|| -> std::result::Result<(), Split1> {
            let d = core.ir.def(def);
            // `eval(args[i], con, TLCState.Empty)`: values, not lazy values.
            let mut vals = Vec::with_capacity(args.len());
            for a in &args {
                vals.push(match a {
                    Arg::Expr { expr, .. } => Sv::from(core.eval(&mut env.clone(), &st, *expr)?),
                    Arg::Op { op, .. } => Sv::op(core.op_value(env, op)?),
                });
            }
            let parent = match d.scope {
                DefScope::Local { .. } | DefScope::Inline { .. } => Some(
                    env.frame_at_depth(d.depth.saturating_sub(1))
                        .cloned()
                        .ok_or_else(|| Split1::Fatal(EvalError::bug("no enclosing frame")))?,
                ),
                DefScope::LetValue { .. } | DefScope::TopLevel => None,
            };
            let mut env1 = match d.scope {
                // A zero-arity `LET` value is evaluated in its parent's frame.
                DefScope::LetValue { .. } => env.clone(),
                _ => Env::new(def, d.depth, d.frame.len(), parent, env.dynamic.clone()),
            };
            let mut params = Vec::new();
            for (i, sv) in vals.into_iter().enumerate() {
                if let (Sv::Val(v), Some(p)) = (&sv, d.params.get(i)) {
                    params.push((p.name.clone(), v.clone()));
                }
                env1.bind(i, sv);
            }
            self.stack.push((def, params));
            let r = self.expand(body, &env1);
            self.stack.pop();
            r
        })();
        match attempt {
            Ok(()) => Ok(()),
            Err(Split1::Throw(_)) => {
                // `catch (Throwable e) { /*SKIP*/ }`: the actions the body
                // added stay, and the application is one more action.
                self.push(expr, env, name);
                Ok(())
            }
            Err(fatal) => Err(fatal),
        }
    }
}

// -----------------------------------------------------------------------------
// ENABLED (`Tool.enabled`, `Tool.java:2953-3570`)
// -----------------------------------------------------------------------------

impl Evaluator {
    /// Decides whether a scoped action has a successor, using TLC's existing
    /// ENABLED search. A subscript requests a non-stuttering successor,
    /// equivalent to `ENABLED <<action>>_sub`. The checker, the liveness
    /// checker and the trace validator all decide `ENABLED` here (ADR 0103).
    ///
    /// ```
    /// use std::sync::Arc;
    /// use dive_eval::{Evaluator, Model, Options};
    /// use dive_value::Value;
    /// let src = "---- MODULE M ----\nVARIABLE x\nNext == x' = ~x\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    /// let d = ir.root_def("Next").expect("Next");
    /// let ev = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("valid context");
    /// assert!(ev.enabled_scoped(&ev.scope(d).expect("valid scope"), ir.def(d).body, None,
    ///     Arc::from([Some(Value::Bool(false))])).expect("enabled"));
    /// ```
    ///
    /// # Errors
    /// Malformed current state or the existing TLC ENABLED evaluation errors.
    pub fn enabled_scoped(
        &self,
        scope: &Scope,
        action: ExprId,
        sub: Option<ExprId>,
        state: StateVals,
    ) -> Result<bool> {
        self.validate_scope(scope)?;
        self.admit_scoped_expr(scope, action)?;
        if let Some(sub) = sub {
            self.admit_scoped_expr(scope, sub)?;
        }
        if state.len() != self.core.ir.vars().len() || state.iter().any(Option::is_none) {
            return Err(EvalError::general(
                "ENABLED requires a complete current state",
            ));
        }
        let mut env = scope.env.clone();
        env.dynamic.branch();
        let acts = sub.and_then(|sub| cons(&None, Kind::Changed, sub, &env));
        let en = Enab {
            core: self.core.clone(),
            s0: StateRef::Vals(state),
        };
        let successor = crate::NamedState::empty(&self.core);
        Ok(en.enabled(action, &acts, &env, successor)?.is_some())
    }
}

impl Core {
    /// `OPCODE_enabled`: whether the action has a successor from the current
    /// state, searched as TLC's `enabled` searches, in a branched context.
    pub(crate) fn eval_enabled_native(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        a: ExprId,
        _e: ExprId,
    ) -> Result<Ev> {
        let mut env1 = env.clone();
        env1.dynamic.branch();
        let s1 = crate::NamedState::empty(self);
        let en = Enab {
            core: self.clone(),
            s0: st.s0.clone(),
        };
        let found = en.enabled(a, &None, &env1, s1)?;
        Ok(Ev::V(Value::Bool(found.is_some())))
    }
}

#[derive(Clone, Copy)]
enum VariableLookup {
    Root(bool),
    Enabled,
}

impl VariableLookup {
    fn cutoff(self) -> bool {
        match self {
            Self::Root(cutoff) => cutoff,
            Self::Enabled => true,
        }
    }
}

type EnabledState = Arc<crate::NamedState>;

struct Enab {
    core: Arc<Core>,
    s0: StateRef,
}

impl Enab {
    fn st(&self, s1: &EnabledState) -> St {
        St {
            s0: self.s0.clone(),
            s1: StateRef::Named(s1.clone()),
            control: control::ENABLED,
        }
    }

    fn eval(&self, env: &Env, s1: &EnabledState, e: ExprId) -> Result<Ev> {
        self.core.eval(&mut env.clone(), &self.st(s1), e)
    }

    fn with(&self, s1: &EnabledState, var: dive_ir::DeclId, v: Value) -> EnabledState {
        s1.with(&self.core.ir.decl(var).name, v)
    }

    fn loc(&self, e: ExprId) -> String {
        self.core.loc(e)
    }

    /// `enabled(acts, s0, s1)`.
    fn cont(&self, acts: &Acts, s1: EnabledState) -> Result<Option<EnabledState>> {
        let Some(item) = acts else {
            return Ok(Some(s1));
        };
        match item.kind {
            Kind::Pred => self.enabled(item.expr, &item.next, &item.env, s1),
            Kind::Unchanged => self.unchanged(item.expr, &item.next, &item.env, s1),
            Kind::Changed => {
                let st0 = St {
                    s0: self.s0.clone(),
                    s1: StateRef::Empty,
                    control: control::ENABLED,
                };
                let st1 = St {
                    s0: StateRef::Named(s1.clone()),
                    s1: StateRef::Null,
                    control: control::PRIMED,
                };
                let v1 = self.core.eval_v(&mut item.env.clone(), &st0, item.expr)?;
                let v2 = self.core.eval_v(&mut item.env.clone(), &st1, item.expr)?;
                if self.core.equals(&v1, &v2)? {
                    return Ok(None);
                }
                self.cont(&item.next, s1)
            }
        }
    }

    fn after_bool(
        &self,
        v: &Ev,
        acts: &Acts,
        s1: EnabledState,
        e: ExprId,
    ) -> Result<Option<EnabledState>> {
        match v {
            Ev::V(Value::Bool(true)) => self.cont(acts, s1),
            Ev::V(Value::Bool(false)) => Ok(None),
            other => Err(EvalError::coded(
                Ec::ExpectedExpressionInComputing,
                format!(
                    "In computing ENABLED, TLC expected a boolean expression,\nbut instead found {}.\n{}",
                    self.core.ev_string(other),
                    self.loc(e)
                ),
            )),
        }
    }

    fn wrong(&self, what: &str, e: ExprId) -> EvalError {
        EvalError::coded(
            Ec::EnabledWrongFormula,
            format!(
                "In computing ENABLED, TLC encountered a temporal formula ({what}).\n{}",
                self.loc(e)
            ),
        )
    }

    /// `enabled(pred, acts, c, s0, s1)`.
    fn enabled(
        &self,
        e: ExprId,
        acts: &Acts,
        env: &Env,
        s1: EnabledState,
    ) -> Result<Option<EnabledState>> {
        let core = &self.core;
        let _g = core.enter(e)?;
        match &core.ir.node(e).expr {
            Expr::Let { defs, body } => {
                let env1 = core.bind_let(env, defs, true);
                self.enabled(*body, acts, &env1, s1)
            }
            Expr::Subst { bindings, body } => {
                let env1 = core.bind_subst(env, &self.st(&s1), bindings, false)?;
                self.enabled(*body, acts, &env1, s1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.enabled(*body, acts, env, s1),
            Expr::Def(_)
            | Expr::Apply { .. }
            | Expr::Local(_)
            | Expr::Const(_)
            | Expr::SubstRef { primed: false, .. } => {
                match core.op_target(env, &self.st(&s1), e)? {
                    OpTarget::Body(env1, body) => self.enabled(body, acts, &env1, s1),
                    OpTarget::Thunk(t) => self.enabled(t.expr, acts, &t.env, s1),
                    OpTarget::Value(v) => self.after_bool(&v, acts, s1, e),
                }
            }
            Expr::StepAngle { action, sub } => {
                let acts1 = cons(acts, Kind::Changed, *sub, env);
                self.enabled(*action, &acts1, env, s1)
            }
            Expr::Quant {
                kind: QuantKind::Exists,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                let st = self.st(&s1);
                let mut found = None;
                let mut err = None;
                core.for_each_context(&mut env.clone(), &st, bounds, e, &mut |_, env1| match self
                    .enabled(*body, acts, env1, s1.clone())
                {
                    Ok(Some(s)) => {
                        found = Some(s);
                        Ok(false)
                    }
                    Ok(None) => Ok(true),
                    Err(x) => {
                        err = Some(x);
                        Ok(false)
                    }
                })?;
                match err {
                    Some(x) => Err(x),
                    None => Ok(found),
                }
            }
            Expr::Quant {
                kind: QuantKind::Forall,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                let st = self.st(&s1);
                let mut envs: Vec<Env> = Vec::new();
                core.for_each_context(&mut env.clone(), &st, bounds, e, &mut |_, env1| {
                    envs.push(env1.clone());
                    Ok(true)
                })?;
                let Some((first, rest)) = envs.split_first() else {
                    return self.cont(acts, s1);
                };
                let mut acts1 = acts.clone();
                for c2 in rest {
                    acts1 = cons(&acts1, Kind::Pred, *body, c2);
                }
                self.enabled(*body, &acts1, first, s1)
            }
            Expr::Quant { kind, .. } => Err(EvalError::general(match kind {
                QuantKind::Choose => format!(
                    "In computing ENABLED, TLC encountered unbounded CHOOSE. Make sure that the expression is of form CHOOSE x \\in S: P(x).\n{}",
                    self.loc(e)
                ),
                QuantKind::Exists => format!(
                    "In computing ENABLED, TLC encountered unbounded quantifier. Make sure that the expression is of form \\E x \\in S: P(x).\n{}",
                    self.loc(e)
                ),
                QuantKind::Forall => format!(
                    "In computing ENABLED, TLC encountered unbounded quantifier. Make sure that the expression is of form \\A x \\in S: P(x).\n{}",
                    self.loc(e)
                ),
            })),
            Expr::Case { arms, other } => {
                for arm in arms {
                    match self.eval(env, &s1, arm.guard)? {
                        Ev::V(Value::Bool(true)) => return self.enabled(arm.value, acts, env, s1),
                        Ev::V(Value::Bool(false)) => {}
                        other => {
                            return Err(EvalError::general(format!(
                                "In computing ENABLED, a non-boolean expression({}) was used as a guard condition of a CASE.\n{}",
                                core.kind_of(&other),
                                self.loc(arm.value)
                            )));
                        }
                    }
                }
                match other {
                    Some(o) => self.enabled(*o, acts, env, s1),
                    None => Err(EvalError::general(format!(
                        "In computing ENABLED, TLC encountered a CASE with no conditions true.\n{}",
                        self.loc(e)
                    ))),
                }
            }
            Expr::Junction { kind, items, .. } => match kind {
                JunctionKind::And => {
                    let mut acts1 = acts.clone();
                    for &i in items.iter().skip(1).rev() {
                        acts1 = cons(&acts1, Kind::Pred, i, env);
                    }
                    match items.first() {
                        Some(&f) => self.enabled(f, &acts1, env, s1),
                        None => self.cont(acts, s1),
                    }
                }
                JunctionKind::Or => {
                    for &i in items {
                        if let Some(s) = self.enabled(i, acts, env, s1.clone())? {
                            return Ok(Some(s));
                        }
                    }
                    Ok(None)
                }
            },
            Expr::FcnApply { fcn, arg } => {
                let st = St {
                    control: control::ENABLED | control::KEEP_LAZY,
                    ..self.st(&s1)
                };
                let f = core.eval(&mut env.clone(), &st, *fcn)?;
                let fv = match f {
                    Ev::F(lf) => {
                        if let Some(v) = lf.materialized.get(core.host.cache_domain()) {
                            v
                        } else {
                            let a = core.force_fcn(self.eval(env, &s1, *arg)?)?;
                            let env1 = core.fcn_context(&lf, &a, e)?;
                            return self.enabled(lf.body, acts, &env1, s1);
                        }
                    }
                    Ev::V(v) => v,
                };
                let Value::Fcn(fv) = fv else {
                    return Err(EvalError::general(format!(
                        "In computing ENABLED, a non-function ({}) was applied as a function.\n{}",
                        crate::core::kind_string(&fv),
                        self.loc(e)
                    )));
                };
                let a = core.force_fcn(self.eval(env, &s1, *arg)?)?;
                match core.apply_fcn(&fv, &a)? {
                    Value::Bool(true) => self.cont(acts, s1),
                    Value::Bool(false) => Ok(None),
                    _ => Err(EvalError::coded(
                        Ec::ExpectedExpressionInComputing2,
                        format!(
                            "In computing ENABLED, TLC expected a boolean expression,\nbut didn't find one.\n{}",
                            self.loc(e)
                        ),
                    )),
                }
            }
            Expr::If { cond, then, other } => match self.eval(env, &s1, *cond)? {
                Ev::V(Value::Bool(b)) => {
                    self.enabled(if b { *then } else { *other }, acts, env, s1)
                }
                other => Err(EvalError::general(format!(
                    "In computing ENABLED, a non-boolean expression({}) was used as the guard condition of an IF.\n{}",
                    core.kind_of(&other),
                    self.loc(e)
                ))),
            },
            Expr::StepBox { action, sub } => {
                if let Some(s) = self.enabled(*action, acts, env, s1.clone())? {
                    return Ok(Some(s));
                }
                self.unchanged(*sub, acts, env, s1)
            }
            Expr::TemporalQuant { .. } => Err(EvalError::general(format!(
                "In computing ENABLED, TLC encountered temporal quantifier.\n{}",
                self.loc(e)
            ))),
            Expr::Sf { .. } => Err(self.wrong("SF", e)),
            Expr::Wf { .. } => Err(self.wrong("WF", e)),
            Expr::Always(_) => Err(self.wrong("[]", e)),
            Expr::Eventually(_) => Err(self.wrong("<>", e)),
            Expr::LeadsTo(..) => Err(EvalError::general(format!(
                "In computing ENABLED, TLC encountered a temporal formula (a ~> b).\n{}",
                self.loc(e)
            ))),
            Expr::WhilePlus(..) => Err(EvalError::general(format!(
                "In computing ENABLED, TLC encountered a temporal formula (a -+-> formula).\n{}",
                self.loc(e)
            ))),
            Expr::Unchanged(a) => self.unchanged(*a, acts, env, s1),
            Expr::Implies(a, b) => match self.eval(env, &s1, *a)? {
                Ev::V(Value::Bool(true)) => self.enabled(*b, acts, env, s1),
                Ev::V(Value::Bool(false)) => self.cont(acts, s1),
                other => Err(EvalError::general(format!(
                    "While computing ENABLED of an expression of the form P => Q, P was {}.\n{}",
                    core.kind_of(&other),
                    self.loc(e)
                ))),
            },
            Expr::Cdot(a, b) => self.cdot(*a, *b, acts, env, &s1, e),
            Expr::Builtin {
                op: op @ (BuiltinOp::Eq | BuiltinOp::In | BuiltinOp::Subseteq),
                args,
            } if args.len() == 2 => self.assign(*op, args[0], args[1], e, acts, env, s1),
            _ => {
                let v = self.eval(env, &s1, e)?;
                self.after_bool(&v, acts, s1, e)
            }
        }
    }

    fn bind_value(
        &self,
        var: dive_ir::DeclId,
        value: Value,
        acts: &Acts,
        s1: &EnabledState,
    ) -> Result<Option<EnabledState>> {
        // Rejected query candidates are work even when no witness is found.
        self.core.loop_work()?;
        self.cont(acts, self.with(s1, var, value))
    }

    fn bind_interval(
        &self,
        var: dive_ir::DeclId,
        lo: &dive_value::Int,
        hi: &dive_value::Int,
        acts: &Acts,
        s1: &EnabledState,
    ) -> Result<Option<EnabledState>> {
        // IntervalValue.elements() is incremental and ascending, as in the
        // ordinary assignment path. ENABLED stops at its first witness.
        if let (Some(lo), Some(hi)) = (lo.to_i64(), hi.to_i64()) {
            for value in lo..=hi {
                if let Some(found) = self.bind_value(var, Value::Int(value), acts, s1)? {
                    return Ok(Some(found));
                }
            }
        } else {
            let mut value = lo.clone();
            while value <= *hi {
                if let Some(found) = self.bind_value(var, Value::int(value.clone()), acts, s1)? {
                    return Ok(Some(found));
                }
                if value == *hi {
                    break;
                }
                value = value.add(&dive_value::Int::ONE, dive_value::IntMode::Big)?;
            }
        }
        Ok(None)
    }

    fn assign(
        &self,
        op: BuiltinOp,
        lhs: ExprId,
        rhs: ExprId,
        e: ExprId,
        acts: &Acts,
        env: &Env,
        s1: EnabledState,
    ) -> Result<Option<EnabledState>> {
        let core = &self.core;
        let Some(var) = core.var_lookup(env, lhs, VariableLookup::Enabled, true, 0)? else {
            return match self.eval(env, &s1, e)? {
                Ev::V(Value::Bool(true)) => self.cont(acts, s1),
                Ev::V(Value::Bool(false)) => Ok(None),
                _ => Err(EvalError::bug("=, \\in or \\subseteq not boolean")),
            };
        };
        let lval = s1.get(&core.ir.decl(var).name).cloned();
        match op {
            BuiltinOp::Eq => {
                let rval = core.embed(self.eval(env, &s1, rhs)?)?;
                match lval {
                    None => {
                        let s2 = s1.with_owned(&core.ir.decl(var).name, rval);
                        self.cont(acts, s2)
                    }
                    Some(l) => {
                        if core.equals(&l, &rval)? {
                            self.cont(acts, s1)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
            BuiltinOp::In => {
                let rval = core.force_fcn(self.eval(env, &s1, rhs)?)?;
                match lval {
                    None => {
                        let elems = match &rval {
                            Value::Set(set) if is_enumerable(&rval) => {
                                if let Some((lo, hi)) = set.as_interval() {
                                    return self.bind_interval(var, lo, hi, acts, &s1);
                                }
                                if let Some(elements) = core.in_element_stream(set)? {
                                    for value in elements.with_owner(rval) {
                                        let value = value?;
                                        if let Some(found) =
                                            self.bind_value(var, value, acts, &s1)?
                                        {
                                            return Ok(Some(found));
                                        }
                                    }
                                    return Ok(None);
                                }
                                core.in_elements(set)?
                            }
                            _ => {
                                return Err(EvalError::general(format!(
                                    "The right side of \\IN is not enumerable.\n{}",
                                    self.loc(e)
                                )));
                            }
                        };
                        for x in elems.iter() {
                            if let Some(s) = self.bind_value(var, x.clone(), acts, &s1)? {
                                return Ok(Some(s));
                            }
                        }
                        Ok(None)
                    }
                    Some(l) => {
                        if core.member(&l, &rval)? {
                            self.cont(acts, s1)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
            _ => {
                let rset = core.force_fcn(self.eval(env, &s1, rhs)?)?;
                if !matches!(rset, Value::Set(_)) {
                    return Err(EvalError::unsupported(format!(
                        "x' \\subseteq S where S is not a set ({})",
                        self.loc(e)
                    )));
                }
                let rval = Value::subset(rset, core.ctx);
                match lval {
                    None => {
                        let Value::Set(s) = &rval else {
                            return Err(EvalError::bug("SUBSET is not a set"));
                        };
                        for x in core.explore(s)?.iter() {
                            let s2 = self.with(&s1, var, x.clone());
                            if let Some(s) = self.cont(acts, s2)? {
                                return Ok(Some(s));
                            }
                        }
                        Ok(None)
                    }
                    Some(l) => {
                        if core.member(&l, &rval)? {
                            self.cont(acts, s1)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
        }
    }

    /// `enabledUnchanged` (`Tool.java:3506-3570`).
    fn unchanged(
        &self,
        e: ExprId,
        acts: &Acts,
        env: &Env,
        s1: EnabledState,
    ) -> Result<Option<EnabledState>> {
        let core = &self.core;
        let _g = core.enter(e)?;
        if let Some(var) = core.var_lookup(env, e, VariableLookup::Enabled, false, 0)? {
            let v0 = core.force_fcn(self.eval(env, &s1, e)?)?;
            return match s1.get(&core.ir.decl(var).name).cloned() {
                None => {
                    let s2 = s1.with_owned(&core.ir.decl(var).name, v0);
                    self.cont(acts, s2)
                }
                Some(v1) => {
                    if core.equals(&v1, &v0)? {
                        self.cont(acts, s1)
                    } else {
                        core.unchanged_named_warning(&core.ir.decl(var).name, e);
                        Ok(None)
                    }
                }
            };
        }
        match &core.ir.node(e).expr {
            Expr::Tuple(items) => {
                let Some((&first, rest)) = items.split_first() else {
                    return self.cont(acts, s1);
                };
                // `for (int i = 1; i < alen; i++) acts1 = acts1.cons(args[i])`:
                // the last item is checked right after the first.
                let mut acts1 = acts.clone();
                for &i in rest {
                    acts1 = cons(&acts1, Kind::Unchanged, i, env);
                }
                self.unchanged(first, &acts1, env, s1)
            }
            Expr::Def(_)
            | Expr::Local(_)
            | Expr::Const(_)
            | Expr::SubstRef { primed: false, .. } => {
                match core.zero_arity_target(env, e)? {
                    Target::Body(env1, body) => self.unchanged(body, acts, &env1, s1),
                    // Any other value of a zero-arity name is unchanged.
                    Target::Value => self.cont(acts, s1),
                }
            }
            _ => {
                let st0 = St {
                    s0: self.s0.clone(),
                    s1: StateRef::Empty,
                    control: control::ENABLED,
                };
                let st1 = St {
                    s0: StateRef::Named(s1.clone()),
                    s1: StateRef::Empty,
                    control: control::PRIMED,
                };
                let v0 = core.eval_v(&mut env.clone(), &st0, e)?;
                let v1 = core.eval_v(&mut env.clone(), &st1, e)?;
                if core.equals(&v0, &v1)? {
                    self.cont(acts, s1)
                } else {
                    Ok(None)
                }
            }
        }
    }
}
