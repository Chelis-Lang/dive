//! Finite source choices for fresh-runtime existential replay.
//!
//! Each complete attempt is rebound by Session. No failed attempt may lend
//! values, normalized caches, registers or atoms to its successor.
//!
//! **Recorded draws** (ADR 0162 §8). A simulation trace records the values
//! each step's random operators drew ([`crate::TraceDoc::draws`]) and those
//! start-up drew ([`crate::TraceDoc::startup_draws`]). They are untrusted
//! hints. A call first chooses among the step's recorded values that this
//! validator's own evaluation shows to be a possible result
//! ([`crate::draws::admits`]), from the one after the last used, then falls
//! back to the complete bounded enumeration below, which only
//! `RandomElement` has (any other operator refuses there); a start-up value
//! only moves the matching member to the front of that enumeration. The
//! search runs in two phases: the first offers only the admitted recorded
//! draws, so a wrong `\E` binding above a call is revisited before that
//! call's whole set is enumerated under it; if it finds no reproduction and
//! left any enumeration out, the second phase searches again with every
//! enumeration. A recorded value is never a result the arguments do not
//! admit, and the second phase keeps an exhausted search complete, so a
//! trace is invalid only when no admitted result of any call reproduces it.

use std::sync::{Mutex, PoisonError};

use dive_eval::HostError;
use dive_ir::StdOp;
use dive_value::{Ctx, SetKind, Value};
use serde_json::Value as J;

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
    /// The recorded draws of the step being checked, decoded.
    step: Vec<Value>,
    /// The position after the step's last recorded draw used.
    cursor: usize,
    /// Start-up draws in dive's JSON form, and the next one to match.
    startup: Vec<J>,
    startup_next: usize,
    /// No step's draws were offered since the attempt began: the source is
    /// still being bound.
    binding: bool,
    phase: Phase,
}

/// Which alternatives a call with admitted recorded draws offers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Only its admitted recorded draws.
    #[default]
    Recorded,
    /// The first phase left out some call's enumeration.
    RecordedNarrowed,
    /// Its admitted recorded draws, then the complete enumeration.
    Complete,
}

impl Search {
    pub(crate) fn rewind(&self) {
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        choices.visited.clear();
        choices.refusal = None;
        choices.hints.clear();
        choices.random = false;
        choices.step.clear();
        choices.cursor = 0;
        choices.startup_next = 0;
        choices.binding = true;
    }

    /// The start-up draws of the trace being validated, kept across
    /// attempts.
    pub(crate) fn set_startup(&self, draws: &[J]) {
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        choices.startup = draws.to_vec();
        choices.binding = true;
    }

    /// The recorded draws of the step now checked (none for a step without
    /// any); binding is over.
    pub(crate) fn set_step(&self, draws: Option<&[Value]>) {
        let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        choices.step = draws.map(<[Value]>::to_vec).unwrap_or_default();
        choices.cursor = 0;
        choices.binding = false;
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

    /// One of `count` elements of a set, trying `preferred` first.
    pub(crate) fn element(&self, count: usize, preferred: usize) -> Result<usize, HostError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .select(count, preferred)
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

    /// Moves to the next unexplored path. When the first phase is
    /// exhausted after leaving out some call's enumeration, the search
    /// starts again from the first path with every enumeration offered, so
    /// an exhausted search is always a complete one.
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
        if choices.phase == Phase::RecordedNarrowed {
            choices.phase = Phase::Complete;
            choices.path.clear();
            return true;
        }
        false
    }

    pub(crate) fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, HostError> {
        let (recorded, startup) = {
            let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            choices.random = true;
            let startup = if choices.binding {
                let hint = choices.startup.get(choices.startup_next).cloned();
                choices.startup_next += 1;
                hint
            } else {
                None
            };
            (choices.admitted(op, args, ctx), startup)
        };
        if !recorded.is_empty() {
            let mut choices = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            // In the second phase the last alternative is the complete
            // enumeration.
            let alternatives = if choices.phase == Phase::Complete {
                recorded.len() + 1
            } else {
                choices.phase = Phase::RecordedNarrowed;
                recorded.len()
            };
            let index = choices.select(alternatives, 0)?;
            if let Some((position, value)) = recorded.get(index) {
                choices.cursor = position + 1;
                return Ok(value.clone());
            }
        }
        self.random_inner(op, args, ctx, 0, startup.as_ref())
    }

    fn random_inner(
        &self,
        op: StdOp,
        args: &[Value],
        ctx: Ctx,
        depth: usize,
        startup: Option<&J>,
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
                    self.random_inner(op, std::slice::from_ref(range), ctx, depth + 1, None)?,
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
                    self.random_inner(op, std::slice::from_ref(field), ctx, depth + 1, None)?,
                ));
            }
            return Value::record(record, ctx)
                .map_err(|error| self.refuse(format!("random record value: {error}")));
        }
        if let Some(factors) = domain.as_product() {
            let mut tuple = Vec::with_capacity(factors.len());
            for factor in factors {
                tuple.push(self.random_inner(
                    op,
                    std::slice::from_ref(factor),
                    ctx,
                    depth + 1,
                    None,
                )?);
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
            let preferred = choices.interval_preference(lo, hi, startup);
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
        let preferred = choices.element_preference(&elements, startup, ctx);
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
    /// The offset in `lo..hi` to try first: the start-up draw, else a
    /// state scalar in the interval, else the first.
    fn interval_preference(&self, lo: i64, hi: i64, startup: Option<&J>) -> usize {
        let offset = |v: i64| {
            (lo <= v && v <= hi)
                .then(|| usize::try_from(i128::from(v) - i128::from(lo)).ok())
                .flatten()
        };
        startup
            .and_then(J::as_i64)
            .and_then(offset)
            .or_else(|| {
                self.hints
                    .iter()
                    .find_map(|value| offset(value.as_int()?.to_i64()?))
            })
            .unwrap_or(0)
    }

    /// The element to try first: the start-up draw, else one equal to a
    /// state scalar, else the first.
    fn element_preference(&self, elements: &[Value], startup: Option<&J>, ctx: Ctx) -> usize {
        startup
            .and_then(|hint| {
                elements.iter().position(|value| {
                    crate::value_json::to_json(value, ctx).is_ok_and(|json| &json == hint)
                })
            })
            .or_else(|| {
                elements
                    .iter()
                    .position(|value| self.hints.iter().any(|hint| scalar_equal(value, hint)))
            })
            .unwrap_or(0)
    }

    /// The step's recorded draws that are possible results of `op` on
    /// `args` by this evaluation, starting after the last one used and
    /// without repeats, with their positions.
    fn admitted(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Vec<(usize, Value)> {
        let n = self.step.len();
        let mut out: Vec<(usize, Value)> = Vec::new();
        for offset in 0..n {
            let position = (self.cursor + offset) % n;
            let value = &self.step[position];
            if out.iter().any(|(_, seen)| seen == value) {
                continue;
            }
            if crate::draws::admits(op, args, value, ctx) {
                out.push((position, value.clone()));
            }
        }
        out
    }

    fn select(&mut self, count: usize, preferred: usize) -> Result<usize, HostError> {
        // A single alternative is no choice point: nothing to revisit. Every
        // attempt of a phase skips it alike, so the path stays aligned.
        if count == 1 && preferred == 0 {
            return Ok(0);
        }
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
