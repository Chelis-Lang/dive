//! Fixed configuration outside the genuine temporal inventory.
use super::{Miss, miss};
use crate::ConfigReport;
use dive_config::{ConfigValue, ConstantEntry, ModelConfig};

#[derive(PartialEq, Eq)]
pub(super) struct Config {
    specification: Option<String>,
    init: Option<String>,
    next: Option<String>,
    deadlock: Option<bool>,
    constants: Vec<Constant>,
    model_values: Vec<String>,
}
#[derive(PartialEq, Eq)]
enum Constant {
    Literal(String, Vec<u8>),
    Substitute(String, String),
}

impl Config {
    pub(super) fn read(report: &ConfigReport, cfg: &ModelConfig) -> Result<Self, Miss> {
        let mut allowed = ModelConfig::default();
        allowed.specification.clone_from(&cfg.specification);
        allowed.init.clone_from(&cfg.init);
        allowed.next.clone_from(&cfg.next);
        allowed.check_deadlock.clone_from(&cfg.check_deadlock);
        allowed.properties.clone_from(&cfg.properties);
        allowed.constants.clone_from(&cfg.constants);
        allowed.model_values.clone_from(&cfg.model_values);
        if &allowed != cfg
            || cfg.properties.is_empty()
            || !matches!(
                (&cfg.specification, &cfg.init, &cfg.next),
                (Some(_), None, None) | (None, Some(_), Some(_))
            )
        {
            return Err(miss(
                "requires a behavior and only genuine temporal properties",
            ));
        }
        let constants=cfg.constants.iter().map(|entry| match entry {
            ConstantEntry::Substitution{name,module:None,target} => Ok(Constant::Substitute(name.text.clone(),target.text.clone())),
            ConstantEntry::Assignment{name,module:None,args,value} if args.is_empty() => {
                fn literal(v:&ConfigValue)->bool {matches!(v,ConfigValue::Int(_) | ConfigValue::Bool(_) | ConfigValue::ModelValue(_))}
                if !(literal(&value.value) || matches!(&value.value,ConfigValue::Set(items) if items.iter().all(|e|literal(&e.value)))) {
                    return Err(miss("requires finite flat literal constant assignments"));
                }
                let bytes=report.source_bytes().get(usize::from(value.span.start())..usize::from(value.span.end())).ok_or_else(||miss("constant source range"))?;
                Ok(Constant::Literal(name.text.clone(),bytes.to_vec()))
            }
            _ => Err(miss("requires unscoped ordinary constant bindings")),
        }).collect::<Result<_,_>>()?;
        Ok(Self {
            specification: cfg.specification.as_ref().map(|n| n.text.clone()),
            init: cfg.init.as_ref().map(|n| n.text.clone()),
            next: cfg.next.as_ref().map(|n| n.text.clone()),
            deadlock: cfg.check_deadlock.as_ref().map(|entry| entry.value),
            constants,
            model_values: cfg.model_values.clone(),
        })
    }
}
