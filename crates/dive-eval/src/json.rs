//! Pure `Json!ToJson`, following MIT `tlc2.module.Json` at tlaplus `94d0c50`.
//! Gson escaping is shared with the value-layer trace serializer. TLC object
//! insertion history is absent from normalized dive functions, so compatibility
//! mode refuses objects with multiple keys and history-sensitive compound keys
//! instead of inventing an order.

use dive_ir::ExprId;

use dive_value::{SetKind, Value, ValueOrder, json::json_string, print::to_tlc_string};

use crate::core::Core;
use crate::error::{ErrorClass, EvalError, Result};

fn refuse(why: &str) -> EvalError {
    EvalError::unsupported(format!("Json!ToJson ({why})"))
}

impl Core {
    pub(crate) fn to_json(&self, value: &Value, origin: ExprId) -> Result<Value> {
        let text = self.json_node(value, origin)?;
        Ok(Value::Str(self.names.intern(&text)))
    }

    fn json_node(&self, value: &Value, origin: ExprId) -> Result<String> {
        let _depth = self.enter(origin)?;
        match value {
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_) => {
                Ok(dive_value::json::to_tlc_json(value, self.ctx)?)
            }
            Value::Fcn(function) => {
                if let Some(sequence) = function.as_tuple() {
                    let nodes = sequence
                        .iter()
                        .map(|item| self.json_node(item, origin))
                        .collect::<Result<Vec<_>>>()?;
                    return Ok(format!("[{}]", nodes.join(",")));
                }
                if self.ctx.order == ValueOrder::Tlc && function.len() >= 2 {
                    return Err(refuse(
                        "history-dependent object order with two or more keys",
                    ));
                }
                for (domain, _) in function.iter() {
                    self.json_domain_guard(&domain, origin)?;
                }
                let mut pairs: Vec<_> = function.iter().collect();
                pairs.sort_by(|left, right| left.0.cmp_with(&right.0, self.ctx.order));
                let mut nodes = Vec::with_capacity(pairs.len());
                for (domain, item) in pairs {
                    let key = match domain {
                        Value::Str(atom) => atom.as_str().to_owned(),
                        other => to_tlc_string(&other, self.ctx)?,
                    };
                    nodes.push(format!(
                        "{}:{}",
                        json_string(&key),
                        self.json_node(item, origin)?
                    ));
                }
                Ok(format!("{{{}}}", nodes.join(",")))
            }
            Value::Set(set) => {
                if matches!(
                    set.kind(),
                    SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq
                ) {
                    return Err(refuse("infinite or symbolic set conversion"));
                }
                let elements = set.elements(self.ctx).map_err(|error| {
                    let error = EvalError::from(error);
                    match error.class() {
                        ErrorClass::ResourceLimit | ErrorClass::Unsupported | ErrorClass::Bug => {
                            error
                        }
                        ErrorClass::Eval => refuse(&format!("set conversion: {}", error.message())),
                    }
                })?;
                let mut elements = elements.to_vec();
                if set.order() != self.ctx.order {
                    elements.sort_by(|left, right| left.cmp_with(right, self.ctx.order));
                }
                let nodes = elements
                    .iter()
                    .map(|item| self.json_node(item, origin))
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!("[{}]", nodes.join(",")))
            }
        }
    }

    // Inspect stored children before the value printer allocates, normalizes or
    // calls a deferred provider. Printing must not bypass the evaluator's bound.
    fn json_domain_guard(&self, value: &Value, origin: ExprId) -> Result<()> {
        let _depth = self.enter(origin)?;
        match value {
            Value::Fcn(function) => {
                if let Some(sequence) = function.as_tuple() {
                    for item in sequence {
                        self.json_domain_guard(item, origin)?;
                    }
                } else {
                    if self.ctx.order == ValueOrder::Tlc
                        && function.as_record().is_some_and(|fields| fields.len() >= 2)
                    {
                        return Err(refuse(
                            "history-sensitive compound object domain containing a multi-field record",
                        ));
                    }
                    for (domain, item) in function.iter() {
                        self.json_domain_guard(&domain, origin)?;
                        self.json_domain_guard(item, origin)?;
                    }
                }
            }
            Value::Set(set) => {
                if let Some(elements) = set.stored_elements() {
                    for item in elements {
                        self.json_domain_guard(item, origin)?;
                    }
                } else if !matches!(
                    set.kind(),
                    SetKind::Interval | SetKind::Nat | SetKind::Int | SetKind::Strings
                ) {
                    return Err(refuse(
                        "symbolic or deferred domain-set printing is not bounded",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
