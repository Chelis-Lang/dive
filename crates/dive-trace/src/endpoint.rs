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
        Self::from_expression(ir, error.origin()?)
    }

    fn from_expression(ir: &dive_ir::Ir, expression: dive_ir::ExprId) -> Option<Self> {
        let origin = ir.nodes().get(expression.index())?.origin;
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
    ordered: Option<(u32, u32)>,
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
            .map(|source| Self {
                source,
                ordered: None,
            })
    }

    /// Requests actual ordered primary and diagnostic Next replay. Reported
    /// and deciding codes are untrusted claims, not classifier overrides.
    #[must_use]
    pub fn from_ordered_error(
        ir: &dive_ir::Ir,
        error: &EvalError,
        reported: u32,
        deciding: u32,
    ) -> Option<Self> {
        let mut request = Self::from_error(ir, error)?;
        request.ordered = Some((reported, deciding));
        Some(request)
    }

    /// Claimed deciding code, requiring independent diagnostic replay.
    #[must_use]
    pub fn deciding_code(&self) -> Option<u32> {
        self.ordered.map(|(_, deciding)| deciding)
    }

    pub(crate) fn ordered_codes(&self) -> Option<(u32, u32)> {
        self.ordered
    }

    pub(crate) fn matches_error_source(&self, ir: &dive_ir::Ir, error: &EvalError) -> bool {
        TemporalStateEndpoint::from_origin(ir, error).as_ref() == Some(&self.source)
    }

    pub(crate) fn to_json(&self) -> Value {
        if let Some((reported, deciding)) = self.ordered {
            let mut value = self.source.to_stage("next-ordered-terminal");
            value["reported"] = Value::from(reported);
            value["deciding"] = Value::from(deciding);
            value
        } else {
            self.source.to_stage("next-before-successor")
        }
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        if value["stage"] == "next-ordered-terminal" {
            let object = value.as_object().ok_or("Next endpoint must be an object")?;
            if object.len() != 4 {
                return Err("unknown ordered Next endpoint fields".into());
            }
            let code = |name: &str| {
                value[name]
                    .as_u64()
                    .and_then(|code| u32::try_from(code).ok())
                    .filter(|code| *code != 0)
                    .ok_or("invalid ordered Next error code")
            };
            let ordered = Some((code("reported")?, code("deciding")?));
            let mut source = value.clone();
            let context = source
                .as_object_mut()
                .ok_or("Next endpoint must be an object")?;
            context.remove("reported");
            context.remove("deciding");
            Ok(Self {
                source: TemporalStateEndpoint::from_stage(&source, "next-ordered-terminal")?,
                ordered,
            })
        } else {
            TemporalStateEndpoint::from_stage(value, "next-before-successor").map(|source| Self {
                source,
                ordered: None,
            })
        }
    }
}

/// Untrusted terminal partial bindings, distinct from every complete prefix row.
/// The source names the actual generating action; it selects no replay action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialStateEndpoint {
    initial: bool,
    source: TemporalStateEndpoint,
    assignments: serde_json::Map<String, Value>,
    unassigned: Vec<String>,
}

/// Compatibility name for the original Next partial request.
pub type PartialNextEndpoint = PartialStateEndpoint;

impl PartialStateEndpoint {
    /// Records an actual incomplete candidate without certifying it.
    #[must_use]
    pub fn from_candidate(
        ir: &dive_ir::Ir,
        expression: dive_ir::ExprId,
        assignments: serde_json::Map<String, Value>,
        unassigned: Vec<String>,
    ) -> Option<Self> {
        (!unassigned.is_empty()).then_some(Self {
            initial: false,
            source: TemporalStateEndpoint::from_expression(ir, expression)?,
            assignments,
            unassigned,
        })
    }

    /// Records actual partial Init bindings and the observed generating source.
    #[must_use]
    pub fn from_initial_candidate(
        ir: &dive_ir::Ir,
        expression: dive_ir::ExprId,
        assignments: serde_json::Map<String, Value>,
        unassigned: Vec<String>,
    ) -> Option<Self> {
        let mut endpoint = Self::from_candidate(ir, expression, assignments, unassigned)?;
        endpoint.initial = true;
        Some(endpoint)
    }

    pub(crate) fn initial(&self) -> bool {
        self.initial
    }

    pub(crate) fn assignments(&self) -> &serde_json::Map<String, Value> {
        &self.assignments
    }

    pub(crate) fn unassigned(&self) -> &[String] {
        &self.unassigned
    }

    pub(crate) fn matches_source(&self, ir: &dive_ir::Ir, expression: dive_ir::ExprId) -> bool {
        TemporalStateEndpoint::from_expression(ir, expression).as_ref() == Some(&self.source)
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut value = self.source.to_stage(if self.initial {
            "init-partial-state"
        } else {
            "next-partial-state"
        });
        value["assignments"] = Value::Object(self.assignments.clone());
        value["unassigned"] = serde_json::json!(self.unassigned);
        value
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("partial endpoint must be an object")?;
        if object.len() != 4
            || !object.contains_key("assignments")
            || !object.contains_key("unassigned")
        {
            return Err("unknown partial endpoint fields".into());
        }
        let mut context = value.clone();
        let object = context
            .as_object_mut()
            .ok_or("partial endpoint must be an object")?;
        object.remove("assignments");
        object.remove("unassigned");
        let stage = match value["stage"].as_str() {
            Some("next-partial-state") => "next-partial-state",
            Some("init-partial-state") => "init-partial-state",
            _ => return Err("unknown partial terminal stage".into()),
        };
        let source = TemporalStateEndpoint::from_stage(&context, stage)?;
        let assignments = value["assignments"]
            .as_object()
            .ok_or("partial assignments must be an object")?
            .clone();
        let unassigned: Vec<String> = value["unassigned"]
            .as_array()
            .ok_or("partial unassigned must be a list")?
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or("partial unassigned needs variable names")
            })
            .collect::<Result<_, _>>()?;
        let names: std::collections::BTreeSet<_> = unassigned.iter().collect();
        if unassigned.is_empty()
            || names.len() != unassigned.len()
            || unassigned.iter().any(|name| assignments.contains_key(name))
        {
            return Err("partial assigned and unassigned variables must be disjoint, with at least one unassigned".into());
        }
        Ok(Self {
            initial: stage == "init-partial-state",
            source,
            assignments,
            unassigned,
        })
    }
}

/// The independently discovered candidate-side error site (ADR 0128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateErrorSite {
    /// An ordered state constraint.
    Constraint,
    /// An ordered action constraint.
    ActionConstraint,
    /// An ordered implied action property.
    ImpliedAction,
}
impl CandidateErrorSite {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Constraint => "constraint",
            Self::ActionConstraint => "action-constraint",
            Self::ImpliedAction => "implied-action",
        }
    }
}

/// Untrusted candidate-side primary and diagnostic identities. This never
/// selects an expression, action or candidate during independent execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateStateEndpoint {
    pub(crate) next: NextStateEndpoint,
    pub(crate) site: CandidateErrorSite,
}
impl CandidateStateEndpoint {
    /// Records a typed producer observation; only fresh replay can prove it.
    #[must_use]
    pub fn from_error(
        ir: &dive_ir::Ir,
        error: &EvalError,
        site: CandidateErrorSite,
        reported: u32,
        deciding: u32,
    ) -> Option<Self> {
        Some(Self {
            next: NextStateEndpoint::from_ordered_error(ir, error, reported, deciding)?,
            site,
        })
    }

    /// The untrusted deciding identity, checked against the report and replay.
    #[must_use]
    pub fn deciding_code(&self) -> Option<u32> {
        self.next.deciding_code()
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut value = self.next.to_json();
        value["stage"] = Value::from("candidate-ordered-terminal");
        value["site"] = Value::from(self.site.name());
        value
    }

    pub(crate) fn from_json(value: &Value) -> Result<Self, String> {
        let mut value = value.clone();
        let object = value
            .as_object_mut()
            .ok_or("candidate endpoint must be an object")?;
        if object.len() != 5
            || object.get("stage").and_then(Value::as_str) != Some("candidate-ordered-terminal")
        {
            return Err("unknown candidate endpoint fields or stage".into());
        }
        let site = match object
            .remove("site")
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
        {
            Some("constraint") => CandidateErrorSite::Constraint,
            Some("action-constraint") => CandidateErrorSite::ActionConstraint,
            Some("implied-action") => CandidateErrorSite::ImpliedAction,
            _ => return Err("unknown candidate error site".into()),
        };
        object.insert("stage".into(), Value::from("next-ordered-terminal"));
        Ok(Self {
            next: NextStateEndpoint::from_json(&value)?,
            site,
        })
    }
}
