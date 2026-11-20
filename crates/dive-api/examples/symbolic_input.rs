//! Export bound finite safety roles for the optional symbolic checker.
//!
//! Build this helper and the validator from the same checkout as
//! `scripts/symbolic/check.py`. Its output is a private interchange format.

use std::{env, fs, path::Path, sync::Arc};

use dive_eval::ConstBinding;
use dive_ir::DefId;
use serde_json::{Value, json};

fn role(m: &dive_model::Model, p: &dive_model::Pred) -> Result<dive_ir::ExprId, String> {
    let def = m.ir.def(p.scope.def());
    if p.scope.has_substitutions() || def.source.is_some() || !def.params.is_empty() {
        return Err("unsupported captured role".to_string());
    }
    for i in 0..def.frame.len() {
        let slot = dive_ir::Slot(u32::try_from(i).map_err(|e| e.to_string())?);
        let view = m
            .evaluator
            .scope_slot(&p.scope, dive_ir::LocalRef { up: 0, slot });
        if !matches!(view, dive_eval::SlotView::Unset) {
            return Err("unsupported bound role slot".into());
        }
    }
    Ok(p.expr)
}

fn run(module: &Path, config: &Path) -> Result<Value, String> {
    let analysis = dive_sema::analyze_file(module, &dive_sema::Options::default())
        .map_err(|e| e.to_string())?;
    if !analysis.accepted() {
        return Err(format!("semantic analysis: {:?}", analysis.diagnostics()));
    }
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| format!("{e:?}"))?);
    let cfg = dive_config::parse(&fs::read_to_string(config).map_err(|e| e.to_string())?)
        .into_config()
        .ok_or("invalid configuration")?;
    if !cfg.properties.is_empty()
        || !cfg.constraints.is_empty()
        || !cfg.action_constraints.is_empty()
        || cfg.view.is_some()
        || cfg.alias.is_some()
        || !cfg.postconditions.is_empty()
        || cfg.periodic.is_some()
        || cfg.rl_reward.is_some()
        || !cfg.possible.is_empty()
    {
        return Err("symbolic checking admits safety invariants and deadlock only".into());
    }
    if cfg.constants.iter().any(|entry| {
        !matches!(entry,
        dive_config::ConstantEntry::Assignment { args, module: None, .. } if args.is_empty())
    }) {
        return Err("unsupported scoped or operator constant binding".into());
    }
    let m = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
        .map_err(|e| format!("{e:?}"))?;
    let symmetry = m
        .prepare_symmetry(
            &cfg,
            dive_model::symmetry::Mode::Checked,
            dive_model::symmetry::Limits::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    if symmetry.as_ref().is_some_and(|p| !p.warnings.is_empty()) {
        return Err("symmetry startup warning".into());
    }
    let symmetry_flag = m.symmetry.as_ref().map(|name| format!("SYMMETRY {name}"));
    if let Some(reason) = m
        .unsupported
        .iter()
        .find(|reason| Some(&reason.feature) != symmetry_flag.as_ref())
    {
        return Err(reason.feature.clone());
    }
    if !m.output.is_empty() {
        return Err(format!("unsupported startup diagnostic {:?}", m.output));
    }
    if !m.temporals.is_empty()
        || !m.implied_actions.is_empty()
        || !m.implied_inits.is_empty()
        || !m.implied_temporals.is_empty()
    {
        return Err("temporal or implied obligations".into());
    }
    if !m.assumptions.is_empty() || m.init.len() != 1 || m.stateful_checking().is_some() {
        return Err(
            "unsupported assumption, initial-role multiplicity or stateful operator".into(),
        );
    }
    let init = role(&m, &m.init[0].pred)?;
    let next = role(&m, m.next.as_ref().ok_or("missing NEXT")?)?;
    let invariants = m
        .invariants
        .iter()
        .map(|x| Ok(json!([x.name, role(&m, &x.pred)?])))
        .collect::<Result<Vec<_>, String>>()?;
    let ctx = m.evaluator.options().ctx;
    let permutations = symmetry.and_then(|p| p.reduction).map(|r| json!({
        "atoms": r.universe().iter().map(dive_value::Atom::as_str).collect::<Vec<_>>(),
        "maps": r.maps().iter().map(dive_model::symmetry::Permutation::images).collect::<Vec<_>>()
    }));
    let mut constants = Vec::new();
    for &decl in ir.consts() {
        let c = ir.decl(decl);
        let Some(ConstBinding::Value(v)) =
            m.evaluator.constant_binding(c.constant.ok_or("constant")?)
        else {
            return Err(format!("unsupported constant binding {}", c.name));
        };
        constants.push(dive_trace::value_json::to_json(&v, ctx)?);
    }
    let mut overrides = serde_json::Map::new();
    for (i, _) in ir.defs().iter().enumerate() {
        let id = DefId(u32::try_from(i).map_err(|e| e.to_string())?);
        if m.evaluator.redirected(id) != id {
            return Err("unsupported operator substitution".into());
        }
        if let Some(v) = m.evaluator.value_override(id) {
            overrides.insert(i.to_string(), dive_trace::value_json::to_json(&v, ctx)?);
        }
    }
    Ok(
        json!({"format": "dive-symbolic-input-v1", "ir": &*ir, "constants": constants, "overrides": overrides,
        "init": init, "next": next, "invariants": invariants, "symmetry": permutations,
        "specification": cfg.specification.as_ref().map(|n| &n.text),
        "check_deadlock": m.settings.check_deadlock}),
    )
}

fn main() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let [module, cfg] = args.as_slice() else {
        return Err("expected MODULE.tla MODEL.cfg".into());
    };
    println!("{}", run(Path::new(module), Path::new(cfg))?);
    Ok(())
}
