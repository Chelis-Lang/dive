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
        Self::from_origin(&model.ir, error)
    }

    fn from_origin(ir: &dive_ir::Ir, error: &EvalError) -> Option<Self> {
        let origin = ir.nodes().get(error.origin()?.index())?.origin;
        let file = ir.files().get(origin.file?.index())?;
        let instance = if let Some(definition) = origin.instance {
            let definition = ir.defs().get(definition.index())?;
            Some(format!(
                "{}: {}",
                definition.name,
                ir.location(&definition.origin)
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
        self.to_stage("temporal-state-row")
    }

    fn to_stage(&self, stage: &str) -> Value {
        json!({"stage":stage, "source":{
            "module":self.module,"start":self.start,"end":self.end,"instance":self.instance
        }})
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        Self::from_stage(value, "temporal-state-row")
    }

    fn from_stage(value: &Value, stage: &str) -> Result<Self, String> {
        let object = value.as_object().ok_or("endpoint must be an object")?;
        if object.len() != 2 || value["stage"] != stage {
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

/// Untrusted original source context for a Next error before any successor.
/// The fresh validator enumerates the real configured actions independently;
/// this descriptor selects no expression, action, scope or cached value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextStateEndpoint {
    source: TemporalStateEndpoint,
}

impl NextStateEndpoint {
    pub(crate) fn instantiated(&self) -> bool {
        self.source.instance.is_some()
    }

    /// Records an observed evaluation error's original source, without proof.
    /// The producer separately requires the typed Next error site.
    ///
    /// ```no_run
    /// # fn request(ir: &dive_ir::Ir, error: &dive_eval::EvalError) {
    /// let origin = dive_trace::NextStateEndpoint::from_error(ir, error);
    /// // The descriptor must still be checked by fresh independent replay.
    /// # let _ = origin;
    /// # }
    /// ```
    #[must_use]
    pub fn from_error(ir: &dive_ir::Ir, error: &EvalError) -> Option<Self> {
        (error.class() == ErrorClass::Eval)
            .then(|| TemporalStateEndpoint::from_origin(ir, error))
            .flatten()
            .map(|source| Self { source })
    }

    pub(crate) fn to_json(&self) -> Value {
        self.source.to_stage("next-before-successor")
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        TemporalStateEndpoint::from_stage(value, "next-before-successor")
            .map(|source| Self { source })
    }
}
