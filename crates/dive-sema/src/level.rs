//! Levels. This stage keeps only the accessor the model needs; SANY's level
//! checking is ported in the next stage.

use crate::model::{Model, NodeId, NodeKind};

/// SANY's level of a node after level checking (`getLevel`).
pub(crate) fn level_of(m: &Model, id: NodeId) -> u8 {
    match &m.node(id).kind {
        NodeKind::Label(d) => d.body.map_or(0, |b| level_of(m, b)),
        NodeKind::Assume(d) => d.expr.map_or(0, |e| level_of(m, e)),
        NodeKind::Instance(_) => 0,
        _ => m.node(id).level.level,
    }
}
