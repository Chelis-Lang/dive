//! Finite source choices for fresh-runtime existential replay.
//!
//! Each complete attempt is rebound by Session. No failed attempt may lend
//! values, normalized caches, registers or atoms to its successor.

use std::sync::{Mutex, PoisonError};

use dive_eval::HostError;
use dive_ir::StdOp;
use dive_value::{Ctx, SetKind, Value};

pub(crate) const MAX_ATTEMPTS: usize = 65_536;
const MAX_CHOICES: usize = 256;
const MAX_DOMAIN: u64 = 65_536;

#[derive(Debug, Default)]
pub(crate) struct Search(Mutex<Choices>);

#[derive(Debug, Default)]
struct Choices {
    path: Vec<usize>,
    visited: Vec<(usize, usize)>,
    refusal: Option<String>,
    hints: Vec<Value>,
    random: bool,
}

impl Search {
    pub(crate) fn rewind(&self) {
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        choices.visited.clear();
        choices.refusal = None;
        choices.hints.clear();
        choices.random = false;
    }

    /// Supplied scalars only prioritize actual domain elements. They are never
    /// answers or membership authority, and comparing them forces no value.
    pub(crate) fn prefer(&self, state: &crate::State) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).hints = state
            .iter()
            .flatten()
            .filter(|value| !matches!(value, Value::Set(_) | Value::Fcn(_)))
            .take(MAX_CHOICES)
            .cloned()
            .collect();
    }

    pub(crate) fn drew(&self) -> bool {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).random
    }

    pub(crate) fn branch(&self, count: usize) -> Result<usize, HostError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .select(count, 0)
    }

    pub(crate) fn refusal(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refusal
            .clone()
    }

    pub(crate) fn refuse(&self, message: impl Into<String>) -> HostError {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refuse(message.into())
    }

    pub(crate) fn advance(&self) -> bool {
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        while let Some((choice, count)) = choices.visited.pop() {
            if choice + 1 < count {
                choices.path = choices.visited.iter().map(|&(index, _)| index).collect();
                choices.path.push(choice + 1);
                choices.visited.clear();
                return true;
            }
        }
        false
    }

    pub(crate) fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, HostError> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).random = true;
        self.random_inner(op, args, ctx, 0)
    }

    fn random_inner(
        &self,
        op: StdOp,
        args: &[Value],
        ctx: Ctx,
        depth: usize,
    ) -> Result<Value, HostError> {
        if depth >= MAX_CHOICES {
            return Err(self.refuse("independent random choice constructor depth"));
        }
        if op != StdOp::TlcRandomElement {
            return Err(self.refuse("independent finite choice operator"));
        }
        let [Value::Set(domain)] = args else {
            return Err(self.refuse("RandomElement relation requires a finite set"));
        };
        // Follow the native constructor algorithm, rather than enumerate every
        // function/record/product. In particular, an empty function domain must
        // not demand its range. Each recursive selection is still independently
        // checked against the actual component domain.
        if let Some((domain, range)) = domain.as_fcn_set() {
            let Value::Set(domain) = domain else {
                return Err(self.refuse("random function domain must be finite"));
            };
            self.bounded_domain(domain, ctx)?;
            let keys = domain
                .elements(ctx)
                .map_err(|error| self.refuse(format!("random function domain: {error}")))?;
            let mut pairs = Vec::with_capacity(keys.len());
            for key in keys.iter() {
                pairs.push((
                    key.clone(),
                    self.random_inner(op, std::slice::from_ref(range), ctx, depth + 1)?,
                ));
            }
            return Value::fcn(pairs, ctx)
                .map_err(|error| self.refuse(format!("random function value: {error}")));
        }
        if let Some(fields) = domain.as_rcd_set() {
            let mut record = Vec::with_capacity(fields.len());
            for (name, field) in fields {
                record.push((
                    name.clone(),
                    self.random_inner(op, std::slice::from_ref(field), ctx, depth + 1)?,
                ));
            }
            return Value::record(record, ctx)
                .map_err(|error| self.refuse(format!("random record value: {error}")));
        }
        if let Some(factors) = domain.as_product() {
            let mut tuple = Vec::with_capacity(factors.len());
            for factor in factors {
                tuple.push(self.random_inner(op, std::slice::from_ref(factor), ctx, depth + 1)?);
            }
            return Ok(Value::tuple(tuple));
        }
        if let Some((lo, hi)) = domain.as_interval() {
            let (Some(lo), Some(hi)) = (lo.to_i64(), hi.to_i64()) else {
                return Err(self.refuse("independent random interval requires i64 endpoints"));
            };
            let size = domain
                .size(ctx)
                .map_err(|error| self.refuse(format!("random interval size: {error}")))?;
            let count = usize::try_from(size)
                .map_err(|_| self.refuse("independent random interval cardinality"))?;
            if count == 0 {
                return Err(self.refuse("RandomElement relation has an empty interval"));
            }
            let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let preferred = choices
                .hints
                .iter()
                .find_map(|value| {
                    let value = value.as_int()?.to_i64()?;
                    (lo <= value && value <= hi)
                        .then(|| usize::try_from(i128::from(value) - i128::from(lo)).ok())
                        .flatten()
                })
                .unwrap_or(0);
            let selected = choices.select(count, preferred)?;
            let value = i64::try_from(i128::from(lo) + selected as i128)
                .map_err(|_| choices.refuse("independent random interval offset".into()))?;
            return Ok(Value::Int(value));
        }
        // This proof operation bounds a complete domain. The producer may use
        // a cheaper recursive random algorithm for some larger symbolic sets;
        // refusing this proof never grants its result a success claim.
        let size = self.bounded_domain(domain, ctx)?;
        let elements = domain
            .elements(ctx)
            .map_err(|error| self.refuse(format!("random choice elements: {error}")))?;
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if size == 0 || elements.is_empty() {
            return Err(choices.refuse("RandomElement relation has an empty domain".into()));
        }
        let preferred = elements
            .iter()
            .position(|value| choices.hints.iter().any(|hint| scalar_equal(value, hint)))
            .unwrap_or(0);
        let index = choices.select(elements.len(), preferred)?;
        elements
            .get(index)
            .cloned()
            .ok_or_else(|| choices.refuse("independent random choice path/domain mismatch".into()))
    }

    fn bounded_domain(&self, domain: &dive_value::Set, ctx: Ctx) -> Result<u64, HostError> {
        if domain.kind() == SetKind::Deferred {
            return Err(self.refuse("independent random choice deferred domain"));
        }
        match domain.size(ctx) {
            Ok(size) if size <= MAX_DOMAIN => Ok(size),
            Ok(_) => Err(self.refuse("independent random choice domain bound")),
            Err(error) => Err(self.refuse(format!("random choice domain: {error}"))),
        }
    }
}

impl Choices {
    fn select(&mut self, count: usize, preferred: usize) -> Result<usize, HostError> {
        if self.visited.len() >= MAX_CHOICES {
            return Err(self.refuse("independent random choice draw bound".into()));
        }
        let index = self.path.get(self.visited.len()).copied().unwrap_or(0);
        if index >= count || preferred >= count {
            return Err(self.refuse("independent random choice path/domain mismatch".into()));
        }
        self.visited.push((index, count));
        // Swap only enumeration positions. Every alternative is still visited
        // once; source predicates must independently accept the chosen element.
        Ok(if index == 0 {
            preferred
        } else if index == preferred {
            0
        } else {
            index
        })
    }
    fn refuse(&mut self, message: String) -> HostError {
        self.refusal.get_or_insert_with(|| message.clone());
        HostError { code: 0, message }
    }
}

fn scalar_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Int(_) | Value::BigInt(_), Value::Int(_) | Value::BigInt(_)) => {
            left.as_int() == right.as_int()
        }
        (Value::Str(left), Value::Str(right)) | (Value::Model(left), Value::Model(right)) => {
            left.as_str() == right.as_str()
        }
        _ => false,
    }
}
