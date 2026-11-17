//! Core's random streams and `TLCSet`/`TLCGet` registers:
//! `dive-stdlib`'s seeded streams and `dive-store`'s TLC state fingerprint
//! behind `dive_core::random::Draws`, so that Core's draws continue the
//! streams start-up evaluation drew from.

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
    /// Constant definitions or `ASSUME`s drew: `Init` would continue their
    /// stream, where TLC's trace rebuild starts a fresh one (fixture
    /// `RndTraceAssume`), so a draw in `Init` is refused.
    pub(crate) startup_drew: bool,
    pub(crate) seeder: StateFingerprinter,
    pub(crate) names: Arc<Interner>,
    pub(crate) ctx: Ctx,
}

impl Draws for CoreDraws {
    fn startup(&self) -> Box<dyn DrawStream> {
        Box::new(CoreStream {
            stream: self.startup,
            refuse: self.startup_drew,
        })
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
        Ok(Box::new(CoreStream {
            stream: self.streams.state(fp),
            refuse: false,
        }))
    }
}

/// One stream in use.
#[derive(Debug)]
struct CoreStream {
    stream: Stream,
    /// Refuse every draw (`CoreDraws::startup_drew`).
    refuse: bool,
}

impl DrawStream for CoreStream {
    fn draw(&mut self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, Error> {
        if self.refuse {
            return Err(Error::unsupported(
                "RandomElement in Init after constant definitions or ASSUMEs drew (M1 core)",
            ));
        }
        match (op, args) {
            (StdOp::TlcRandomElement, [s]) => {
                random_element(s, ctx, &mut self.stream).map_err(|e| draw_error(&e))
            }
            _ => Err(Error::unsupported(format!(
                "{} (M1 core)",
                op.qualified_name()
            ))),
        }
    }
}

/// TLC's error of a draw, as `dive-stdlib`'s random operators report it.
fn draw_error(e: &RandomError) -> Error {
    match e {
        RandomError::Value(v) if v.kind() == dive_value::ErrorKind::DiveLimit => {
            Error::unsupported(v.message().to_owned())
        }
        _ => Error::eval_class(TlcClass::eval_exception(e.tlc_code()), e.message()),
    }
}

/// The run's registers: the dive-eval evaluator's, which start-up wrote.
pub(crate) struct CoreRegisters(pub(crate) dive_eval::Evaluator);

impl dive_core::random::Registers for CoreRegisters {
    fn call(&self, op: StdOp, args: &[Value]) -> Result<Value, Error> {
        self.0.tlc_register(op, args).map_err(|e| {
            if e.is_eval() {
                Error::eval_class(
                    TlcClass {
                        code: e.ec().code(),
                        eval_exception: e.is_eval_exception(),
                    },
                    e.message(),
                )
            } else {
                Error::unsupported(format!("{} (M1 core)", e.message()))
            }
        })
    }
}
