//! The bound model: what the checker, simulator and trace validator consume.

use std::sync::Arc;

use dive_eval::{AssumptionOutcome, Evaluator, Scope, St};
use dive_ir::{DefId, ExprId, Ir};
use dive_value::{Interner, Value};

use crate::error::{BindError, Message};

/// A predicate or expression TLC evaluates for a role, with the context it
/// is evaluated in: TLC's `tlc2.tool.Action` (`pred`, `con`).
#[derive(Clone, Debug)]
pub struct Pred {
    /// The expression.
    pub expr: ExprId,
    /// Where it is evaluated (operator parameters, split `\E` variables,
    /// instance substitutions).
    pub scope: Scope,
    /// The node TLC reports the role's location from: the expression, or the
    /// outermost instance substitution TLC wrapped around it
    /// (`Specs.addSubsts`).
    pub location: ExprId,
}

impl Pred {
    /// A predicate evaluated in its own definition's frame.
    #[must_use]
    pub fn new(expr: ExprId, scope: Scope) -> Self {
        Self {
            expr,
            scope,
            location: expr,
        }
    }
}

/// A role with the name TLC reports it under.
#[derive(Clone, Debug)]
pub struct Named {
    /// The name: the configuration's name for `INVARIANT` and the
    /// constraints, TLC's derived name for parts of a `PROPERTY`.
    pub name: String,
    /// The predicate.
    pub pred: Pred,
}

/// An initial predicate (`getInitStateSpec`).
#[derive(Clone, Debug)]
pub struct Init {
    /// The predicate.
    pub pred: Pred,
    /// The definition it came from, if any (`INIT Init`, or a conjunct of a
    /// `SPECIFICATION` that is a use of a definition).
    pub def: Option<DefId>,
}

/// The `-deadlock` and `CHECK_DEADLOCK` outcome and other run settings.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Whether deadlock is checked: `CHECK_DEADLOCK` and TLC's `-deadlock`
    /// flag together.
    pub check_deadlock: bool,
}

/// Why the model cannot yet receive a complete checking verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedKind {
    /// A temporal property requires the liveness backend.
    TemporalProperty,
    /// Another unimplemented feature, which graph capture cannot defer.
    Feature,
}

/// A feature of the model that dive does not check yet. The checker refuses
/// the model as `unsupported`, naming it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The structured reason, never inferred from diagnostic text.
    pub kind: UnsupportedKind,
    /// The feature, by name.
    pub feature: String,
}

/// A specification bound to its configuration (ADR 0013 §8).
#[derive(Clone, Debug)]
pub struct Model {
    /// The lowered specification.
    pub ir: Arc<Ir>,
    /// The evaluator, with the constants bound and the start-up values
    /// computed. Clones share them.
    pub evaluator: Evaluator,
    /// The run's names, in TLC's encounter order.
    pub names: Arc<Interner>,
    /// What TLC prints while it processes the model, before it checks the
    /// assumptions: `Print` output of constant definitions and warnings.
    pub output: Vec<Message>,
    /// Initial predicates, in TLC's order.
    pub init: Vec<Init>,
    /// The next-state relation as TLC holds it before splitting
    /// (`getNextStateSpec`), if any.
    pub next: Option<Pred>,
    /// The initial predicates as the successor generator's actions
    /// (`Evaluator::init_states`), numbered from 0.
    #[cfg(feature = "enumerate")]
    pub init_actions: Vec<dive_eval::next::Action>,
    /// The next-state relation split into TLC's actions, with TLC's labels,
    /// numbered after the initial predicates (`Evaluator::successors`).
    #[cfg(feature = "enumerate")]
    pub actions: Vec<dive_eval::next::Action>,
    /// Invariants: `INVARIANT`s, then the state predicates `[]P` of
    /// `PROPERTY`s.
    pub invariants: Vec<Named>,
    /// Implied initial predicates: state-level parts of `PROPERTY`s.
    pub implied_inits: Vec<Named>,
    /// Implied actions: the `[A]_v` of `[][A]_v` in `PROPERTY`s.
    pub implied_actions: Vec<Named>,
    /// Temporal formulas of the behavior specification (fairness), named as
    /// TLC names them. Inert for safety checking.
    pub temporals: Vec<Named>,
    /// Temporal properties to check (liveness).
    pub implied_temporals: Vec<Named>,
    /// Original configured PROPERTY formulas, before TLC's role splitting.
    /// These retain configuration names for complete counterexample claims;
    /// the role vectors above keep TLC's evaluation/checking order unchanged.
    pub configured_properties: Vec<Named>,
    /// `CONSTRAINT`s.
    pub constraints: Vec<Named>,
    /// `ACTION_CONSTRAINT`s.
    pub action_constraints: Vec<Named>,
    /// `VIEW`.
    pub view: Option<Named>,
    /// `ALIAS`. TLC looks it up only when it prints a trace
    /// (`Spec.getAliasSpec`), so a lookup error is reported then, not at
    /// start-up.
    pub alias: Option<Result<Named, BindError>>,
    /// `POSTCONDITION`s. TLC looks them up after checking
    /// (`Spec.getPostConditionSpecs`), stopping at the first error, which it
    /// reports then.
    pub postconditions: Result<Vec<Named>, BindError>,
    /// `SYMMETRY`, by name.
    pub symmetry: Option<String>,
    /// Pre-constant-evaluation lookup, retained only for the configured declaration.
    pub(crate) symmetry_definition: Option<crate::tables::Look>,
    /// Run settings.
    pub settings: Settings,
    /// The root module's assumptions TLC checks (not `AXIOM`s), in order.
    pub assumptions: Vec<DefId>,
    /// Features this model uses that dive does not check yet.
    pub unsupported: Vec<Unsupported>,
    /// Why the model must be checked in sequential mode, with one logical
    /// worker evaluating in TLC's order (ADR 0015 §6); empty when a parallel
    /// search gives the same results. Each entry names the feature.
    pub sequential: Vec<String>,
    /// The only sequential restriction is preserving runtime name encounter
    /// order. This is not an I/O, random-stream or live-checker-state effect.
    pub sequential_names_only: bool,
    /// Checking may create runtime names whose encounter order is observable
    /// under the selected value or exploration order. Remains true when other
    /// sequential effects are also reachable.
    pub runtime_name_order: bool,
}

impl Model {
    /// Original configured property claims, or the temporal roles for a model
    /// without retained originals. A quantified conjunction must be attributed
    /// as one configured property, not as ambiguous identically named roles.
    #[must_use]
    pub fn property_claims(&self) -> &[Named] {
        if self.configured_properties.is_empty() {
            &self.implied_temporals
        } else {
            &self.configured_properties
        }
    }

    /// Evaluates a role's predicate.
    ///
    /// # Errors
    /// Any evaluation error, refusal or resource limit.
    pub fn eval(&self, pred: &Pred, st: &St) -> dive_eval::Result<Value> {
        self.evaluator.eval_scoped(&pred.scope, pred.expr, st)
    }

    /// TLC's `Tool.checkAssumptions`: the assumptions in order, `AXIOM`s
    /// skipped, stopping at the first that is not `TRUE`.
    #[must_use]
    pub fn check_assumptions(&self) -> AssumptionOutcome {
        let axioms: Vec<DefId> = {
            let root = self.ir.module(self.ir.root());
            root.assumptions
                .iter()
                .zip(&root.axioms)
                .filter(|(_, ax)| **ax)
                .map(|(d, _)| *d)
                .collect()
        };
        self.evaluator.check_assumptions(|d| axioms.contains(&d))
    }

    /// The `Print` output collected since the last call (the evaluator's
    /// default sink).
    #[must_use]
    pub fn take_output(&self) -> Vec<String> {
        self.evaluator.take_output()
    }

    /// The location TLC prints for a role's predicate (`getLocation()`).
    #[must_use]
    pub fn location(&self, pred: &Pred) -> String {
        self.ir.location(&self.ir.node(pred.location).origin)
    }
}
