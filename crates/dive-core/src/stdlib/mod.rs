//! Standard and CommunityModules operators of the core, by TLC's override
//! table (`dive_ir::StdOp`, `crates/dive-ir/src/std_op.rs`).
//!
//! The compiler asks [`implements`] for a standard operator it does not
//! compile itself and emits `Node::Lib(op, args)`; the evaluator evaluates
//! the arguments (normalized) and calls [`call`]. Semantics, error points
//! and error classes are the reference engine's
//! (`crates/dive-eval/src/community.rs`, `std.rs`), which follow TLC's Java
//! overrides (`docs/provenance.md`). Where the reference refuses a case,
//! so does the core.
//!
//! Every operator here is pure: [`effectful`] is false for all of them,
//! so the compiler may fold and memoize their calls.

use dive_ir::StdOp;

use crate::eval::Machine;
use crate::node::Kind;
use crate::{Error, Handle, TlcClass};

mod bags;
mod tlc;

/// Whether the core implements `op` here.
#[must_use]
pub fn implements(op: StdOp) -> bool {
    bags::implements(op) || tlc::implements(op)
}

/// Whether a call of `op` has an effect TLC performs once per evaluation
/// (output, registers, randomness): such a call is never folded or
/// memoized. None of the operators implemented so far has one.
#[must_use]
pub fn effectful(op: StdOp) -> bool {
    implements(op) && op.impure()
}

/// Calls `op` on normalized arguments.
///
/// # Errors
/// TLC's evaluation error, with its class; `unsupported` for a case the
/// core does not reproduce; internal for an operator not implemented here.
pub fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    if bags::implements(op) {
        return bags::call(m, op, args);
    }
    if tlc::implements(op) {
        return tlc::call(m, op, args);
    }
    Err(Error::internal(format!(
        "{} is not a core library operator",
        op.qualified_name()
    )))
}

/// The elements of a set TLC enumerates (`Enumerable`: not `Nat`, `Int`
/// or `Seq(S)`, and finite); `None` for any other value.
///
/// # Errors
/// `unsupported` for a set the core keeps lazy.
fn enumerable(m: &mut Machine, op: StdOp, s: Handle) -> Result<Option<Vec<Handle>>, Error> {
    let set = s.is_compound()
        && !matches!(
            m.heap.view(s).kind,
            Kind::IntFcn
                | Kind::Fcn
                | Kind::Lambda
                | Kind::BigInt
                | Kind::Nat
                | Kind::Int
                | Kind::SeqSet
        );
    if !set || !m.heap.is_finite(s) {
        return Ok(None);
    }
    if m.heap.is_lazy(s) {
        return Err(refuse(op, "a set the core keeps lazy"));
    }
    m.heap.elements(s).map(Some)
}

/// Argument `i`.
fn arg(args: &[Handle], i: usize) -> Result<Handle, Error> {
    args.get(i)
        .copied()
        .ok_or_else(|| Error::internal("missing argument"))
}

/// An `EvalException` of TLC class `code`.
fn exc(code: u32, what: impl Into<String>) -> Error {
    Error::eval_class(TlcClass::eval_exception(code), what)
}

/// `EC.TLC_MODULE_APPLYING_TO_WRONG_VALUE` (2176).
fn wrong_value(op: &str, what: &str) -> Error {
    exc(
        2176,
        format!("Applying {op} to the following value,\nwhich is not {what}:"),
    )
}

/// `EC.TLC_MODULE_ARGUMENT_ERROR` (2169).
fn arg_error(which: &str, op: &str, what: &str) -> Error {
    exc(
        2169,
        format!("The {which} argument of {op} should be a {what}, but instead it is:"),
    )
}

/// A case of `op` the core refuses, as the reference engine does.
fn refuse(op: StdOp, why: &str) -> Error {
    Error::unsupported(format!("{} ({why})", op.qualified_name()))
}
