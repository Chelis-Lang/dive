//! Untrusted source context for independently replayed temporal row errors.

use dive_eval::{Ec, ErrorClass, EvalError};
use dive_model::Model;
use serde_json::{Value, json};

/// A request to validate the first source-state temporal demand error.
///
/// This contains no scope, predicate selector, execution proof or cached truth.
/// Only fresh source-bound validation may establish its claim. An older trace
/// reader retains the ordinary unsupported error ending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemporalStateEndpoint {
    module: String,
    start: u32,
    end: u32,
    instance: Option<String>,
}

impl TemporalStateEndpoint {
    /// Records an original inner error's source context, without certifying it.
    ///
    /// ```no_run
    /// # fn context(model: &dive_model::Model, error: &dive_eval::EvalError) {
    /// let request = dive_trace::TemporalStateEndpoint::from_error(model, error);
    /// // The independent validator must reconstruct and replay the demand.
    /// # let _ = request;
    /// # }
    /// ```
    #[must_use]
    pub fn from_error(model: &Model, error: &EvalError) -> Option<Self> {
        if error.class() != ErrorClass::Eval
            || !error.is_eval_exception()
            || !matches!(error.ec(), Ec::DivisionByZero | Ec::ModuleEvaluating)
        {
            return None;
        }
        let origin = model.ir.nodes().get(error.origin()?.index())?.origin;
        let file = model.ir.files().get(origin.file?.index())?;
        let instance = if let Some(definition) = origin.instance {
            let definition = model.ir.defs().get(definition.index())?;
            Some(format!(
                "{}: {}",
                definition.name,
                model.ir.location(&definition.origin)
            ))
        } else {
            None
        };
        Some(Self {
            module: file.module.clone(),
            start: origin.range.0,
            end: origin.range.1,
            instance,
        })
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({"stage":"temporal-state-row", "source":{
            "module":self.module,"start":self.start,"end":self.end,"instance":self.instance
        }})
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("endpoint must be an object")?;
        if object.len() != 2 || value["stage"] != "temporal-state-row" {
            return Err("unknown temporal error endpoint stage or fields".into());
        }
        let source = value["source"]
            .as_object()
            .ok_or("missing endpoint source")?;
        if source.len() != 4
            || !["module", "start", "end", "instance"]
                .iter()
                .all(|key| source.contains_key(*key))
        {
            return Err("unknown temporal error source fields".into());
        }
        let number = |key: &str| {
            value["source"][key]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| format!("invalid endpoint source {key}"))
        };
        let module = value["source"]["module"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("invalid endpoint module")?
            .to_owned();
        let instance = if value["source"]["instance"].is_null() {
            None
        } else {
            Some(
                value["source"]["instance"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or("invalid endpoint instance")?
                    .to_owned(),
            )
        };
        let start = number("start")?;
        let end = number("end")?;
        if start > end {
            return Err("reversed endpoint source range".into());
        }
        Ok(Self {
            module,
            start,
            end,
            instance,
        })
    }
}
