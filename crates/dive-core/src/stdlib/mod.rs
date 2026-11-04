//! Standard and CommunityModules operators of the core, by TLC's override
//! table (`dive_ir::StdOp`, `crates/dive-ir/src/std_op.rs`).
//!
//! The compiler asks [`implements`] for a standard operator it does not
//! compile itself and emits `Node::Lib`; the evaluator evaluates
//! the arguments (normalized) and calls [`call`]. Semantics, error points
//! and error classes are the reference engine's
//! (`crates/dive-eval/src/community.rs`, `std.rs`), which follow TLC's Java
//! overrides (`docs/provenance.md`). Where the reference refuses a case,
//! so does the core.
//!
//! Every operator here is pure except `Print` and `PrintT` ([`effectful`]),
//! whose calls the compiler never folds, memoizes or skips.

use dive_ir::StdOp;

use crate::eval::Machine;
use crate::node::Kind;
use crate::plan::{Code, P};
use crate::{Error, Handle, TlcClass};

mod bags;
mod community;
mod graphs;
mod higher;
mod tlc;

/// Argument positions whose established override contract consumes normalized
/// set elements, rather than using or returning the original set receiver.
pub(crate) fn normalized_set_argument(op: StdOp, index: usize) -> bool {
    matches!(
        (op, index),
        (
            StdOp::SequencesExtSetToSeq
                | StdOp::SequencesExtSetToSeqs
                | StdOp::TlcPermutations
                | StdOp::BagsSetToBag
                | StdOp::FiniteSetsExtQuantify,
            0
        ) | (StdOp::FiniteSetsExtFoldSet, 2)
            | (StdOp::FunctionsFoldFunctionOnSet, 3)
    )
}

/// Whether the core implements `op` here.
#[must_use]
pub fn implements(op: StdOp) -> bool {
    bags::implements(op)
        || tlc::implements(op)
        || community::implements(op)
        || graphs::implements(op)
}

/// Whether a call of `op` has an effect TLC performs once per evaluation
/// (output, registers, randomness): such a call is never folded or
/// memoized. Of the operators implemented here, `Print` and `PrintT`.
#[must_use]
pub fn effectful(op: StdOp) -> bool {
    implements(op) && op.impure()
}

/// Whether the core implements `op` here as an operator with operator
/// arguments (`Node::LibHo`).
#[must_use]
pub fn higher_order(op: StdOp) -> bool {
    higher::implements(op)
}

/// The arity of argument `i` of a higher-order operator when it is an
/// operator argument; `None` for a value argument.
#[must_use]
pub fn op_arity(op: StdOp, i: usize) -> Option<usize> {
    higher::op_arity(op, i)
}

/// Whether `op` takes its operator argument as TLC's `OpValue`, which TLC
/// cannot apply when the operator's body is a constant: the compiler
/// refuses such a call.
#[must_use]
pub fn takes_op_value(op: StdOp) -> bool {
    higher::takes_op_value(op)
}

/// An operator argument, compiled: its parameters' slots and its body.
#[derive(Clone, Copy, Debug)]
pub struct OpFn<'a> {
    code: &'a Code,
    params: &'a [u32],
    body: P,
}

impl<'a> OpFn<'a> {
    /// The operator whose parameters are `params` and whose body is `body`.
    #[must_use]
    pub fn new(code: &'a Code, params: &'a [u32], body: P) -> Self {
        Self { code, params, body }
    }

    /// Applies the operator to `args`, as TLC applies an operator argument
    /// from a Java override: the parameters are bound to the values, the
    /// body is evaluated (normalized), and the slots are restored.
    ///
    /// # Errors
    /// The body's evaluation errors; internal for an arity mismatch.
    pub fn apply(&self, m: &mut Machine, args: &[Handle]) -> Result<Handle, Error> {
        if args.len() != self.params.len() {
            return Err(Error::internal("an operator argument's arity"));
        }
        let saved: Vec<Handle> = self.params.iter().map(|&s| m.slot(s)).collect();
        for (&s, &v) in self.params.iter().zip(args) {
            m.set_slot(s, v);
        }
        let r = m.eval_norm(self.code, self.body);
        for (&s, &v) in self.params.iter().zip(&saved) {
            m.set_slot(s, v);
        }
        r
    }
}

/// An argument of a higher-order operator at the call.
#[derive(Clone, Copy, Debug)]
pub enum Ho<'a> {
    /// A normalized value.
    Value(Handle),
    /// An operator.
    Op(OpFn<'a>),
}

/// Calls the higher-order operator `op`.
///
/// # Errors
/// As [`call`].
pub fn call_ho(m: &mut Machine, op: StdOp, args: &[Ho<'_>]) -> Result<Handle, Error> {
    higher::call(m, op, args)
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
    if community::implements(op) {
        return community::call(m, op, args);
    }
    if graphs::implements(op) {
        return graphs::call(m, op, args);
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
    m.work_elements(s).map(Some)
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
