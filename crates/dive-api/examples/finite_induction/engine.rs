//! Enumerate concrete finite predicates and certify their source obligations.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use dive_eval::next::{Action, Flow, StateVals};
use dive_eval::{ConstBinding, St};
use dive_ir::actions::ActionName;
use dive_ir::{DefId, Expr, OpRef, StdOp};
use dive_model::{BindOptions, Model, Named};
use dive_trace::value_json::{from_json, to_json};
use dive_value::Value as TlaValue;
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
    let unsupported = [
        ("SPECIFICATION", cfg.specification.is_some()),
        ("SYMMETRY", cfg.symmetry.is_some()),
        ("VIEW", cfg.view.is_some()),
        ("ALIAS", cfg.alias.is_some()),
        ("ACTION_CONSTRAINT", !cfg.action_constraints.is_empty()),
        ("PROPERTY", !cfg.properties.is_empty()),
        ("POSTCONDITION", !cfg.postconditions.is_empty()),
        ("PERIODIC", cfg.periodic.is_some()),
        ("RL_REWARD", cfg.rl_reward.is_some()),
        ("POSSIBLE", !cfg.possible.is_empty()),
    ];
    if let Some((feature, _)) = unsupported.into_iter().find(|(_, present)| *present) {
        return Err(format!("{feature} is unsupported in finite induction"));
    }
    if cfg.constants.iter().any(|entry| {
        !matches!(
            entry,
            dive_config::ConstantEntry::Assignment { args, module: None, .. } if args.is_empty()
        )
    }) {
        return Err("finite induction requires unscoped value constant assignments".into());
    }
    let model = dive_model::bind_reachable(ir, &cfg, &BindOptions::dive())
        .map_err(|error| format!("{error:?}"))?;
    if let Some(unsupported) = model.unsupported.first() {
        return Err(unsupported.feature.clone());
    }
    if let Some(stateful) = model.stateful_checking() {
        return Err(format!("{stateful} is unsupported in finite induction"));
    }
    if !model.assumptions.is_empty() {
        return Err("ASSUME is unsupported in finite induction".into());
    }
    if !model.output.is_empty() || dive_runtime::admission::effectful(&model) {
        return Err(
            "finite induction requires pure bound definitions and no startup diagnostics".into(),
        );
    }
    if let Some(operator) = representation_observer(&model) {
        return Err(format!(
            "finite induction refuses representation-sensitive {}",
            operator.qualified_name()
        ));
    }
    if model.invariants.is_empty() || model.init_actions.is_empty() || model.actions.is_empty() {
        return Err("finite induction requires bound INIT, NEXT and INVARIANT roles".into());
    }
    Ok(model)
}

fn representation_observer(model: &Model) -> Option<StdOp> {
    let ir = &model.ir;
    let roots: Vec<_> = ir
        .defs()
        .iter()
        .enumerate()
        .filter(|(_, def)| !ir.module(def.module).standard)
        .filter_map(|(index, _)| u32::try_from(index).ok().map(DefId))
        .collect();
    let reachable = ir.reachable_defs(&roots);
    let observes = |op| {
        matches!(
            op,
            StdOp::TlcToString
                | StdOp::JsonToJson
                | StdOp::JsonToJsonArray
                | StdOp::JsonToJsonObject
                | StdOp::FiniteSetsIsFiniteSet
                | StdOp::FunctionsIsInjective
        )
    };

    let mut pending = Vec::new();
    for (definition, used) in ir.defs().iter().zip(reachable) {
        if used {
            if let Some(op) = definition.std {
                if observes(op) {
                    return Some(op);
                }
            } else {
                pending.push(definition.body);
            }
        }
    }
    let mut visited = HashSet::new();
    while let Some(expr) = pending.pop() {
        if !visited.insert(expr) {
            continue;
        }
        let expr = ir.expr(expr);
        if let Some(op) = expr.op_refs().into_iter().find_map(|op| match op {
            OpRef::Std(operator) if observes(*operator) => Some(*operator),
            _ => None,
        }) {
            return Some(op);
        }
        if let Expr::Def(id) = expr
            && let Some(op) = ir.def(*id).std.filter(|op| observes(*op))
        {
            return Some(op);
        }
        expr.for_each_child(|child| pending.push(child));
    }
    None
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
    retained_boundary: Option<HashSet<StateVals>>,
}

impl Candidate {
    fn new(
        model: &Model,
        rows: Option<Vec<StateVals>>,
        retain_boundary: bool,
    ) -> Result<Self, String> {
        let mut candidate = Self {
            rows: Vec::new(),
            members: HashSet::new(),
            fixed: rows.is_some(),
            retained_boundary: retain_boundary.then(HashSet::new),
        };
        if let Some(rows) = rows {
            if rows.is_empty() || rows.len() > MAX_STATES {
                return Err("invalid number of finite predicate rows".into());
            }
            for row in rows {
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

    fn retain_boundary(&mut self, row: StateVals) -> Result<(), String> {
        if let Some(boundary) = &mut self.retained_boundary {
            boundary.insert(row);
            if boundary.len() > MAX_STATES {
                return Err("finite induction boundary row limit".into());
            }
        }
        Ok(())
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
                } else {
                    candidate.retain_boundary(row)?;
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

fn exact_cover(model: &Model, rows: Option<Vec<StateVals>>, packed: bool) -> Result<Value, String> {
    let mut coverage = Coverage {
        action: vec![(0, 0); model.actions.len()],
        ..Coverage::default()
    };
    let mut candidate = Candidate::new(model, rows, packed)?;
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
                            return candidate.retain_boundary(row);
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
    let states = if candidate.fixed || packed {
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
        "table": candidate.retained_boundary.as_ref()
            .map(|boundary| encode_table(model, &candidate.rows, boundary))
            .transpose()?,
        "variables": variables(model), "actions": actions,
        "constraints": model.constraints.iter().map(|role| &role.name).collect::<Vec<_>>(),
        "targets": model.invariants.iter().map(|role| &role.name).collect::<Vec<_>>(),
        "boundary": "check-target-before-filter",
        "initial_generated": coverage.initial, "generated": coverage.generated,
        "expanded": coverage.expansions, "outside": coverage.outside, "states": states
    }))
}

type Column = HashMap<TlaValue, usize>;

fn encode_codes(rows: &[StateVals], columns: &mut [Column]) -> Result<Vec<Vec<usize>>, String> {
    rows.iter()
        .map(|row| {
            row.iter()
                .zip(columns.iter_mut())
                .map(|(value, column)| {
                    let value = value.as_ref().ok_or("incomplete retained value")?;
                    let next = column.len();
                    Ok(*column.entry(value.clone()).or_insert(next))
                })
                .collect()
        })
        .collect()
}

#[allow(
    clippy::mutable_key_type,
    reason = "semantic value keys are immutable; only their memoized hashes change"
)]
fn encode_table(
    model: &Model,
    inside: &[StateVals],
    outside: &HashSet<StateVals>,
) -> Result<Value, String> {
    let mut boundary: Vec<_> = outside.iter().cloned().collect();
    let ctx = model.evaluator.options().ctx;
    // A stable encoding must exclude HashSet iteration order.
    boundary.sort_by(|left, right| {
        for (left, right) in left.iter().zip(right.iter()) {
            let order = match (left, right) {
                (Some(left), Some(right)) => left.cmp_with(right, ctx.order),
                _ => left.is_some().cmp(&right.is_some()),
            };
            if order != std::cmp::Ordering::Equal {
                return order;
            }
        }
        std::cmp::Ordering::Equal
    });
    let mut columns = vec![HashMap::new(); model.ir.vars().len()];
    let inside = encode_codes(inside, &mut columns)?;
    let boundary = encode_codes(&boundary, &mut columns)?;
    let mut dictionaries = Vec::new();
    for column in columns {
        let mut ordered = vec![Value::Null; column.len()];
        for (value, index) in column {
            ordered[index] = to_json(&value, ctx)?;
        }
        dictionaries.push(ordered);
    }
    Ok(json!({"columns": dictionaries, "inside": inside, "boundary": boundary}))
}

fn read_json(path: &Path, limit: u64) -> Result<Value, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("finite predicate file limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn decode_table(model: &Model, table: &Value, boundary: bool) -> Result<Vec<StateVals>, String> {
    let ctx = model.evaluator.options().ctx;
    let values = table["columns"]
        .as_array()
        .ok_or("missing finite predicate columns")?;
    if values.len() != model.ir.vars().len() {
        return Err("wrong number of finite predicate columns".into());
    }
    let columns = values
        .iter()
        .map(|column| {
            let values = column.as_array().ok_or("invalid finite predicate column")?;
            if values.len() > 2 * MAX_STATES {
                return Err("finite predicate column limit".into());
            }
            values
                .iter()
                .map(|value| from_json(value, &model.names, ctx))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, String>>()?;
    let raw = table[if boundary { "boundary" } else { "inside" }]
        .as_array()
        .ok_or("missing finite predicate rows")?;
    if raw.len() > MAX_STATES {
        return Err("finite predicate row limit".into());
    }
    raw.iter()
        .map(|code| {
            let code = code.as_array().ok_or("expected finite predicate code")?;
            if code.len() != columns.len() {
                return Err("wrong width in finite predicate code".into());
            }
            code.iter()
                .zip(&columns)
                .map(|(value, dictionary)| {
                    let index = value.as_u64().and_then(|index| usize::try_from(index).ok());
                    index
                        .and_then(|index| dictionary.get(index))
                        .cloned()
                        .map(Some)
                        .ok_or_else(|| "invalid finite predicate index".to_owned())
                })
                .collect()
        })
        .collect()
}

fn bound_question(model: &Model) -> Result<Value, String> {
    let ctx = model.evaluator.options().ctx;
    let constants = model
        .ir
        .consts()
        .iter()
        .map(|decl| {
            let decl = model.ir.decl(*decl);
            let id = decl.constant.ok_or("expected constant declaration")?;
            let Some(ConstBinding::Value(value)) = model.evaluator.constant_binding(id) else {
                return Err("expected bound constant value".into());
            };
            to_json(&value, ctx)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut value_overrides = Vec::new();
    for (index, _) in model.ir.defs().iter().enumerate() {
        let id = DefId(u32::try_from(index).map_err(|error| error.to_string())?);
        if model.evaluator.redirected(id) != id {
            return Err("operator redirection is unsupported in finite induction".into());
        }
        if let Some(value) = model.evaluator.value_override(id) {
            value_overrides.push((id, to_json(&value, ctx)?));
        }
    }
    let pred = |pred: &dive_model::Pred| (pred.expr, pred.scope.def());
    let actions = model
        .actions
        .iter()
        .map(|action| action_manifest(model, action))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "boundary": "check-target-before-filter", "profile": "dive",
        "ir": &*model.ir, "constants": constants, "value_overrides": value_overrides,
        "variables": variables(model),
        "Init": model.init.iter().map(|init| pred(&init.pred)).collect::<Vec<_>>(),
        "Next": model.next.as_ref().map(pred),
        "constraints": model.constraints.iter().map(|role| (&role.name, pred(&role.pred))).collect::<Vec<_>>(),
        "deadlock": model.settings.check_deadlock,
        "actions": actions,
    }))
}

fn target_checks(model: &Model, table: &Value) -> Result<Value, String> {
    let (inside, boundary) = (
        decode_table(model, table, false)?,
        decode_table(model, table, true)?,
    );
    if inside.is_empty() {
        return Err("empty finite predicate".into());
    }
    for row in inside.iter().chain(&boundary) {
        satisfies(model, &model.invariants, row)?;
    }
    if !model.take_output().is_empty() {
        return Err("finite target evaluation produced diagnostics".into());
    }
    Ok(json!({"checked": model.invariants.len()}))
}

fn target_manifest(model: &Model) -> Value {
    json!(
        model
            .invariants
            .iter()
            .map(|role| {
                let pred = &role.pred;
                json!({"name": role.name, "expr": pred.expr, "scope": pred.scope.def(),
              "source_tokens": model.ir.human_readable_image(pred.expr)})
            })
            .collect::<Vec<_>>()
    )
}

fn run(args: &[OsString]) -> Result<(), String> {
    let report = match args {
        [flag, module, cfg] if flag == "--finite-produce" => {
            let model = load(Path::new(module), Path::new(cfg))?;
            let mut report = exact_cover(&model, None, true)?;
            report["binding"] = bound_question(&model)?;
            report["producer_targets"] = target_manifest(&model);
            report
        }
        [flag, module, cfg, candidate]
            if flag == "--finite-check"
                || flag == "--finite-targets"
                || flag == "--finite-freeze" =>
        {
            let model = load(Path::new(module), Path::new(cfg))?;
            let table = read_json(Path::new(candidate), 16 << 20)?;
            let mut report = if flag == "--finite-targets" {
                target_checks(&model, &table)?
            } else {
                exact_cover(
                    &model,
                    Some(decode_table(&model, &table, false)?),
                    flag == "--finite-freeze",
                )?
            };
            report["binding"] = bound_question(&model)?;
            report["producer_targets"] = target_manifest(&model);
            report
        }
        [module, cfg] => {
            let model = load(Path::new(module), Path::new(cfg))?;
            exact_cover(&model, None, false)?
        }
        [module, cfg, rows] => {
            let model = load(Path::new(module), Path::new(cfg))?;
            let raw = read_json(Path::new(rows), 64 << 20)?;
            let rows = raw.as_array().ok_or("expected finite predicate rows")?;
            let rows = rows
                .iter()
                .map(|row| decode(&model, row))
                .collect::<Result<Vec<_>, _>>()?;
            exact_cover(&model, Some(rows), false)?
        }
        _ => return Err("expected MODULE.tla MODEL.cfg [PREDICATE_ROWS.json]".into()),
    };
    println!("{report}");
    Ok(())
}

pub(super) fn entry(args: &[OsString]) -> Result<(), String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, move || run(args))
            .map_err(|error| error.to_string())?
            .join()
            .map_err(|_| "finite induction worker panicked".to_owned())?
    })
}
