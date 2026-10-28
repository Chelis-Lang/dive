//! Truth on original behavior states and edges, without unused-domain rows.

#[cfg(test)]
use super::Graph;
use super::{BehaviorView, Labels, ProductError};
use crate::formula::{Atom, AtomKind};

pub(crate) trait TruthAccess {
    fn contains(&self, atom: Atom) -> bool;
    fn validate_dimensions<'a>(
        &self,
        graph: impl Into<BehaviorView<'a>>,
    ) -> Result<(), ProductError>;
    fn state(&self, atom: usize, vertex: usize) -> bool;
    fn action(&self, atom: usize, source: usize, edge: usize) -> bool;

    fn truth(&self, atom: Atom, source: usize, edge: usize) -> bool {
        match atom.kind {
            AtomKind::State => self.state(atom.index, source),
            AtomKind::Action => self.action(atom.index, source, edge),
        }
    }
}

impl TruthAccess for Labels {
    fn contains(&self, atom: Atom) -> bool {
        atom.index
            < match atom.kind {
                AtomKind::State => self.states.len(),
                AtomKind::Action => self.actions.len(),
            }
    }

    fn validate_dimensions<'a>(
        &self,
        graph: impl Into<BehaviorView<'a>>,
    ) -> Result<(), ProductError> {
        let graph = graph.into();
        if self
            .states
            .iter()
            .any(|row| row.len() != graph.vertex_count())
        {
            return Err(ProductError::Shape("state predicate dimensions".into()));
        }
        if self.actions.iter().any(|rows| {
            rows.len() != graph.vertex_count()
                || rows
                    .iter()
                    .enumerate()
                    .any(|(v, row)| row.len() != graph.row_len(v))
        }) {
            return Err(ProductError::Shape(
                "action predicate adjacency dimensions".into(),
            ));
        }
        Ok(())
    }

    fn state(&self, atom: usize, vertex: usize) -> bool {
        self.states[atom][vertex]
    }

    fn action(&self, atom: usize, source: usize, edge: usize) -> bool {
        self.actions[atom][source][edge]
    }
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
struct Column {
    kind: AtomKind,
    words: Vec<u64>,
}

/// Each global atom retains its original identity but owns only its actual
/// state or action domain. Edge offsets are shared across action columns.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) struct PackedTruth {
    vertices: usize,
    columns: Vec<Column>,
    offsets: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
struct Layout {
    state_words: usize,
    action_words: usize,
    offsets: usize,
    minimum_bytes: usize,
}

fn words(bits: usize) -> usize {
    bits / 64 + usize::from(!bits.is_multiple_of(64))
}

const SIZE: ProductError = ProductError::Limit("atom label bytes");
const ALLOCATION: ProductError = ProductError::Limit("atom label allocation");

impl Layout {
    fn checked(states: usize, edges: usize, kinds: &[AtomKind]) -> Result<Self, ProductError> {
        let state_words = words(states);
        let action_words = words(edges);
        let offsets = if kinds.contains(&AtomKind::Action) {
            states.checked_add(1).ok_or(SIZE)?
        } else {
            0
        };
        let minimum_bytes = kinds
            .iter()
            .try_fold(
                kinds.len().checked_mul(size_of::<Column>()).ok_or(SIZE)?,
                |bytes, kind| {
                    let count = match kind {
                        AtomKind::State => state_words,
                        AtomKind::Action => action_words,
                    };
                    count
                        .checked_mul(size_of::<u64>())
                        .and_then(|n| bytes.checked_add(n))
                        .ok_or(SIZE)
                },
            )?
            .checked_add(offsets.checked_mul(size_of::<usize>()).ok_or(SIZE)?)
            .ok_or(SIZE)?;
        Ok(Self {
            state_words,
            action_words,
            offsets,
            minimum_bytes,
        })
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, ProductError> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| ALLOCATION)?;
    Ok(values)
}

impl PackedTruth {
    pub(crate) fn new<'a>(
        graph: impl Into<BehaviorView<'a>>,
        kinds: &[AtomKind],
    ) -> Result<Self, ProductError> {
        let graph = graph.into();
        let edges = if kinds.contains(&AtomKind::Action) {
            (0..graph.vertex_count())
                .try_fold(0_usize, |n, v| n.checked_add(graph.row_len(v)))
                .ok_or(SIZE)?
        } else {
            0
        };
        // Check every buffer's required size before allocating any of them.
        // This is a lower bound on live memory, not a guessed process size.
        let layout = Layout::checked(graph.vertex_count(), edges, kinds)?;
        debug_assert!(layout.minimum_bytes >= layout.offsets * size_of::<usize>());
        let mut offsets = reserve(layout.offsets)?;
        if layout.offsets != 0 {
            let mut position = 0;
            offsets.push(position);
            for v in 0..graph.vertex_count() {
                if crate::budget::exhausted() {
                    return Err(ProductError::Limit(crate::budget::EXHAUSTED));
                }
                position += graph.row_len(v); // checked sum above bounds every partial sum
                offsets.push(position);
            }
        }
        let mut columns = reserve(kinds.len())?;
        for &kind in kinds {
            if crate::budget::exhausted() {
                return Err(ProductError::Limit(crate::budget::EXHAUSTED));
            }
            let len = match kind {
                AtomKind::State => layout.state_words,
                AtomKind::Action => layout.action_words,
            };
            let mut words = reserve(len)?;
            words.resize(len, 0);
            columns.push(Column { kind, words });
        }
        Ok(Self {
            vertices: graph.vertex_count(),
            columns,
            offsets,
        })
    }

    pub(crate) fn set_state(
        &mut self,
        atom: usize,
        vertex: usize,
        value: bool,
    ) -> Result<(), ProductError> {
        if vertex >= self.vertices {
            return Err(ProductError::Shape("state predicate dimensions".into()));
        }
        self.set(atom, AtomKind::State, vertex, value)
    }

    pub(crate) fn set_action(
        &mut self,
        atom: usize,
        source: usize,
        edge: usize,
        value: bool,
    ) -> Result<(), ProductError> {
        let start = self
            .offsets
            .get(source)
            .copied()
            .ok_or_else(|| ProductError::Shape("action predicate source".into()))?;
        let end = self
            .offsets
            .get(source + 1)
            .copied()
            .ok_or_else(|| ProductError::Shape("action predicate source".into()))?;
        if edge >= end - start {
            return Err(ProductError::Shape("action predicate edge".into()));
        }
        self.set(atom, AtomKind::Action, start + edge, value)
    }

    fn set(
        &mut self,
        atom: usize,
        kind: AtomKind,
        position: usize,
        value: bool,
    ) -> Result<(), ProductError> {
        let column = self
            .columns
            .get_mut(atom)
            .filter(|column| column.kind == kind)
            .ok_or_else(|| ProductError::Shape("atomic predicate outside label table".into()))?;
        let word = column
            .words
            .get_mut(position / 64)
            .ok_or_else(|| ProductError::Shape("atomic predicate position".into()))?;
        let mask = 1_u64 << (position % 64);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        Ok(())
    }
}

impl TruthAccess for PackedTruth {
    fn contains(&self, atom: Atom) -> bool {
        self.columns
            .get(atom.index)
            .is_some_and(|column| column.kind == atom.kind)
    }

    fn validate_dimensions<'a>(
        &self,
        graph: impl Into<BehaviorView<'a>>,
    ) -> Result<(), ProductError> {
        let graph = graph.into();
        let actions = self
            .columns
            .iter()
            .any(|column| column.kind == AtomKind::Action);
        if self.vertices != graph.vertex_count()
            || if actions {
                self.offsets.len() != graph.vertex_count().saturating_add(1)
                    || self.offsets.first() != Some(&0)
                    || (0..graph.vertex_count()).any(|v| {
                        self.offsets[v + 1].checked_sub(self.offsets[v]) != Some(graph.row_len(v))
                    })
            } else {
                !self.offsets.is_empty()
            }
        {
            return Err(ProductError::Shape("packed predicate dimensions".into()));
        }
        let edges = self.offsets.last().copied().unwrap_or(0);
        if self.columns.iter().any(|column| {
            column.words.len()
                != words(match column.kind {
                    AtomKind::State => graph.vertex_count(),
                    AtomKind::Action => edges,
                })
        }) {
            return Err(ProductError::Shape("packed predicate columns".into()));
        }
        Ok(())
    }

    fn state(&self, atom: usize, vertex: usize) -> bool {
        self.columns[atom].words[vertex / 64] & (1_u64 << (vertex % 64)) != 0
    }

    fn action(&self, atom: usize, source: usize, edge: usize) -> bool {
        let position = self.offsets[source] + edge;
        self.columns[atom].words[position / 64] & (1_u64 << (position % 64)) != 0
    }
}

#[cfg(test)]
mod tests;
