//! Chosen-action flows from TLC's `SimulationWorkerStatistics.collectPostSuccessor`
//! and `Simulator.getActionFlowGraphSnapshot(REDUCE)` at tlaplus `94d0c50` (MIT).

use std::collections::BTreeMap;

use dive_ir::actions::ActionName;
use dive_model::Model;

/// One source-location action group, with parameter contexts combined.
#[derive(Clone, Debug)]
pub struct ActionGroup {
    /// Stable group index in initial/next action order.
    pub id: usize,
    /// The first bound action's name.
    pub name: String,
    /// The first bound action's source label (including its parameter context).
    pub label: String,
    /// Source location used to combine parameter contexts.
    pub location: dive_ir::Origin,
    /// Whether the representative is an initial predicate.
    pub initial: bool,
    /// Original bound action identities combined into this group.
    pub bound_action_ids: Vec<usize>,
}

/// Sparse counts of consecutive chosen actions; no state-space coverage claim.
#[derive(Clone, Debug)]
pub struct ActionFlow {
    /// Source-location groups, including groups never chosen.
    pub actions: Vec<ActionGroup>,
    /// Nonzero counts keyed by `(previous group, chosen group)`.
    pub transitions: BTreeMap<(usize, usize), u64>,
    groups: BTreeMap<usize, usize>,
}

impl ActionFlow {
    pub(crate) fn new(model: &Model) -> Result<Self, dive_eval::EvalError> {
        model
            .evaluator
            .validate_model_context(&model.ir, &model.names)?;
        let mut flow = Self {
            actions: Vec::new(),
            transitions: BTreeMap::new(),
            groups: BTreeMap::new(),
        };
        let mut locations = BTreeMap::new();
        for (action, initial) in model
            .init_actions
            .iter()
            .map(|a| (a, true))
            .chain(model.actions.iter().map(|a| (a, false)))
        {
            model
                .evaluator
                .validate_predicate(&action.scope(), action.expr())?;
            let origin = model.ir.node(action.expr()).origin;
            let name = match action.name() {
                ActionName::Def(def) => model
                    .ir
                    .defs()
                    .get(def.index())
                    .ok_or_else(|| {
                        dive_eval::EvalError::unsupported(
                            "basic simulation statistics require a valid action definition ID",
                        )
                    })?
                    .name
                    .clone(),
                ActionName::Std(op) => op.name().to_owned(),
                ActionName::Builtin(name) => name.to_owned(),
                ActionName::Unnamed => "Action".to_owned(),
            };
            // TLC groups Action.getDefinition(), the predicate's source location,
            // rather than its name or its evaluated parameter values.
            let key = (origin.file, origin.begin, origin.end);
            let group = *locations.entry(key).or_insert_with(|| {
                let id = flow.actions.len();
                flow.actions.push(ActionGroup {
                    id,
                    name,
                    label: action.label().to_owned(),
                    location: origin,
                    initial,
                    bound_action_ids: Vec::new(),
                });
                id
            });
            flow.actions[group].bound_action_ids.push(action.id());
            if flow.groups.insert(action.id(), group).is_some() {
                return Err(dive_eval::EvalError::unsupported(
                    "basic simulation statistics require unique bound action IDs",
                ));
            }
        }
        Ok(flow)
    }

    pub(crate) fn record(
        &mut self,
        previous: usize,
        chosen: usize,
    ) -> Result<(), dive_eval::EvalError> {
        // Init callbacks return an action from model.init_actions; chosen actions
        // come directly from model.actions. new() maps both vectors and checks
        // uniqueness. The private map stays unchanged throughout sampling.
        let from = self.groups.get(&previous).copied().ok_or_else(|| {
            dive_eval::EvalError::bug("simulation previous action is missing from its flow groups")
        })?;
        let to = self.groups.get(&chosen).copied().ok_or_else(|| {
            dive_eval::EvalError::bug("simulation chosen action is missing from its flow groups")
        })?;
        let count = self.transitions.entry((from, to)).or_default();
        // Each counted edge consumes one retained state, so max_retained bounds
        // both counts and sparse entries. Still check overflow for public options.
        *count = count.checked_add(1).ok_or_else(|| {
            dive_eval::EvalError::resource_limit("simulation action-flow count overflow")
        })?;
        Ok(())
    }
}
