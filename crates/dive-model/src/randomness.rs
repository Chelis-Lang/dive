//! Source/config refusal of transformed randomness before startup evaluation.

use dive_config::{ConstantEntry, ModelConfig};
use dive_ir::{Ir, OpRef, StdOp, Symbol};

fn random(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets
            | StdOp::RandomizationRandomSubsetSet
    )
}

/// Names an unsupported random interaction with VIEW or SYMMETRY, without
/// evaluating source, binding constants or invoking a host callback.
///
/// This preserves the checker's conservative source/config policy, including
/// unused references and configuration-selected native replacements. Absence
/// grants no purity, replay, binding or host authority; callers still perform
/// ordinary structural IR and model checks. Malformed references are never
/// dereferenced here and remain the structural validator's responsibility.
#[must_use]
pub fn transformed_randomness(ir: &Ir, cfg: &ModelConfig) -> Option<&'static str> {
    if cfg.view.is_none() && cfg.symmetry.is_none() {
        return None;
    }
    source_randomness(ir, cfg).then_some(if cfg.view.is_some() {
        "randomness under VIEW is not supported"
    } else {
        "randomness under SYMMETRY is not supported"
    })
}

pub(crate) fn source_randomness(ir: &Ir, cfg: &ModelConfig) -> bool {
    let source = ir.nodes().iter().any(|node| {
        node.expr.op_refs().into_iter().any(|op| match op {
            OpRef::Std(op) => random(*op),
            OpRef::Def(id) => ir
                .defs()
                .get(id.index())
                .and_then(|d| d.std)
                .is_some_and(random),
            _ => false,
        })
    });
    let configured = cfg.constants.iter().any(|entry| {
        let ConstantEntry::Substitution { target, .. } = entry else {
            return false;
        };
        ir.modules().iter().any(|module| {
            module.context.iter().any(|(name, symbol)| {
                name == &target.text
                    && matches!(symbol, Symbol::Def(id)
                        if ir.defs().get(id.index()).and_then(|d| d.std).is_some_and(random))
            })
        }) || ir.defs().iter().any(|def| {
            def.source.is_none() && def.name == target.text && def.std.is_some_and(random)
        })
    });
    source || configured
}
