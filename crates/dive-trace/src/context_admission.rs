//! Non-evaluating boundaries for independent context inspection.

use dive_ir::{Arg, Pass, SubstBinding};

/// A caller-supplied inertness annotation cannot authorize eager execution
/// while deriving a proof. Ordinary evaluator entry keeps its own semantics.
pub(crate) fn substitution_is_lazy(bindings: &[SubstBinding]) -> bool {
    bindings.iter().all(|binding| {
        !matches!(
            binding.value,
            Arg::Expr {
                pass: Pass::Strict,
                ..
            }
        )
    })
}

pub(crate) const STRICT_SUBSTITUTION: &str =
    "Strict substitution requires evaluation before independent context admission";
