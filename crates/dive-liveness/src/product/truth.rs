//! Truth on original behavior states and edges, without unused-domain rows.

#[cfg(test)]
use super::Graph;
#[cfg(test)]
use super::Labels;
use std::borrow::Cow;

use super::{BehaviorView, ProductError};
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

#[cfg(test)]
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
    pages: Option<Pages>,
}

/// Physical edge bits with no TRUE result have an implicit zero page.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
struct Pages {
    logical_words: usize,
    /// One-based physical word offsets; zero means the page is all false.
    directory: Vec<usize>,
}

const PAGE: usize = 64;

impl Column {
    fn logical_words(&self) -> usize {
        self.pages
            .as_ref()
            .map_or(self.words.len(), |p| p.logical_words)
    }

    fn word(&self, index: usize) -> u64 {
        if let Some(pages) = &self.pages {
            let base = pages.directory.get(index / PAGE).copied().unwrap_or(0);
            if base == 0 {
                return 0;
            }
            self.words[base - 1 + index % PAGE]
        } else {
            self.words[index]
        }
    }

    fn set_bit(&mut self, position: usize, value: bool) -> Result<(), ProductError> {
        let mut index = position / 64;
        if index >= self.logical_words() {
            return Err(ProductError::Shape("atomic predicate position".into()));
        }
        if let Some(pages) = &mut self.pages {
            if pages.directory.is_empty() {
                if !value {
                    return Ok(());
                }
                let count = pages.logical_words.div_ceil(PAGE);
                pages
                    .directory
                    .try_reserve_exact(count)
                    .map_err(|_| ALLOCATION)?;
                pages.directory.resize(count, 0);
            }
            let directory_len = pages.directory.len();
            let base = &mut pages.directory[index / PAGE];
            if *base == 0 {
                if !value {
                    return Ok(());
                }
                let length = self.words.len().checked_add(PAGE).ok_or(SIZE)?;
                if length > self.words.capacity() {
                    let maximum = directory_len.checked_mul(PAGE).ok_or(SIZE)?;
                    let desired = self
                        .words
                        .capacity()
                        .saturating_mul(2)
                        .min(maximum)
                        .max(length);
                    self.words
                        .try_reserve_exact(desired - self.words.len())
                        .map_err(|_| ALLOCATION)?;
                }
                *base = self.words.len() + 1;
                self.words.resize(length, 0);
            }
            index = *base - 1 + index % PAGE;
        }
        let mask = 1_u64 << (position % 64);
        if value {
            self.words[index] |= mask;
        } else {
            self.words[index] &= !mask;
        }
        Ok(())
    }
}

/// Each global atom retains its original identity but owns only its actual
/// state or action domain. Edge offsets are shared across action columns.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) struct PackedTruth<'a> {
    vertices: usize,
    columns: Vec<Column>,
    offsets: Cow<'a, [usize]>,
}

#[derive(Debug, PartialEq, Eq)]
struct Layout {
    state_words: usize,
    action_words: usize,
    offsets: usize,
    dense_bytes: usize,
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
        let dense_bytes = kinds
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
            dense_bytes,
        })
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, ProductError> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| ALLOCATION)?;
    Ok(values)
}

impl<'a> PackedTruth<'a> {
    pub(crate) fn column_words(
        &self,
        atom: usize,
        action: bool,
    ) -> Option<impl ExactSizeIterator<Item = u64> + '_> {
        let kind = if action {
            AtomKind::Action
        } else {
            AtomKind::State
        };
        let column = self.columns.get(atom).filter(|c| c.kind == kind)?;
        Some((0..column.logical_words()).map(move |i| column.word(i)))
    }

    pub(crate) fn restore_prefix(
        &mut self,
        atom: usize,
        kind: AtomKind,
        cells: usize,
        words: &[u64],
    ) -> Result<(), ProductError> {
        let column = self
            .columns
            .get_mut(atom)
            .filter(|c| c.kind == kind)
            .ok_or_else(|| ProductError::Shape("periodic truth column".into()))?;
        for (i, &word) in words.iter().enumerate() {
            if i.is_multiple_of(1024) && crate::budget::exhausted() {
                return Err(ProductError::Limit(crate::budget::EXHAUSTED));
            }
            let mut bits = word;
            while bits != 0 {
                let at = 64 * i + bits.trailing_zeros() as usize;
                if at >= cells {
                    break;
                }
                column.set_bit(at, true)?;
                bits &= bits - 1;
            }
        }
        Ok(())
    }

    pub(crate) fn action_position(&self, source: usize, edge: usize) -> Option<usize> {
        let start = *self.offsets.get(source)?;
        let end = *self.offsets.get(source.checked_add(1)?)?;
        (edge < end.checked_sub(start)?).then(|| start + edge)
    }

    pub(crate) fn new(
        graph: impl Into<BehaviorView<'a>>,
        kinds: &[AtomKind],
    ) -> Result<Self, ProductError> {
        let graph = graph.into();
        let borrowed = graph
            .csr_offsets()
            .filter(|_| kinds.contains(&AtomKind::Action));
        let edges = if let Some(offsets) = borrowed {
            offsets.last().copied().unwrap_or(0)
        } else if kinds.contains(&AtomKind::Action) {
            (0..graph.vertex_count())
                .try_fold(0_usize, |n, v| n.checked_add(graph.row_len(v)))
                .ok_or(SIZE)?
        } else {
            0
        };
        // Retain the original checked logical domain before choosing its representation.
        let layout = Layout::checked(graph.vertex_count(), edges, kinds)?;
        debug_assert!(layout.dense_bytes >= layout.offsets * size_of::<usize>());
        let offsets = if let Some(offsets) = borrowed {
            Cow::Borrowed(offsets)
        } else {
            let mut offsets = reserve(layout.offsets)?;
            if layout.offsets != 0 {
                let mut position = 0;
                offsets.push(position);
                for v in 0..graph.vertex_count() {
                    if crate::budget::exhausted() {
                        return Err(ProductError::Limit(crate::budget::EXHAUSTED));
                    }
                    position += graph.row_len(v);
                    offsets.push(position);
                }
            }
            Cow::Owned(offsets)
        };
        let mut columns = reserve(kinds.len())?;
        for &kind in kinds {
            if crate::budget::exhausted() {
                return Err(ProductError::Limit(crate::budget::EXHAUSTED));
            }
            let len = match kind {
                AtomKind::State => layout.state_words,
                AtomKind::Action => layout.action_words,
            };
            let (words, pages) = if kind == AtomKind::Action && borrowed.is_some() && len > PAGE {
                (
                    Vec::new(),
                    Some(Pages {
                        logical_words: len,
                        directory: Vec::new(),
                    }),
                )
            } else {
                let mut words = reserve(len)?;
                words.resize(len, 0);
                (words, None)
            };
            columns.push(Column { kind, words, pages });
        }
        Ok(Self {
            vertices: graph.vertex_count(),
            columns,
            offsets,
        })
    }

    pub(crate) fn set_state_word(
        &mut self,
        atom: usize,
        first: usize,
        word: u64,
    ) -> Result<(), ProductError> {
        if !first.is_multiple_of(64) || self.vertices.saturating_sub(first) < 64 {
            return Err(ProductError::Shape(
                "state predicate word dimensions".into(),
            ));
        }
        let slot = self
            .columns
            .get_mut(atom)
            .filter(|column| column.kind == AtomKind::State)
            .and_then(|column| column.words.get_mut(first / 64))
            .ok_or_else(|| ProductError::Shape("state predicate word column".into()))?;
        *slot = word;
        Ok(())
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
        column.set_bit(position, value)
    }
}

impl TruthAccess for PackedTruth<'_> {
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
            column.logical_words()
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
        self.columns[atom].word(vertex / 64) & (1_u64 << (vertex % 64)) != 0
    }

    fn action(&self, atom: usize, source: usize, edge: usize) -> bool {
        let position = self.offsets[source] + edge;
        self.columns[atom].word(position / 64) & (1_u64 << (position % 64)) != 0
    }
}

#[cfg(test)]
mod tests;
