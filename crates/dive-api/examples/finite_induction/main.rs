//! Enumerate concrete finite predicates and certify their source obligations.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use dive_eval::St;
use dive_eval::next::{Action, Flow, StateVals};
use dive_ir::actions::ActionName;
use dive_model::{BindOptions, Model, Named};
use dive_trace::value_json::{from_json, to_json};
use serde_json::{Value, json};

const MAX_STATES: usize = 100_000;
const MAX_GENERATED: u64 = 5_000_000;

fn load(module: &Path, cfg: &Path) -> Result<Model, String> {
    let analysis = dive_sema::analyze_file(module, &dive_sema::Options::default())
        .map_err(|error| error.to_string())?;
    if !analysis.accepted() {
        return Err(format!("rejected source: {:?}", analysis.diagnostics()));
    }
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|error| format!("{error:?}"))?);
    let text = std::fs::read_to_string(cfg).map_err(|error| error.to_string())?;
    let cfg = dive_config::parse(&text)
        .into_config()
        .ok_or("invalid finite safety configuration")?;
    if cfg.specification.is_some()
        || cfg.symmetry.is_some()
        || cfg.view.is_some()
        || cfg.alias.is_some()
        || !cfg.action_constraints.is_empty()
        || !cfg.properties.is_empty()
        || !cfg.postconditions.is_empty()
        || cfg.periodic.is_some()
        || cfg.rl_reward.is_some()
        || !cfg.possible.is_empty()
        || cfg.constants.iter().any(|entry| {
            !matches!(
                entry,
                dive_config::ConstantEntry::Assignment { args, module: None, .. } if args.is_empty()
            )
        })
    {
        return Err(
            "expected unreduced INIT/NEXT safety configuration with value constants".into(),
        );
    }
    let model = dive_model::bind_reachable(ir, &cfg, &BindOptions::dive())
        .map_err(|error| format!("{error:?}"))?;
    if !model.output.is_empty()
        || !model.assumptions.is_empty()
        || !model.unsupported.is_empty()
        || model.stateful_checking().is_some()
        || model.invariants.is_empty()
        || model.init_actions.is_empty()
        || model.actions.is_empty()
        || model
            .invariants
            .iter()
            .chain(&model.constraints)
            .any(|role| model.evaluator.scope_has_effects(&role.pred.scope))
        || model
            .actions
            .iter()
            .chain(&model.init_actions)
            .any(|action| model.evaluator.action_shares_side_effects(action))
    {
        return Err("unsupported finite induction question or an impure bound role".into());
    }
    Ok(model)
}

fn variables(model: &Model) -> Vec<&str> {
    model
        .ir
        .vars()
        .iter()
        .map(|decl| model.ir.decl(*decl).name.as_str())
        .collect()
}

fn action_manifest(model: &Model, action: &Action) -> Result<Value, String> {
    let ActionName::Def(id) = action.name() else {
        return Err("an action has no resolved definition name".into());
    };
    let ctx = model.evaluator.options().ctx;
    let bindings = action
        .params()
        .iter()
        .map(|(name, value)| Ok(json!([name, to_json(value, ctx)?])))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(
        json!({"name": model.ir.def(id).name.as_str(), "bindings": bindings, "label": action.label()}),
    )
}

fn state_json(model: &Model, row: &StateVals) -> Result<Value, String> {
    let ctx = model.evaluator.options().ctx;
    row.iter()
        .map(|slot| to_json(slot.as_ref().ok_or("incomplete state")?, ctx))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn decode(model: &Model, row: &Value) -> Result<StateVals, String> {
    let ctx = model.evaluator.options().ctx;
    let values = row.as_array().ok_or("expected a finite predicate row")?;
    if values.len() != model.ir.vars().len() {
        return Err("wrong width in finite predicate".into());
    }
    values
        .iter()
        .map(|v| from_json(v, &model.names, ctx).map(Some))
        .collect()
}

fn satisfies(model: &Model, propositions: &[Named], row: &StateVals) -> Result<(), String> {
    let state = St::state(Arc::clone(row));
    for proposition in propositions {
        if !model
            .eval_boolean(&proposition.pred, &state)
            .map_err(|error| format!("{error:?}"))?
        {
            return Err(format!("false predicate {}", proposition.name));
        }
    }
    Ok(())
}

fn admitted(model: &Model, row: &StateVals) -> Result<bool, String> {
    let state = St::state(Arc::clone(row));
    for constraint in &model.constraints {
        if !model
            .eval_boolean(&constraint.pred, &state)
            .map_err(|error| format!("{error:?}"))?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Default)]
struct Coverage {
    generated: u64,
    initial: u64,
    outside: u64,
    expansions: usize,
    action: Vec<(u64, u64)>,
}

impl Coverage {
    fn emit(&mut self) -> Result<(), String> {
        self.generated += 1;
        if self.generated > MAX_GENERATED {
            return Err("finite induction generated-state limit".into());
        }
        Ok(())
    }
}

struct Candidate {
    rows: Vec<StateVals>,
    members: HashSet<StateVals>,
    fixed: bool,
}

impl Candidate {
    fn new(model: &Model, encoded: Option<&[Value]>) -> Result<Self, String> {
        let mut candidate = Self {
            rows: Vec::new(),
            members: HashSet::new(),
            fixed: encoded.is_some(),
        };
        if let Some(encoded) = encoded {
            if encoded.is_empty() || encoded.len() > MAX_STATES {
                return Err("invalid number of finite predicate rows".into());
            }
            for raw in encoded {
                let row = decode(model, raw)?;
                if !admitted(model, &row)? || !candidate.members.insert(Arc::clone(&row)) {
                    return Err("ineligible or duplicate finite predicate row".into());
                }
                satisfies(model, &model.invariants, &row)
                    .map_err(|reason| format!("target implication: {reason}"))?;
                candidate.rows.push(row);
            }
        }
        Ok(candidate)
    }

    fn require_or_extend(&mut self, row: StateVals, reason: &str) -> Result<(), String> {
        if self.fixed {
            if !self.members.contains(&row) {
                return Err(reason.to_owned());
            }
        } else if self.members.insert(Arc::clone(&row)) {
            if self.rows.len() == MAX_STATES {
                return Err("finite induction state limit".into());
            }
            self.rows.push(row);
        }
        Ok(())
    }
}

fn initial(
    model: &Model,
    candidate: &mut Candidate,
    coverage: &mut Coverage,
) -> Result<(), String> {
    let mut failure: Result<(), String> = Ok(());
    model
        .evaluator
        .init_states(&model.init_actions, &mut |edge| {
            failure = (|| {
                coverage.emit()?;
                coverage.initial += 1;
                let row: StateVals = edge.state.into();
                if !edge.complete() {
                    return Err("incomplete Init".into());
                }
                satisfies(model, &model.invariants, &row)?;
                if admitted(model, &row)? {
                    candidate.require_or_extend(row, "Init inclusion")?;
                }
                Ok(())
            })();
            if failure.is_ok() {
                Flow::Continue
            } else {
                Flow::Stop
            }
        })
        .map_err(|error| format!("Init generation: {error:?}"))?;
    failure?;
    if coverage.initial == 0 || candidate.rows.is_empty() {
        return Err("empty finite initial question".into());
    }
    Ok(())
}

fn exact_cover(model: &Model, encoded: Option<&[Value]>) -> Result<Value, String> {
    let mut coverage = Coverage {
        action: vec![(0, 0); model.actions.len()],
        ..Coverage::default()
    };
    let mut candidate = Candidate::new(model, encoded)?;
    initial(model, &mut candidate, &mut coverage)?;
    while let Some(source) = candidate.rows.get(coverage.expansions).cloned() {
        let mut ordinal = 0;
        for (index, action) in model.actions.iter().enumerate() {
            let mut failure: Result<(), String> = Ok(());
            let outcome = model
                .evaluator
                .next_states(action, &source, ordinal, &mut |edge| {
                    failure = (|| {
                        coverage.emit()?;
                        coverage.action[index].0 += 1;
                        let row: StateVals = edge.state.into();
                        if !edge.complete() {
                            return Err("incomplete successor".into());
                        }
                        // The claimed safety statement includes generated boundary states.
                        satisfies(model, &model.invariants, &row)
                            .map_err(|reason| format!("target successor: {reason}"))?;
                        if !admitted(model, &row)? {
                            coverage.outside += 1;
                            coverage.action[index].1 += 1;
                            return Ok(());
                        }
                        candidate.require_or_extend(row, "action closure")
                    })();
                    if failure.is_ok() {
                        Flow::Continue
                    } else {
                        Flow::Stop
                    }
                })
                .map_err(|error| format!("{}: {error:?}", action.label()))?;
            failure.map_err(|error| format!("{}: {error}", action.label()))?;
            ordinal += outcome.emitted;
        }
        if ordinal == 0 && model.settings.check_deadlock {
            return Err("deadlock at an admitted prestate".into());
        }
        coverage.expansions += 1;
    }
    if !model.take_output().is_empty() {
        return Err("finite induction produced diagnostics".into());
    }
    let actions = model
        .actions
        .iter()
        .zip(coverage.action)
        .map(|(action, (generated, outside))| {
            let mut manifest = action_manifest(model, action)?;
            manifest["generated"] = json!(generated);
            manifest["outside"] = json!(outside);
            Ok(manifest)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let states = if candidate.fixed {
        None
    } else {
        Some(
            candidate
                .rows
                .iter()
                .map(|row| state_json(model, row))
                .collect::<Result<Vec<_>, _>>()?,
        )
    };
    Ok(json!({
        "variables": variables(model), "actions": actions,
        "constraints": model.constraints.iter().map(|role| &role.name).collect::<Vec<_>>(),
        "targets": model.invariants.iter().map(|role| &role.name).collect::<Vec<_>>(),
        "boundary": "check-target-before-filter",
        "initial_generated": coverage.initial, "generated": coverage.generated,
        "expanded": coverage.expansions, "outside": coverage.outside, "states": states
    }))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (module, cfg, candidate) = match args.as_slice() {
        [module, cfg] => (module, cfg, None),
        [module, cfg, candidate] => {
            let bytes = std::fs::read(candidate).map_err(|error| error.to_string())?;
            if bytes.len() > 64 << 20 {
                return Err("finite predicate file limit".into());
            }
            let rows =
                serde_json::from_slice::<Vec<Value>>(&bytes).map_err(|error| error.to_string())?;
            (module, cfg, Some(rows))
        }
        _ => return Err("expected MODULE.tla MODEL.cfg [PREDICATE_ROWS.json]".into()),
    };
    let model = load(Path::new(module), Path::new(cfg))?;
    println!("{}", exact_cover(&model, candidate.as_deref())?);
    Ok(())
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(run)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "finite induction worker panicked".to_owned())?
}
