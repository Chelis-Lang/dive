//! A ready-made [`dive_eval::Host`] for a model-checking run: the run's random
//! streams (C5), the level of the state being evaluated, the live counters
//! and `TLCGet("config")`, and `TLCSet("exit")`.
//!
//! The engine owns a [`RunHost`], passes it to the binding stage
//! (`dive_model::BindOptions::host`) so that start-up definitions and
//! `ASSUME`s already draw from the run's start-up stream, and then drives it:
//! [`RunHost::expand`] before expanding a state (TLC reseeds its generator
//! with `fingerprint(state) ^ seed`), [`RunHost::set_level`] before each
//! check, [`RunHost::set_counter`] for the live counters, and
//! [`RunHost::exit_requested`] after each state.
//!
//! One `RunHost` serves one evaluating thread at a time: the stream and the
//! level are the current worker's. A parallel engine keeps one per worker.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Mutex, PoisonError};

use dive_eval::{Counter, Host, HostError, RunConfig};
use dive_ir::StdOp;
use dive_value::{Ctx, ErrorKind, Value};

use crate::random::{
    RandomError, RandomSource, Stream, Streams, random_element, random_set_of_subsets,
    random_subset,
};

/// No value: the host does not provide it at this point of the run.
const UNSET: i64 = i64::MIN;

/// A model-checking run's [`Host`].
#[derive(Debug)]
pub struct RunHost {
    streams: Streams,
    stream: Mutex<Stream>,
    level: AtomicI64,
    counters: [AtomicI64; 6],
    config: Mutex<Option<RunConfig>>,
    exit: AtomicBool,
}

fn counter_index(c: Counter) -> usize {
    match c {
        Counter::Generated => 0,
        Counter::Distinct => 1,
        Counter::Queue => 2,
        Counter::Diameter => 3,
        Counter::Initial => 4,
        Counter::DurationSecs => 5,
    }
}

impl RunHost {
    /// A host drawing from `streams` (start-up stream first) and reporting
    /// `config` for `TLCGet("config")`. No level and no counter is set.
    #[must_use]
    pub fn new(streams: Streams, config: Option<RunConfig>) -> Self {
        Self {
            streams,
            stream: Mutex::new(streams.startup()),
            level: AtomicI64::new(UNSET),
            counters: std::array::from_fn(|_| AtomicI64::new(UNSET)),
            config: Mutex::new(config),
            exit: AtomicBool::new(false),
        }
    }

    /// The run's streams.
    #[must_use]
    pub fn streams(&self) -> Streams {
        self.streams
    }

    /// Restarts the start-up stream (trace reconstruction regenerates the
    /// initial states from a fresh one, as TLC does; D-002).
    pub fn restart(&self) {
        *self.stream.lock().unwrap_or_else(PoisonError::into_inner) = self.streams.startup();
    }

    /// Switches to the stream of the state with fingerprint `fp`, before the
    /// state is expanded (`RandomEnumerableValues`' per-state reseeding).
    pub fn expand(&self, fp: u64) {
        *self.stream.lock().unwrap_or_else(PoisonError::into_inner) = self.streams.state(fp);
    }

    /// The level `TLCGet("level")` reports from now on (TLC's `s0.getLevel()`).
    pub fn set_level(&self, level: i64) {
        self.level.store(level, Ordering::Relaxed);
    }

    /// Sets a live counter (`None`: not provided, so reading it is refused).
    pub fn set_counter(&self, c: Counter, value: Option<i64>) {
        self.counters[counter_index(c)].store(value.unwrap_or(UNSET), Ordering::Relaxed);
    }

    /// Sets what `TLCGet("config")` reports.
    pub fn set_config(&self, config: Option<RunConfig>) {
        *self.config.lock().unwrap_or_else(PoisonError::into_inner) = config;
    }

    /// Whether the specification asked to stop (`TLCSet("exit", TRUE)`).
    #[must_use]
    pub fn exit_requested(&self) -> bool {
        self.exit.load(Ordering::Relaxed)
    }
}

fn host_error(e: &RandomError) -> HostError {
    match e {
        RandomError::Value(v) if v.kind() == ErrorKind::DiveLimit => HostError {
            code: 0,
            message: v.message().to_owned(),
        },
        _ => HostError {
            code: e.tlc_code(),
            message: e.message(),
        },
    }
}

impl Host for RunHost {
    fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Option<Result<Value, HostError>> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        let mut bits = |n| stream.next_bits(n);
        let r = match (op, args) {
            (StdOp::TlcRandomElement, [s]) => random_element(s, ctx, &mut bits),
            (StdOp::RandomizationRandomSubset, [k, s]) => random_subset(k, s, ctx, &mut bits),
            (StdOp::RandomizationRandomSetOfSubsets, [k, n, s]) => {
                random_set_of_subsets(k, n, s, ctx, &mut bits)
            }
            _ => return None,
        };
        Some(r.map_err(|e| host_error(&e)))
    }

    fn level(&self) -> Option<i64> {
        let l = self.level.load(Ordering::Relaxed);
        (l != UNSET).then_some(l)
    }

    fn counter(&self, c: Counter) -> Option<i64> {
        let v = self.counters[counter_index(c)].load(Ordering::Relaxed);
        (v != UNSET).then_some(v)
    }

    fn config(&self) -> Option<RunConfig> {
        self.config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn request_exit(&self) -> bool {
        self.exit.store(true, Ordering::Relaxed);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::RandomMode;
    use dive_value::Int;

    #[test]
    fn draws_from_the_startup_stream_then_the_state_stream() {
        let ctx = Ctx::tlc();
        let host = RunHost::new(Streams::new(7, RandomMode::Tlc), None);
        let s = Value::interval(Int::Small(1), Int::Small(1_000_000), ctx);
        let first = host
            .random(StdOp::TlcRandomElement, std::slice::from_ref(&s), ctx)
            .and_then(Result::ok);
        // TLC -seed 7: `RandomElement(1..1000000)` is 730700 (probe P9).
        assert_eq!(first, Some(Value::Int(730_700)));
        host.restart();
        let again = host
            .random(StdOp::TlcRandomElement, std::slice::from_ref(&s), ctx)
            .and_then(Result::ok);
        assert_eq!(again, first);
        assert_eq!(host.level(), None);
        host.set_level(3);
        assert_eq!(host.level(), Some(3));
        assert!(!host.exit_requested());
        assert!(host.request_exit());
        assert!(host.exit_requested());
    }
}
