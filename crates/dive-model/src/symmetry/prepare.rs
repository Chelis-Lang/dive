//! Tool-level symmetry preparation and concrete-state canonical keys.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dive_config::{ConstantEntry, ModelConfig};
use dive_eval::{ConstBinding, St};
use dive_ir::{Arg, Expr, ExprId, OpRef, StdOp};
use dive_value::{Atom, Ctx, SetKind, Value, ValueOrder};

use super::{Declaration, Group, Limits, Mode, Permutation, symmetry_runtime_error};
use crate::{BindError, Ec, Message, Model, Pred};

/// Complete pure tool-level preparation, before attachment to a safety checker.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The bounded closure; `None` means TLC's too-small heuristic disabled it.
    pub reduction: Option<Arc<Reduction>>,
    /// Startup warnings, in TLC's order.
    pub warnings: Vec<Message>,
}

/// A bounded symmetry closure over the model values it mentions.
/// State canonicalization changes keys only: the checker explores and retains
/// the original concrete state and predecessor.
#[derive(Clone, Debug)]
pub struct Reduction {
    universe: Vec<Atom>,
    indices: HashMap<Atom, usize>,
    group: Group,
}

impl Model {
    /// Prepares a pure declaration using `Tool.getSymmetryPerms`'s ordering:
    /// representations, too-small warnings, domain/range checks, closure,
    /// then unused argument warnings. `cfg` must be the bound configuration.
    /// This API does not itself activate reduction or remove checker refusals.
    ///
    /// # Example
    /// ```
    /// # use std::sync::Arc;
    /// # use dive_model::{BindOptions, bind};
    /// # use dive_model::symmetry::{Mode, Limits};
    /// let source = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT S\nVARIABLE x\n\
    ///               Init == x = TRUE\nNext == UNCHANGED x\n\
    ///               Sym == Permutations(S)\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    /// let cfg = dive_config::parse("CONSTANT S={a,b}\nINIT Init\nNEXT Next\nSYMMETRY Sym\n")
    ///     .into_config().unwrap();
    /// let model = bind(ir, &cfg, &BindOptions::tlc()).unwrap();
    /// let prepared = model.prepare_symmetry(&cfg, Mode::Tlc, Limits::default()).unwrap().unwrap();
    /// assert!(prepared.reduction.is_some());
    /// assert!(prepared.warnings.is_empty());
    /// ```
    ///
    /// # Errors
    /// Invalid declarations, unavailable warning source evidence, unverified
    /// startup exceptions, or a bounded-closure refusal.
    pub fn prepare_symmetry(
        &self,
        cfg: &ModelConfig,
        mode: Mode,
        limits: Limits,
    ) -> Result<Option<Prepared>, BindError> {
        let Some(declaration) = self.symmetry_declaration()? else {
            return Ok(None);
        };
        let args = argument_nodes(self.ir.expr(declaration.predicate.expr))?;
        if !args.is_empty() && args.len() >= declaration.functions.len() {
            // The approved default rejects non-bijective model-value maps even
            // when TLC disables them. Non-model identities retain TLC's warning
            // shortcut; extending that default rejection needs owner approval.
            // A duplicate generator supplies the kernel's capacity precondition
            // without altering its closure; no reduction is attached here.
            if mode == Mode::Checked
                && !declaration.functions.is_empty()
                && declaration.functions.iter().all(|value| match value {
                    Value::Fcn(function) => function.iter().all(|(domain, range)| {
                        matches!(domain, Value::Model(_)) && matches!(range, Value::Model(_))
                    }),
                    _ => false,
                })
            {
                let mut checked = declaration.clone();
                if checked.functions.len() == 1 {
                    checked.functions =
                        vec![checked.functions[0].clone(), checked.functions[0].clone()].into();
                }
                prepare_group(&checked, mode, limits)?;
            }
            return Ok(Some(Prepared {
                reduction: None,
                warnings: vec![warning(self, cfg, &args)?],
            }));
        }
        let reduction = prepare_group(&declaration, mode, limits)?;
        let members: HashSet<Atom> = reduction
            .group
            .maps()
            .iter()
            .flat_map(|map| {
                map.images()
                    .iter()
                    .enumerate()
                    .filter(|(index, image)| index != *image)
                    .map(|(_, image)| reduction.universe[*image].clone())
            })
            .collect();
        let mut unused = Vec::new();
        for arg in args {
            if let Some(values) = permutation_argument(self, &declaration, arg)?
                && !values
                    .iter()
                    .any(|value| matches!(value, Value::Model(m) if members.contains(m)))
            {
                unused.push(arg);
            }
        }
        Ok(Some(Prepared {
            reduction: Some(Arc::new(reduction)),
            warnings: if unused.is_empty() {
                Vec::new()
            } else {
                vec![warning(self, cfg, &unused)?]
            },
        }))
    }
}

fn argument_nodes(expr: &Expr) -> Result<Vec<ExprId>, BindError> {
    match expr {
        Expr::Def(_) | Expr::Const(_) | Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) => {
            Ok(Vec::new())
        }
        Expr::Builtin { args, .. }
        | Expr::SetEnum(args)
        | Expr::Tuple(args)
        | Expr::Cartesian(args) => Ok(args.to_vec()),
        Expr::Apply { args, .. } => args
            .iter()
            .map(|arg| match arg {
                Arg::Expr { expr: id, .. } => Ok(*id),
                Arg::Op { .. } => Err(BindError::unsupported(
                    "SYMMETRY warning arguments containing an operator argument",
                )),
            })
            .collect(),
        Expr::If { cond, then, other } => Ok(vec![*cond, *then, *other]),
        Expr::Junction { items, .. } => Ok(items.to_vec()),
        _ => Err(BindError::unsupported(
            "SYMMETRY declaration syntax whose TLC argument/warning shape is not implemented",
        )),
    }
}

fn warning(model: &Model, cfg: &ModelConfig, args: &[ExprId]) -> Result<Message, BindError> {
    let mut names = Vec::new();
    for arg in args {
        let image = model
            .ir
            .human_readable_image(*arg)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                BindError::unsupported("SYMMETRY warning lacks original source tokens")
            })?;
        let alias = if image.starts_with("Permutations") {
            image
                .get("Permutations".len() + 1..image.len().saturating_sub(1))
                .ok_or_else(|| BindError::unsupported("SYMMETRY warning permutation image"))?
        } else {
            &image
        };
        let original = cfg
            .constants
            .iter()
            .rev()
            .find_map(|entry| match entry {
                ConstantEntry::Substitution {
                    name,
                    target,
                    module: None,
                } if target.text == alias => Some(name.text.as_str()),
                _ => None,
            })
            .unwrap_or(alias);
        names.push(original.to_owned());
    }
    let many = args.len() > 1;
    let parameters = [
        if many { "s" } else { "" },
        &names.join(", and "),
        if many { "have" } else { "has" },
        if many { "" } else { "s" },
    ];
    Ok(Message::Warning {
        code: Ec::TlcSymmetrySetTooSmall,
        message: Ec::TlcSymmetrySetTooSmall.render(
            &parameters
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
        ),
    })
}

fn permutation_argument(
    model: &Model,
    declaration: &Declaration,
    arg: ExprId,
) -> Result<Option<Arc<[Value]>>, BindError> {
    let Expr::Apply { callee, args } = model.ir.expr(arg) else {
        return Ok(None);
    };
    let op = match callee {
        OpRef::Def(def) => model.ir.def(*def).std,
        OpRef::Std(op) => Some(*op),
        _ => None,
    };
    if op != Some(StdOp::TlcPermutations) {
        return Ok(None);
    }
    let [Arg::Expr { expr: domain, .. }] = args.as_ref() else {
        return Ok(None);
    };
    let value = match model.ir.expr(*domain) {
        Expr::Const(constant) => match model.evaluator.constant_binding(*constant) {
            Some(ConstBinding::Value(value)) => value,
            _ => return Ok(None),
        },
        Expr::Def(_) => model
            .eval(
                &Pred::new(*domain, declaration.predicate.scope.clone()),
                &St::constant(),
            )
            .map_err(|error| {
                BindError::unsupported(format!("SYMMETRY unused argument: {}", error.message()))
            })?,
        Expr::SubstRef { .. } => {
            return Err(BindError::unsupported(
                "SYMMETRY unused-set warning under instance substitutions",
            ));
        }
        _ => return Ok(None),
    };
    if let Value::Set(set) = value
        && matches!(set.kind(), SetKind::Enum | SetKind::Raw)
    {
        return set
            .elements(model.evaluator.options().ctx)
            .map(Some)
            .map_err(|error| {
                BindError::unsupported(format!(
                    "SYMMETRY unused-set normalization: {}",
                    error.message()
                ))
            });
    }
    Ok(None)
}

fn prepare_group(
    declaration: &Declaration,
    mode: Mode,
    limits: Limits,
) -> Result<Reduction, BindError> {
    let mut universe = Vec::new();
    let mut indices = HashMap::new();
    let mut generators = Vec::new();
    let mut work = 0usize;
    for value in declaration.functions.iter() {
        let Value::Fcn(fcn) = value else {
            return Err(BindError::bug(
                "validated symmetry function ceased to be a function",
            ));
        };
        let mut pairs = Vec::new();
        for (domain, range) in fcn.iter() {
            work = work
                .checked_add(1)
                .ok_or_else(|| BindError::unsupported("SYMMETRY generator work limit"))?;
            if work > limits.work {
                return Err(BindError::unsupported("SYMMETRY generator work limit"));
            }
            let (Value::Model(domain), Value::Model(range)) = (&domain, range) else {
                return Err(symmetry_runtime_error(
                    "Symmetry function must have model values as domain and range.",
                ));
            };
            let mut ids = [0usize; 2];
            for (at, atom) in [domain, range].into_iter().enumerate() {
                ids[at] = if let Some(index) = indices.get(atom) {
                    *index
                } else {
                    if universe.len() >= limits.model_values {
                        return Err(BindError::unsupported("SYMMETRY model-value limit"));
                    }
                    let index = universe.len();
                    universe.push(atom.clone());
                    indices.insert(atom.clone(), index);
                    index
                };
            }
            pairs.push((ids[0], ids[1]));
        }
        generators.push(pairs);
    }
    let group =
        Group::from_generators(universe.len(), &generators, mode, limits).map_err(|error| {
            BindError::unsupported(format!("SYMMETRY {}: {error}", declaration.name))
        })?;
    Ok(Reduction {
        universe,
        indices,
        group,
    })
}

impl Reduction {
    /// Canonical values in the supplied variable order. The original values
    /// remain the explored state. TLC mode compares each candidate in TLC's
    /// argument order; canonical mode uses its approved total value order.
    ///
    /// # Errors
    /// Values outside the implemented pure permutation representations, value
    /// comparison errors, or the per-state permutation work/depth limit.
    pub fn canonical_values(
        &self,
        values: &[Value],
        variable_order: &[usize],
        ctx: Ctx,
    ) -> Result<Vec<Value>, BindError> {
        if variable_order.len() != values.len()
            || variable_order
                .iter()
                .enumerate()
                .any(|(at, index)| *index >= values.len() || variable_order[..at].contains(index))
        {
            return Err(BindError::bug(
                "symmetry variable order is not a permutation of the state slots",
            ));
        }
        let mut best = values.to_vec();
        let mut work = 0usize;
        // Reuse the two complete state rows, as TLC does. An early-abandoned
        // row is overwritten before it can become the new minimum.
        let mut candidate = values.to_vec();
        for map in self.group.maps() {
            let mut smaller = false;
            for &index in variable_order {
                let value = values
                    .get(index)
                    .ok_or_else(|| BindError::bug("symmetry variable order outside state"))?;
                candidate[index] = self.permute(value, map, ctx, 0, &mut work)?;
                if !smaller {
                    let ordering = if ctx.order == ValueOrder::Tlc {
                        dive_value::tlc::compare(&candidate[index], &best[index], ctx).map_err(
                            |error| {
                                BindError::unsupported(format!(
                                    "SYMMETRY candidate comparison: {}",
                                    error.message()
                                ))
                            },
                        )?
                    } else {
                        candidate[index].cmp_with(&best[index], ctx.order)
                    };
                    match ordering {
                        Ordering::Greater => break,
                        Ordering::Less => smaller = true,
                        Ordering::Equal => {}
                    }
                }
            }
            if smaller {
                std::mem::swap(&mut best, &mut candidate);
            }
        }
        Ok(best)
    }

    /// TLC deep-normalizes the concrete source after selecting another
    /// representative, and before evaluating a VIEW even when identity wins.
    /// This preserves normalization observed by subsequent enumeration.
    ///
    /// # Errors
    /// A value normalization error or the same bounded work/depth refusal.
    pub fn normalize_concrete(values: &[Value], ctx: Ctx) -> Result<(), BindError> {
        let mut work = 0usize;
        for value in values {
            deep_normalize(value, ctx, 0, &mut work)?;
        }
        Ok(())
    }

    fn permute(
        &self,
        value: &Value,
        map: &Permutation,
        ctx: Ctx,
        depth: usize,
        work: &mut usize,
    ) -> Result<Value, BindError> {
        *work = work
            .checked_add(1)
            .ok_or_else(|| BindError::unsupported("SYMMETRY permutation work limit"))?;
        if *work > 32_000_000 || depth > 256 {
            return Err(BindError::unsupported(
                "SYMMETRY per-state permutation work/depth limit",
            ));
        }
        let recur =
            |value: &Value, work: &mut usize| self.permute(value, map, ctx, depth + 1, work);
        let map_error = |error: dive_value::ValueError| {
            BindError::unsupported(format!("SYMMETRY value permutation: {}", error.message()))
        };
        match value {
            Value::Model(atom) => Ok(self
                .indices
                .get(atom)
                .and_then(|&i| map.image(i))
                .and_then(|i| self.universe.get(i))
                .map_or_else(|| value.clone(), |atom| Value::Model(atom.clone()))),
            Value::Fcn(function) => permute_function(function, ctx, |value| recur(value, work)),
            Value::Set(set) => match set.kind() {
                SetKind::Enum | SetKind::Raw => {
                    let elements = match set.raw_elements() {
                        Some(elements) => Arc::from(elements),
                        None => set.elements(ctx).map_err(map_error)?,
                    };
                    if elements.len() > 32_000_000usize.saturating_sub(*work) {
                        return Err(BindError::unsupported(
                            "SYMMETRY set permutation work limit",
                        ));
                    }
                    let permuted = elements
                        .iter()
                        .map(|value| recur(value, work))
                        .collect::<Result<Vec<_>, _>>()?;
                    if elements
                        .iter()
                        .zip(&permuted)
                        .all(|(before, after)| same_value(before, after))
                    {
                        Ok(value.clone())
                    } else {
                        Ok(Value::set_raw(permuted, ctx))
                    }
                }
                SetKind::Interval => Ok(value.clone()),
                _ => Err(BindError::unsupported(format!(
                    "SYMMETRY permutation of lazy {:?} values",
                    set.kind()
                ))),
            },
            _ => Ok(value.clone()),
        }
    }
}

fn permute_function(
    function: &dive_value::Fcn,
    ctx: Ctx,
    mut recur: impl FnMut(&Value) -> Result<Value, BindError>,
) -> Result<Value, BindError> {
    let map_error = |error: dive_value::ValueError| {
        BindError::unsupported(format!("SYMMETRY value permutation: {}", error.message()))
    };
    if let Some(tuple) = function.as_tuple() {
        let values = tuple
            .iter()
            .map(&mut recur)
            .collect::<Result<Vec<_>, _>>()?;
        if tuple
            .iter()
            .zip(&values)
            .all(|(before, after)| same_value(before, after))
        {
            return Ok(Value::Fcn(function.clone()));
        }
        return Ok(if function.is_tuple_literal() {
            Value::tuple(values)
        } else {
            Value::tuple_fcn(values)
        });
    }
    if function.is_record_literal() {
        let fields = function
            .as_record()
            .ok_or_else(|| BindError::bug("record has no fields"))?
            .iter()
            .map(|(name, value)| Ok((name.clone(), recur(value)?)))
            .collect::<Result<Vec<_>, BindError>>()?;
        if function.as_record().is_some_and(|original| {
            original
                .iter()
                .zip(&fields)
                .all(|((_, before), (_, after))| same_value(before, after))
        }) {
            return Ok(Value::Fcn(function.clone()));
        }
        return Value::record(fields, ctx).map_err(map_error);
    }
    // FcnRcdValue.permute visits every range before any domain.
    let ranges = function
        .iter()
        .map(|(_, range)| recur(range))
        .collect::<Result<Vec<_>, BindError>>()?;
    let pairs = function
        .iter()
        .zip(ranges)
        .map(|((domain, _), range)| Ok((recur(&domain)?, range)))
        .collect::<Result<Vec<_>, BindError>>()?;
    if function
        .iter()
        .zip(&pairs)
        .all(|((domain, before), (image, after))| {
            same_value(&domain, image) && same_value(before, after)
        })
    {
        return Ok(Value::Fcn(function.clone()));
    }
    Value::fcn(pairs, ctx).map_err(map_error)
}

fn deep_normalize(
    value: &Value,
    ctx: Ctx,
    depth: usize,
    work: &mut usize,
) -> Result<(), BindError> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| BindError::unsupported("SYMMETRY normalization work limit"))?;
    if *work > 32_000_000 || depth > 256 {
        return Err(BindError::unsupported(
            "SYMMETRY normalization work/depth limit",
        ));
    }
    match value {
        Value::Set(set) if matches!(set.kind(), SetKind::Enum | SetKind::Raw) => {
            // Children first, as SetEnumValue.deepNormalize. Reading raw values
            // here must not sort the parent before its children are normalized.
            if let Some(raw) = set.raw_elements() {
                for child in raw {
                    deep_normalize(child, ctx, depth + 1, work)?;
                }
            } else {
                for child in set
                    .elements(ctx)
                    .map_err(|e| {
                        BindError::unsupported(format!(
                            "SYMMETRY deep normalization: {}",
                            e.message()
                        ))
                    })?
                    .iter()
                {
                    deep_normalize(child, ctx, depth + 1, work)?;
                }
            }
            set.elements(ctx).map_err(|e| {
                BindError::unsupported(format!("SYMMETRY deep normalization: {}", e.message()))
            })?;
        }
        Value::Fcn(fcn) => {
            for (_, child) in fcn.iter() {
                deep_normalize(child, ctx, depth + 1, work)?;
            }
        }
        Value::Set(set) if set.as_deferred().is_some() => {
            let operand = set
                .as_deferred()
                .and_then(|provider| {
                    dive_eval::deferred_filter_normalization_operand(provider.as_ref())
                })
                .ok_or_else(|| {
                    BindError::unsupported(
                        "SYMMETRY deep normalization of unproved deferred values",
                    )
                })?;
            // SetPredValue visits only inVal, never its predicate. Unknown
            // providers and unproved lazy-operation caches remain refused
            // without invoking any provider method.
            deep_normalize(operand, ctx, depth + 1, work)?;
        }
        Value::Set(set) => {
            for (_, child) in set.components() {
                deep_normalize(&child, ctx, depth + 1, work)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// TLC's permute methods retain the original object only when every child keeps
// identity. Extensional equality is insufficient: an invariant set may still
// have changed its raw element order, observable until normalization.
fn same_value(before: &Value, after: &Value) -> bool {
    match (before, after) {
        (Value::Set(a), Value::Set(b)) => a.ptr_eq(b),
        (Value::Fcn(a), Value::Fcn(b)) => a.ptr_eq(b),
        _ => before == after,
    }
}
