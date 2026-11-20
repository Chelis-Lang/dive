//! Strict binding identity for projecting an established invariant inventory.
use super::{Miss, miss};
use dive_config::{ConfigValue, ConstantEntry, ModelConfig};
use dive_sema::{Analysis, Origin};
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Transitions {
    modules: Vec<Module>,
    init: Option<String>,
    next: Option<String>,
    spec: Option<String>,
    constants: Vec<Constant>,
    constraints: Vec<String>,
    deadlock: bool,
}
#[derive(Debug, Eq, PartialEq)]
enum Constant {
    Integer(String, i32),
    Boolean(String, bool),
    Substitute(String, String),
}
#[derive(Debug, Eq, PartialEq)]
enum Resolved {
    File(PathBuf),
    Text(String),
    Standard,
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Module {
    name: String,
    origin: Resolved,
    source: String,
}

impl Transitions {
    pub(super) fn new(analysis: &Analysis, cfg: &ModelConfig) -> Result<Self, Miss> {
        let mut valid = ModelConfig::default();
        valid.init.clone_from(&cfg.init);
        valid.next.clone_from(&cfg.next);
        valid.specification.clone_from(&cfg.specification);
        valid.constants.clone_from(&cfg.constants);
        valid.constraints.clone_from(&cfg.constraints);
        valid.invariants.clone_from(&cfg.invariants);
        valid.check_deadlock.clone_from(&cfg.check_deadlock);
        if &valid != cfg
            || !matches!(
                (&cfg.specification, &cfg.init, &cfg.next),
                (Some(_), None, None) | (None, Some(_), Some(_))
            )
        {
            return Err(miss(
                "requires ordinary Init/Next or a safety specification, optional state constraints, constants and invariants",
            ));
        }
        let constants = cfg
            .constants
            .iter()
            .map(|entry| match entry {
                ConstantEntry::Assignment {
                    name,
                    module: None,
                    args,
                    value,
                } if args.is_empty() => match &value.value {
                    ConfigValue::Int(n) => Ok(Constant::Integer(name.text.clone(), *n)),
                    ConfigValue::Bool(b) => Ok(Constant::Boolean(name.text.clone(), *b)),
                    _ => Err(miss(
                        "requires an integer or Boolean literal constant assignment",
                    )),
                },
                ConstantEntry::Substitution {
                    name,
                    module: None,
                    target,
                } => Ok(Constant::Substitute(name.text.clone(), target.text.clone())),
                _ => Err(miss("unsupported constant override or assignment")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let modules = closed_modules(analysis)?;
        Ok(Self {
            modules,
            init: cfg.init.as_ref().map(|n| n.text.clone()),
            next: cfg.next.as_ref().map(|n| n.text.clone()),
            spec: cfg.specification.as_ref().map(|n| n.text.clone()),
            constants,
            constraints: cfg.constraints.iter().map(|n| n.text.clone()).collect(),
            deadlock: cfg.checks_deadlock(),
        })
    }
}

/// Exact source and loader origins admitted by the projection APIs.
pub(crate) fn closed_modules(analysis: &Analysis) -> Result<Vec<Module>, Miss> {
    let root_dir = analysis
        .model()
        .root()
        .and_then(|id| analysis.model().node(id).span)
        .and_then(|span| analysis.file(span.file))
        .and_then(|f| match &f.origin {
            Origin::File(path) => path.parent(),
            _ => None,
        });
    let mut modules = Vec::new();
    for f in analysis.files() {
        let name = f
            .parse
            .module()
            .and_then(|m| m.name())
            .ok_or_else(|| miss("unnamed module"))?;
        let origin = match &f.origin {
            Origin::File(path) => Resolved::File(
                root_dir
                    .and_then(|d| path.strip_prefix(d).ok())
                    .unwrap_or(path)
                    .to_owned(),
            ),
            Origin::Text => Resolved::Text(f.display.clone()),
            Origin::Standard
                if matches!(
                    name.as_str(),
                    "Naturals" | "Integers" | "FiniteSets" | "Sequences"
                ) =>
            {
                Resolved::Standard
            }
            _ => return Err(miss("external or non-admitted standard module")),
        };
        modules.push(Module {
            name,
            origin,
            source: f.text.clone(),
        });
    }
    Ok(modules)
}
