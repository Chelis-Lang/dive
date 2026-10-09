//! The evaluator's host during a check (ADR 0014 §7): WP-C3's
//! [`RunHost`], with the level kept per worker thread while a level is
//! expanded in parallel.
//!
//! `RunHost` holds one random stream, one level and one register file: one
//! logical worker's, as its documentation says. That is exactly right for
//! everything that forces sequential mode (randomness, registers,
//! `TLCSet("exit")`, live counters; `dive_model::Model::sequential`), which
//! the inline engine evaluates in TLC's order. The one thing the parallel
//! engine evaluates on several threads at once and that the host answers is
//! `TLCGet("level")`: the level of the state the *current thread* is
//! expanding or checking. [`EngineHost`] answers it from a thread-local value
//! while a parallel phase runs on the thread ([`ThreadLevel`]), and from
//! `RunHost` otherwise.

use std::cell::Cell;
use std::sync::Arc;

use dive_eval::{Counter, Host, HostError, RunConfig};
use dive_ir::StdOp;
use dive_stdlib::host::RunHost;
use dive_value::{Ctx, Value};

/// The host the evaluator sees (see the module documentation).
#[derive(Debug)]
pub(crate) struct EngineHost {
    run: Arc<RunHost>,
}

impl EngineHost {
    pub(crate) fn new(run: Arc<RunHost>) -> Self {
        Self { run }
    }
}

thread_local! {
    static LEVEL: Cell<Option<i64>> = const { Cell::new(None) };
}

impl Host for EngineHost {
    fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Option<Result<Value, HostError>> {
        self.run.random(op, args, ctx)
    }

    fn level(&self) -> Option<i64> {
        LEVEL.with(Cell::get).or_else(|| self.run.level())
    }

    fn counter(&self, counter: Counter) -> Option<i64> {
        self.run.counter(counter)
    }

    fn config(&self) -> Option<RunConfig> {
        self.run.config()
    }

    fn request_exit(&self) -> bool {
        self.run.request_exit()
    }
}

/// While it lives, `TLCGet("level")` on this thread reports the level set
/// with [`set_thread_level`], not `RunHost`'s.
pub(crate) struct ThreadLevel {
    /// Bound to the thread it was entered on.
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl ThreadLevel {
    pub(crate) fn enter(level: i64) -> Self {
        LEVEL.with(|l| l.set(Some(level)));
        Self {
            _thread: std::marker::PhantomData,
        }
    }
}

/// Sets this thread's level, inside a [`ThreadLevel`].
pub(crate) fn set_thread_level(level: i64) {
    LEVEL.with(|l| l.set(Some(level)));
}

impl Drop for ThreadLevel {
    fn drop(&mut self) {
        LEVEL.with(|l| l.set(None));
    }
}
