//! Random draws (`TLC!RandomElement`): the run's streams, which the engine
//! supplies (`03-value-model.md` §6.1, `02-semantic-contract.md` B6).
//!
//! The core defines the interface only. `dive-api` implements it with the
//! reference engine's seeding and state fingerprint, so that the core draws
//! what the reference engine draws.

use dive_ir::StdOp;
use dive_value::{Ctx, Value};

use crate::Error;
use crate::handle::Handle;
use crate::heap::Heap;

/// A random stream in use: the startup stream, or one expanded state's.
pub trait DrawStream: Send + std::fmt::Debug {
    /// Applies the random operator `op` to `args`, drawing from the stream.
    ///
    /// # Errors
    /// TLC's evaluation error of the operator, or a refusal.
    fn draw(&mut self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, Error>;
}

/// The run's random streams.
pub trait Draws: Send + Sync {
    /// The startup stream, where the constant definitions and `ASSUME`s
    /// left it: `Init` draws from it.
    fn startup(&self) -> Box<dyn DrawStream>;

    /// The stream for expanding the state whose variables (in IR order)
    /// are `vars`: TLC's `fingerprint(state) ^ seed`, or D-002's stream
    /// keyed by the fingerprint.
    ///
    /// # Errors
    /// A state that cannot be converted or fingerprinted.
    fn state(&self, heap: &mut Heap, vars: &[Handle]) -> Result<Box<dyn DrawStream>, Error>;
}
