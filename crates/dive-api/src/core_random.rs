//! The core's random streams (`docs/developer/core/03-value-model.md` §6.1):
//! the reference engine's seeding and state fingerprint behind
//! `dive_core::random::Draws`, so that the core draws what the reference
//! engine draws.

use std::sync::Arc;

use dive_core::random::{DrawStream, Draws};
use dive_core::{Error, Handle, Heap, TlcClass};
use dive_ir::StdOp;
use dive_stdlib::random::{RandomError, Stream, Streams, random_element};
use dive_store::state::StateFingerprinter;
use dive_value::{Ctx, Interner, Value};

/// The run's streams: where start-up left the startup stream, and how each
/// expanded state's stream is keyed.
pub(crate) struct CoreDraws {
    pub(crate) streams: Streams,
    pub(crate) startup: Stream,
    pub(crate) seeder: StateFingerprinter,
    pub(crate) names: Arc<Interner>,
    pub(crate) ctx: Ctx,
}

impl Draws for CoreDraws {
    fn startup(&self) -> Box<dyn DrawStream> {
        Box::new(CoreStream(self.startup))
    }

    fn state(&self, heap: &mut Heap, vars: &[Handle]) -> Result<Box<dyn DrawStream>, Error> {
        let mut values = Vec::with_capacity(vars.len());
        for &h in vars {
            let v = dive_core::print::to_value(
                heap,
                &self.names,
                h,
                self.ctx,
                dive_core::print::Conversion::Report,
            )
            .map_err(Error::unsupported)?;
            values.push(v);
        }
        let fp = self.seeder.fingerprint(&values, self.ctx).map_err(|e| {
            Error::unsupported(format!(
                "fingerprinting a state to seed its random stream: {e}"
            ))
        })?;
        Ok(Box::new(CoreStream(self.streams.state(fp))))
    }
}

/// One stream in use.
#[derive(Debug)]
struct CoreStream(Stream);

impl DrawStream for CoreStream {
    fn draw(&mut self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, Error> {
        match (op, args) {
            (StdOp::TlcRandomElement, [s]) => {
                random_element(s, ctx, &mut self.0).map_err(|e| draw_error(&e))
            }
            _ => Err(Error::unsupported(format!(
                "{} (M1 core)",
                op.qualified_name()
            ))),
        }
    }
}

/// TLC's error of a draw, as the reference engine's host reports it.
fn draw_error(e: &RandomError) -> Error {
    match e {
        RandomError::Value(v) if v.kind() == dive_value::ErrorKind::DiveLimit => {
            Error::unsupported(v.message().to_owned())
        }
        _ => Error::eval_class(TlcClass::eval_exception(e.tlc_code()), e.message()),
    }
}
