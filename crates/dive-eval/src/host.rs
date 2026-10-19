//! What the evaluator needs from the run around it: the model checker's
//! spec-visible state (`TLCGet`'s live keys, `TLCSet("exit")`), the random
//! stream TLC reseeds per expanded state, and TLC's register file.
//!
//! TLC reads these from its own globals and threads
//! (`tlc2/module/TLCGetSet.java`, `RandomEnumerableValues.java`,
//! `IdThread.java` at tlaplus `94d0c50`). dive keeps them out of the
//! evaluator's core: the engine that drives the evaluator implements
//! [`Host`] and installs it with [`crate::Evaluator::set_host`]. Without a
//! host, the operators that need one are refused by name, never guessed.
//!
//! **Registers** belong to the evaluator itself (one logical register file
//! per run, ADR 0014 §7): TLC's main thread writes every worker's registers,
//! and dive's search is TLC's single-worker search, so one file reproduces
//! every value a TLC run with one worker reads.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dive_ir::{Arg, ExprId, StdOp};
use dive_value::{Ctx, Int, Value};

use crate::core::Core;
use crate::env::{Env, Ev, St, control};
use crate::error::{Ec, EvalError, Result};

/// A live counter of the model checker, as `TLCGet` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Counter {
    /// `"generated"`: states generated so far (`getStatesGenerated`).
    Generated,
    /// `"distinct"`: distinct states found so far.
    Distinct,
    /// `"queue"`: states left on the queue.
    Queue,
    /// `"diameter"`: `getProgress()`, the deepest level reached so far.
    Diameter,
    /// `"stats".initial`: initial states generated.
    Initial,
    /// `"duration"`: whole seconds since the run started. Wall-clock time,
    /// nondeterministic in TLC as in dive.
    DurationSecs,
}

/// `TLCGet("config")`: the run's settings, as TLC's `AbstractChecker.createConfig`
/// records them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunConfig {
    /// Whether deadlock is checked.
    pub deadlock: bool,
    /// The worker count TLC reports. dive reports the logical count, 1
    /// (D-011): its search at any worker count is TLC's single-worker search.
    pub workers: i64,
    /// The run's seed (printed as a string, as TLC does).
    pub seed: i64,
    /// The fingerprint polynomial (`FP64.getIrredPoly()`, printed as a signed
    /// long in a string).
    pub fingerprint: i64,
    /// TLC's install location; dive's is the running executable's.
    pub install: String,
}

/// An error raised by a host operation, with TLC's code and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError {
    /// TLC's error code (`tlc2.output.EC`); 0 for a refusal.
    pub code: u32,
    /// TLC's message, or the refused feature.
    pub message: String,
}

/// The run hosting the evaluator: the model checker (or another driver).
///
/// Every method has a default that provides nothing; an operator that needs
/// what a host does not provide is refused by name.
///
/// **Parallel engines.** The methods take `&self` and are called from the
/// thread that evaluates. A host whose answers differ per worker (the level
/// of the state being expanded, the random stream) keeps them per thread.
pub trait Host: Send + Sync + std::fmt::Debug {
    /// Checks a native operation at runtime, before its arguments or body run.
    /// Ordinary evaluators permit it; an isolated driver may remember a refusal
    /// even if startup binding later catches the evaluation error.
    ///
    /// # Errors
    /// A named refusal or a host evaluation error.
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        Ok(())
    }

    /// Checks a rendered Print/PrintT event before any output sink is called.
    /// Arguments and rendering have already been evaluated. `false` preserves
    /// the operator's return value while discarding replay output; `true`
    /// emits the event. Ordinary hosts keep the existing output behavior.
    ///
    /// # Errors
    /// A named refusal or an output resource bound. Rendering's temporary
    /// allocation is not bounded by this hook.
    fn print_output(&self, _text: &str) -> Result<bool, HostError> {
        Ok(true)
    }

    /// Checks a string TLCGet/TLCSet key before accessing host state.
    ///
    /// # Errors
    /// A named refusal or a host evaluation error.
    fn check_tlc_access(&self, _key: &str, _write: bool) -> Result<(), HostError> {
        Ok(())
    }

    /// Applies a random operator (`RandomElement`, `RandomSubset`,
    /// `RandomSetOfSubsets`) to its evaluated arguments, drawing from the
    /// stream of the current evaluation: TLC's start-up stream during
    /// constant definitions, `ASSUME`s and `Init`, and the stream reseeded
    /// from the expanded state's fingerprint while expanding a state.
    /// `None` when the host has no stream.
    fn random(&self, _op: StdOp, _args: &[Value], _ctx: Ctx) -> Option<Result<Value, HostError>> {
        None
    }

    /// Checks whether this run permits access to the evaluator's register
    /// file. Called before integer/named `TLCGet` reads, aggregate register
    /// reads (`"all"`, `"all:named"`), and integer/named `TLCSet` writes.
    /// Other introspection keys do not call this method. The default permits
    /// access, preserving ordinary evaluator and checker behavior.
    ///
    /// # Errors
    /// A host may refuse search-dependent register access (code 0) or provide
    /// a TLC evaluation error. Refusal occurs before reading or writing.
    fn check_register_access(&self) -> Result<(), HostError> {
        Ok(())
    }

    /// Checks whether expression evaluation may use the successor generator
    /// to decide action composition. Independent replay drivers refuse this
    /// before any intermediate state is generated; ordinary drivers permit it.
    ///
    /// # Errors
    /// A refusal (code 0) or an evaluation error supplied by the driver.
    fn check_action_composition(&self) -> Result<(), HostError> {
        Ok(())
    }

    /// Whether lazy expression results may be read from or written to caches.
    /// Ordinary drivers retain TLC's cache behavior. Independent replay returns
    /// false so a result previously computed by another consumer cannot replace
    /// evaluation of the original expression under the replay host.
    fn reuse_expression_cache(&self) -> bool {
        true
    }

    /// The cache domain that lazy expression results computed under this
    /// host belong to. A cached result is reused only by evaluation under a
    /// host of the same domain. Ordinary drivers share domain 0 (TLC's single
    /// `LazyValue` cache). Independent replay gives each run its own domain, so
    /// a value cached by another consumer (for example through action
    /// composition, which replay refuses) never becomes replay's answer and a
    /// replay verdict does not depend on how warm the caches were.
    fn expression_cache_domain(&self) -> u64 {
        0
    }

    /// `TLCGet("level")` outside `ASSUME` and `Init`: the level of TLC's
    /// `s0`, the state being expanded in the next-state relation and action
    /// checks, the successor in invariants and constraints (initial states
    /// are level 1).
    fn level(&self) -> Option<i64> {
        None
    }

    /// A live counter.
    fn counter(&self, _counter: Counter) -> Option<i64> {
        None
    }

    /// `TLCGet("config")`.
    fn config(&self) -> Option<RunConfig> {
        None
    }

    /// `TLCSet("exit", TRUE)`: ends the run as a success once the current
    /// state is done (`ModelChecker.stop`). Returns whether the host acts on
    /// it.
    fn request_exit(&self) -> bool {
        false
    }
}

/// The names `TLCGetSet`'s static initializer interns, in declaration order
/// (`tlc2/module/TLCGetSet.java:52-162`). TLC loads the class when it
/// creates its model checker (`AbstractChecker.createConfig`), after
/// processing constant definitions and before checking assumptions; a
/// driver that reproduces TLC's name order interns them then
/// ([`crate::Evaluator::intern_checker_names`]). They decide the order of the
/// fields of `TLCGet("config")` and `TLCGet("stats")`.
pub const TLCGET_NAMES: &[&str] = &[
    "exit",
    "pause",
    "config",
    "spec",
    "coverage",
    "action",
    "install",
    "id",
    "behavior",
    "all",
    "all:named",
    "mode",
    "deadlock",
    "seed",
    "fingerprint",
    "worker",
    "traces",
    "depth",
    "aril",
    "sched",
    "revision",
    "timestamp",
    "date",
    "count",
    "tag",
    "calver",
    "impliedinits",
    "impliedactions",
    "invariants",
    "impliedtemporals",
    "temporals",
    "actions",
    "inits",
    "variables",
    "actionconstraints",
    "constraints",
    "level",
    "stats",
    "duration",
    "generated",
    "diameter",
    "distinct",
    "initial",
    "queue",
    "retries",
    "distinctvalues",
    "levelmean",
    "levelvariance",
];

/// TLC's register file (`IdThread.localValues`, `namedRegisters`), one
/// logical worker's.
#[derive(Debug, Default)]
pub(crate) struct Registers {
    ints: BTreeMap<i64, Value>,
    /// Named registers (`"s:..."`), in first-write order.
    named: Vec<(String, Value)>,
}

/// The evaluator's handle on its host and registers.
#[derive(Debug, Default)]
pub(crate) struct HostSlot {
    pub(crate) host: Mutex<Option<Arc<dyn Host>>>,
    pub(crate) registers: Mutex<Registers>,
    /// The installed host's [`Host::expression_cache_domain`], updated with
    /// the host so cache lookups need not lock it.
    pub(crate) cache_domain: AtomicU64,
}

impl HostSlot {
    /// Installs `host` (or none) and returns the host installed before.
    pub(crate) fn replace(&self, host: Option<Arc<dyn Host>>) -> Option<Arc<dyn Host>> {
        let mut slot = self.host.lock().unwrap_or_else(PoisonError::into_inner);
        self.cache_domain.store(
            host.as_ref().map_or(0, |h| h.expression_cache_domain()),
            Ordering::Release,
        );
        std::mem::replace(&mut *slot, host)
    }

    /// The cache domain of the installed host.
    pub(crate) fn cache_domain(&self) -> u64 {
        self.cache_domain.load(Ordering::Acquire)
    }

    pub(crate) fn get(&self) -> Option<Arc<dyn Host>> {
        self.host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

pub(crate) fn host_error(e: HostError) -> EvalError {
    if e.code == 0 {
        EvalError::unsupported(e.message)
    } else {
        // Already in its final form: an `EvalException`, or the override's
        // own `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` wrapping.
        EvalError::eval_exception(Ec::from_tlc_code(e.code), e.message)
    }
}

impl Core {
    fn check_register_access(&self) -> Result<()> {
        match self.host.get() {
            Some(host) => host.check_register_access().map_err(host_error),
            None => Ok(()),
        }
    }

    pub(crate) fn check_action_composition(&self) -> Result<()> {
        match self.host.get() {
            Some(host) => host.check_action_composition().map_err(host_error),
            None => Ok(()),
        }
    }

    pub(crate) fn reuse_expression_cache(&self) -> bool {
        self.host
            .get()
            .is_none_or(|host| host.reuse_expression_cache())
    }

    /// The cache domain lazy results are read from and written to, or `None`
    /// when the host forbids expression caching.
    pub(crate) fn expression_cache(&self) -> Option<u64> {
        self.reuse_expression_cache()
            .then(|| self.host.cache_domain())
    }

    /// `RandomElement` and the `Randomization` operators, through the host.
    pub(crate) fn call_random(&self, op: StdOp, args: &[Value]) -> Result<Value> {
        let Some(host) = self.host.get() else {
            return Err(EvalError::unsupported(format!(
                "{} without a random stream (the evaluator runs outside a model checker that seeds one)",
                op.qualified_name()
            )));
        };
        match host.random(op, args, self.ctx) {
            Some(r) => r.map_err(host_error),
            None => Err(EvalError::unsupported(format!(
                "{} (this run provides no random stream)",
                op.qualified_name()
            ))),
        }
    }

    /// `TLCGetSet.TLCGetEval`.
    pub(crate) fn tlc_get(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        let Some(Arg::Expr { expr, .. }) = args.first() else {
            return Err(EvalError::bug("TLCGet arity"));
        };
        let vidx = self.eval_v(env, st, *expr)?;
        let _g = self.enter(e)?;
        match &vidx {
            Value::Int(_) | Value::BigInt(_) => {
                let idx = self.register_index(&vidx)?;
                if idx >= 0 {
                    self.check_register_access()?;
                    let regs = self
                        .host
                        .registers
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    return match regs.ints.get(&idx) {
                        Some(v) => Ok(Ev::V(v.clone())),
                        None => Err(undefined(&idx.to_string())),
                    };
                }
            }
            Value::Str(s) => return self.tlc_get_string(s.as_str(), st).map(Ev::V),
            _ => {}
        }
        Err(EvalError::eval_exception(
            Ec::OneArgumentError,
            format!(
                "The argument of TLCGet should be a nonnegative integer, but instead it is:\n{}",
                self.ppr(&vidx)
            ),
        ))
    }

    /// A register index: TLC's `IntValue.val`, an `int`.
    fn register_index(&self, v: &Value) -> Result<i64> {
        match v.as_int().and_then(|i| i.to_i64()) {
            Some(i) if i32::try_from(i).is_ok() => Ok(i),
            _ => Err(EvalError::unsupported(format!(
                "a TLCGet/TLCSet register index outside TLC's 32-bit range: {}",
                self.ppr(v)
            ))),
        }
    }

    fn host_for(&self, key: &str) -> Result<Arc<dyn Host>> {
        self.host.get().ok_or_else(|| {
            EvalError::unsupported(format!(
                "TLCGet(\"{key}\") without a model checker (the evaluator runs outside one)"
            ))
        })
    }

    fn counter(&self, key: &str, c: Counter) -> Result<i64> {
        self.host_for(key)?.counter(c).ok_or_else(|| {
            EvalError::unsupported(format!("TLCGet(\"{key}\") (this run does not provide it)"))
        })
    }

    /// `IntValue.gen(Math.toIntExact(n))`, or TLC's overflow error.
    fn narrow(n: i64) -> Result<Value> {
        if i32::try_from(n).is_ok() {
            Ok(Value::Int(n))
        } else {
            Err(EvalError::eval_exception(
                Ec::Overflow,
                format!("Overflow when computing {n}"),
            ))
        }
    }

    fn record(&self, fields: Vec<(&str, Value)>) -> Result<Value> {
        let fields = fields
            .into_iter()
            .map(|(n, v)| (self.names.intern(n), v))
            .collect();
        Ok(Value::record(fields, self.ctx)?)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per key, as in TLCGetStringValue"
    )]
    fn tlc_get_string(&self, key: &str, st: &St) -> Result<Value> {
        if let Some(host) = self.host.get() {
            host.check_tlc_access(key, false).map_err(host_error)?;
        }
        match key {
            "diameter" => Self::narrow(self.counter(key, Counter::Diameter)?),
            "generated" => Self::narrow(self.counter(key, Counter::Generated)?),
            "distinct" => Self::narrow(self.counter(key, Counter::Distinct)?),
            "queue" => Self::narrow(self.counter(key, Counter::Queue)?),
            "duration" => Self::narrow(self.counter(key, Counter::DurationSecs)?),
            "stats" => {
                // `AbstractChecker.getStatistics`; `narrowToIntValue` saturates.
                let sat = |n: i64| Value::Int(n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)));
                let queue = self.counter(key, Counter::Queue)?;
                let distinct = self.counter(key, Counter::Distinct)?;
                let initial = self.counter(key, Counter::Initial)?;
                let generated = self.counter(key, Counter::Generated)?;
                let diameter = self.counter(key, Counter::Diameter)?;
                let duration = self.counter(key, Counter::DurationSecs)?;
                self.record(vec![
                    ("queue", sat(queue)),
                    ("distinct", sat(distinct)),
                    ("initial", sat(initial)),
                    ("generated", sat(generated)),
                    ("diameter", sat(diameter)),
                    ("duration", sat(duration)),
                    // The worker's `IdThread` id: 0 for the one logical worker.
                    ("worker", Value::Int(0)),
                ])
            }
            "config" => {
                let c = self.host_for(key)?.config().ok_or_else(|| {
                    EvalError::unsupported("TLCGet(\"config\") (this run does not provide it)")
                })?;
                self.record(vec![
                    ("mode", Value::string(&self.names, "bfs")),
                    ("deadlock", Value::Bool(c.deadlock)),
                    ("worker", Value::Int(c.workers)),
                    ("seed", Value::string(&self.names, &c.seed.to_string())),
                    (
                        "fingerprint",
                        Value::string(&self.names, &c.fingerprint.to_string()),
                    ),
                    ("install", Value::string(&self.names, &c.install)),
                ])
            }
            "level" => {
                if st.is(control::CONST) || st.is(control::INIT) {
                    // `TLCState.INIT_LEVEL - 1` in `ASSUME`s and `Init`.
                    return Ok(Value::Int(0));
                }
                let level = self.host_for(key)?.level().ok_or_else(|| {
                    EvalError::unsupported(
                        "TLCGet(\"level\") (this run does not track the level of the state)",
                    )
                })?;
                Ok(Value::int(Int::Small(level)))
            }
            "all" => {
                self.check_register_access()?;
                let regs = self
                    .host
                    .registers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                // `getAllValues`: [i \in I |-> [w \in 1..workers |-> v]], one
                // logical worker.
                let pairs = regs
                    .ints
                    .iter()
                    .map(|(i, v)| (Value::Int(*i), Value::tuple(vec![v.clone()])))
                    .collect();
                Ok(Value::fcn(pairs, self.ctx)?)
            }
            "all:named" => {
                self.check_register_access()?;
                let regs = self
                    .host
                    .registers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let pairs = regs
                    .named
                    .iter()
                    .map(|(k, v)| (Value::string(&self.names, k), Value::tuple(vec![v.clone()])))
                    .collect();
                Ok(Value::fcn(pairs, self.ctx)?)
            }
            "spec" | "action" | "revision" => Err(EvalError::unsupported(format!(
                "TLCGet(\"{key}\") (reads TLC's internal description of the run)"
            ))),
            k if k.starts_with("s:") => {
                self.check_register_access()?;
                let regs = self
                    .host
                    .registers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                match regs.named.iter().find(|(n, _)| n == k) {
                    Some((_, v)) => Ok(v.clone()),
                    None => Err(undefined(k)),
                }
            }
            // `System.getProperty(key, "-D" + key)`: the one property dive
            // models is the action composition switch (D-022), set exactly
            // when `\cdot` is enabled.
            "-Dtlc2.tool.impl.Tool.cdot" => Ok(Value::string(
                &self.names,
                if self.opts.action_composition == crate::options::ActionComposition::Off {
                    key
                } else {
                    "true"
                },
            )),
            k if k.starts_with("-D") => Err(EvalError::unsupported(format!(
                "TLCGet(\"{k}\") (reads a Java system property of TLC's JVM)"
            ))),
            k => Err(undefined(k)),
        }
    }

    /// `TLCGetSet.TLCSet`.
    pub(crate) fn tlc_set(&self, vidx: &Value, val: &Value) -> Result<Value> {
        match vidx {
            Value::Int(_) | Value::BigInt(_) => {
                let idx = self.register_index(vidx)?;
                if idx >= 0 {
                    self.check_register_access()?;
                    self.host
                        .registers
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .ints
                        .insert(idx, val.clone());
                    return Ok(Value::TRUE);
                }
            }
            Value::Str(s) => {
                if let Some(host) = self.host.get() {
                    host.check_tlc_access(s.as_str(), true)
                        .map_err(host_error)?;
                }
                match s.as_str() {
                    "exit" => {
                        if matches!(val, Value::Bool(true)) {
                            let handled = self.host.get().is_some_and(|h| h.request_exit());
                            if !handled {
                                return Err(EvalError::unsupported(
                                    "TLCSet(\"exit\", TRUE) without a model checker that stops on it",
                                ));
                            }
                        }
                        return Ok(Value::TRUE);
                    }
                    "pause" => {
                        if matches!(val, Value::Bool(true)) {
                            return Err(EvalError::unsupported(
                                "TLCSet(\"pause\", TRUE) (waits for input on TLC's console)",
                            ));
                        }
                        return Ok(Value::TRUE);
                    }
                    k if k.starts_with("s:") => {
                        self.check_register_access()?;
                        let mut regs = self
                            .host
                            .registers
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner);
                        match regs.named.iter_mut().find(|(n, _)| n == k) {
                            Some(slot) => slot.1 = val.clone(),
                            None => regs.named.push((k.to_owned(), val.clone())),
                        }
                        return Ok(Value::TRUE);
                    }
                    k if k.starts_with("-D") => {
                        return Err(EvalError::unsupported(format!(
                            "TLCSet(\"{k}\", ...) (sets a Java system property of TLC's JVM)"
                        )));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Err(EvalError::eval_exception(
            Ec::ArgumentError,
            format!(
                "The first argument of TLCSet should be a nonnegative integer, but instead it is:\n{}",
                self.ppr(vidx)
            ),
        ))
    }
}

fn undefined(key: &str) -> EvalError {
    EvalError::eval_exception(Ec::TlcGetUndefined, format!("TLCGet({key}) was undefined."))
}
