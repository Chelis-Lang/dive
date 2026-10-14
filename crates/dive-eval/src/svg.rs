//! Bounded numeric result of the pinned SVG `PointOnLine` override.

use std::sync::Arc;

use dive_ir::{Arg, ExprId, StdOp};
use dive_value::Value;

use crate::core::Core;
use crate::env::{Env, Ev, St};
use crate::error::{EvalError, Result};

fn refuse(why: &str) -> EvalError {
    EvalError::unsupported(format!("SVG!PointOnLine ({why})"))
}

fn integer(value: &Value) -> Result<i32> {
    match value {
        Value::Int(n) => i32::try_from(*n).map_err(|_| refuse("requires 32-bit integers")),
        _ => Err(refuse("requires 32-bit integers")),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Java double-to-int narrowing: truncate, NaN zero, saturate outside i32"
)]
fn interpolate(from: i32, to: i32, segment: i32) -> i32 {
    // The subtraction is Java int arithmetic BEFORE promotion to binary64.
    (f64::from(from) + f64::from(to.wrapping_sub(from)) / f64::from(segment)) as i32
}

impl Core {
    pub(crate) fn eval_svg_point(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        args: &[Arg],
        expr: ExprId,
    ) -> Result<Ev> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            // MethodValue evaluates every argument before JVM representation
            // casts. Keep deferred functions unmaterialized, but delay their
            // refusal until later argument expressions have been evaluated.
            values.push(match arg {
                Arg::Expr { expr, .. } => match self.eval(env, &st.keep_lazy(), *expr)? {
                    Ev::V(value) => Some(value),
                    Ev::F(_) => None,
                },
                Arg::Op { .. } => None,
            });
        }
        let _depth = self.enter(expr)?;
        let [Some(from), Some(to), Some(segment)] = values.as_slice() else {
            return Err(refuse("deferred or operator arguments"));
        };
        self.svg_point(from, to, segment)
            .map(Ev::V)
            .map_err(|error| self.wrap_override(StdOp::SvgPointOnLine, error))
    }

    fn svg_point(&self, from: &Value, to: &Value, segment: &Value) -> Result<Value> {
        let (fx, fy) = self.point(from)?;
        let (tx, ty) = self.point(to)?;
        let segment = integer(segment)?;
        Ok(Value::record(
            vec![
                (
                    self.names.intern("x"),
                    Value::Int(i64::from(interpolate(fx, tx, segment))),
                ),
                (
                    self.names.intern("y"),
                    Value::Int(i64::from(interpolate(fy, ty, segment))),
                ),
            ],
            self.ctx,
        )?)
    }

    fn point(&self, value: &Value) -> Result<(i32, i32)> {
        let Value::Fcn(record) = value else {
            return Err(refuse("requires literal stored records"));
        };
        if !record.is_record_literal() || record.as_record().is_none() {
            return Err(refuse("requires literal stored records"));
        }
        if record.len() > 256 {
            return Err(EvalError::resource_limit(
                "SVG!PointOnLine record exceeds 256 fields",
            ));
        }
        let coordinate = |field| {
            record
                .apply(&Value::string(&self.names, field))
                .ok_or_else(|| refuse("missing x/y coordinate"))
                .and_then(integer)
        };
        Ok((coordinate("x")?, coordinate("y")?))
    }
}
