//! Checked borrowed adjacency for a producer retaining its own state rows.

use super::CsrTargets;
use std::ops::Range;

use dive_eval::HostControl;

use super::{CompleteEvidence, GraphCompletion, Limits, PrefixEvidence, bug, limit};
use crate::budget::EvaluationControl;
use crate::{CheckOptions, CheckResult, Verdict};

/// Storage for original successor ordinals, including the synthetic stutter.
#[derive(Clone, Copy, Debug)]
pub enum CsrOrdinals<'a> {
    /// Original optional ordinals from external producers.
    Expanded(&'a [Option<u64>]),
    /// Ordinals 0 through 65533 use one entry. 65535 denotes stutter.
    /// 65534 refers to a full-width ordinal in the sorted (edge, ordinal) table.
    Compact(&'a [u16], &'a [(usize, u64)]),
    /// Ordinals default to their source-local position. The final entry of
    /// each expanded source is its stutter. Only blocks with exceptions
    /// retain words: 65535 uses the default, 65534 indexes the wide table.
    PositionBlocks {
        /// Physical occurrences covered by the graph.
        edges: usize,
        /// Source of the first edge in each block, for bounded random lookup.
        sources: &'a [u32],
        /// Cumulative count of explicit 256-edge blocks.
        directory: &'a [usize],
        /// Words for explicit blocks, concatenated in physical order.
        words: &'a [u16],
        /// Absolute physical edge and full ordinal for extended words.
        large: &'a [(usize, u64)],
    },
}

impl CsrOrdinals<'_> {
    /// First ordinal stored in the full-width side table.
    pub const EXTENDED: u16 = u16::MAX - 1;

    fn len(self) -> usize {
        match self {
            Self::Expanded(v) => v.len(),
            Self::Compact(v, _) => v.len(),
            Self::PositionBlocks { edges, .. } => edges,
        }
    }

    fn word(self, edge: usize) -> Option<u16> {
        match self {
            Self::Compact(words, _) => words.get(edge).copied(),
            Self::PositionBlocks {
                edges,
                directory,
                words,
                ..
            } if edge < edges => {
                let block = edge / CsrEdgeRows::BLOCK;
                let before = block
                    .checked_sub(1)
                    .map_or(0, |i| directory.get(i).copied().unwrap_or(0));
                if *directory.get(block)? == before {
                    Some(u16::MAX)
                } else {
                    words
                        .get(
                            before
                                .checked_mul(CsrEdgeRows::BLOCK)?
                                .checked_add(edge % CsrEdgeRows::BLOCK)?,
                        )
                        .copied()
                }
            }
            _ => None,
        }
    }

    #[allow(
        clippy::option_option,
        reason = "distinguishes missing entries, stutters and generated transitions"
    )]
    fn get(self, index: usize, position: Option<u64>) -> Option<Option<u64>> {
        let (large, implicit) = match self {
            Self::Expanded(v) => return v.get(index).copied(),
            Self::Compact(_, large) => (large, None),
            Self::PositionBlocks { large, .. } => (large, position),
        };
        match self.word(index)? {
            u16::MAX => Some(implicit),
            Self::EXTENDED => large
                .binary_search_by_key(&index, |&(edge, _)| edge)
                .ok()
                .map(|i| Some(large[i].1)),
            n => Some(Some(u64::from(n))),
        }
    }
    fn check(
        self,
        edges: usize,
        offsets: &[usize],
        closed: usize,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        let large = match self {
            CsrOrdinals::PositionBlocks {
                sources,
                directory,
                words,
                large,
                ..
            } => {
                check_blocks(edges, directory, words.len(), "ordinal", control)?;
                if sources.len() != directory.len() {
                    return Err(bug("invalid CSR ordinal block source count"));
                }
                for (block, &source) in sources.iter().enumerate() {
                    checkpoint(control, block)?;
                    let source = source as usize;
                    let edge = block * CsrEdgeRows::BLOCK;
                    if source >= closed || offsets[source] > edge || offsets[source + 1] <= edge {
                        return Err(bug("invalid CSR ordinal block source"));
                    }
                }
                Some(large)
            }
            CsrOrdinals::Compact(_, large) => Some(large),
            CsrOrdinals::Expanded(_) => None,
        };
        if let Some(large) = large {
            let mut previous = None;
            for (i, &(edge, ordinal)) in large.iter().enumerate() {
                checkpoint(control, i)?;
                if previous.is_some_and(|p| p >= edge)
                    || self.word(edge) != Some(CsrOrdinals::EXTENDED)
                    || ordinal < u64::from(CsrOrdinals::EXTENDED)
                {
                    return Err(bug("invalid extended CSR ordinal"));
                }
                previous = Some(edge);
            }
        }
        Ok(())
    }

    /// Keeps the first `edges` entries without expanding their representation.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        match self {
            Self::PositionBlocks {
                edges: full,
                sources,
                directory,
                words,
                large,
            } => {
                if edges > full {
                    return None;
                }
                let (directory, count) = block_prefix(edges, directory)?;
                Some(Self::PositionBlocks {
                    edges,
                    sources: sources.get(..directory.len())?,
                    directory,
                    words: words.get(..count)?,
                    large: &large[..large.partition_point(|&(edge, _)| edge < edges)],
                })
            }
            Self::Expanded(v) => Some(Self::Expanded(v.get(..edges)?)),
            Self::Compact(v, large) => Some(Self::Compact(
                v.get(..edges)?,
                &large[..large.partition_point(|&(edge, _)| edge < edges)],
            )),
        }
    }
}

impl<'a> From<&'a [Option<u64>]> for CsrOrdinals<'a> {
    fn from(values: &'a [Option<u64>]) -> Self {
        Self::Expanded(values)
    }
}

impl<'a, const N: usize> From<&'a [Option<u64>; N]> for CsrOrdinals<'a> {
    fn from(values: &'a [Option<u64>; N]) -> Self {
        Self::Expanded(values)
    }
}

impl<'a> From<&'a Vec<Option<u64>>> for CsrOrdinals<'a> {
    fn from(values: &'a Vec<Option<u64>>) -> Self {
        Self::Expanded(values)
    }
}

/// Dense or run-length encoded action IDs in independently indexed blocks.
/// Each completed block stores either 256 dense IDs or strictly increasing
/// inclusive end-position/ID pairs. The final unsealed block stays dense.
///
/// ```
/// use dive_runtime::capture::{CsrActionBlocks, CsrActions};
/// // The first 200 transitions use action 7, the remaining 56 are unnamed.
/// let actions = CsrActions::ByteBlocks(CsrActionBlocks {
///     edges: 256, directory: &[4], encoded: &[199, 7, 255, 255], tail: &[],
/// });
/// assert!(actions.prefix(201).is_some());
/// assert!(actions.prefix(257).is_none());
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CsrActionBlocks<'a, T> {
    /// Logical prefix length. A prefix may end inside a sealed block.
    pub edges: usize,
    /// Cumulative ends in the encoded array, one per sealed block.
    pub directory: &'a [usize],
    /// Sealed dense blocks and run pairs.
    pub encoded: &'a [T],
    /// Unsealed dense IDs.
    pub tail: &'a [T],
}

impl<T: Copy + Into<u32>> CsrActionBlocks<'_, T> {
    /// Logical physical edge count of each sealed block.
    pub const BLOCK: usize = 256;

    fn get(self, edge: usize) -> Option<u32> {
        if edge >= self.edges {
            return None;
        }
        let block = edge / Self::BLOCK;
        let pos = edge % Self::BLOCK;
        if block == self.directory.len() {
            return self.tail.get(pos).copied().map(Into::into);
        }
        let start = block
            .checked_sub(1)
            .map_or(0, |i| self.directory.get(i).copied().unwrap_or(0));
        let data = self.encoded.get(start..*self.directory.get(block)?)?;
        if data.len() == Self::BLOCK {
            return data.get(pos).copied().map(Into::into);
        }
        let position = u32::try_from(pos).ok()?;
        let pairs = data.as_chunks::<2>().0;
        let run = pairs.partition_point(|pair| pair[0].into() < position);
        pairs.get(run).map(|pair| pair[1].into())
    }

    fn prefix(self, edges: usize) -> Option<Self> {
        if edges > self.edges {
            return None;
        }
        let blocks = self.directory.len().min(edges.div_ceil(Self::BLOCK));
        let directory = self.directory.get(..blocks)?;
        let encoded = self.encoded.get(..directory.last().copied().unwrap_or(0))?;
        let tail = self
            .tail
            .get(..edges.saturating_sub(blocks.checked_mul(Self::BLOCK)?))?;
        Some(Self {
            edges,
            directory,
            encoded,
            tail,
        })
    }

    fn check(self, control: &EvaluationControl) -> Result<(), Verdict> {
        let malformed = || bug("invalid CSR action blocks");
        let sealed = self
            .directory
            .len()
            .checked_mul(Self::BLOCK)
            .ok_or_else(malformed)?;
        if self.directory.len() > self.edges.div_ceil(Self::BLOCK)
            || self.tail.len() > Self::BLOCK
            || self.tail.len() != self.edges.saturating_sub(sealed)
            || self.directory.last().copied().unwrap_or(0) != self.encoded.len()
        {
            return Err(malformed());
        }
        let mut start = 0;
        for (block, &end) in self.directory.iter().enumerate() {
            checkpoint(control, block)?;
            let data = self.encoded.get(start..end).ok_or_else(malformed)?;
            if data.len() != Self::BLOCK {
                if data.len() > Self::BLOCK || data.is_empty() || !data.len().is_multiple_of(2) {
                    return Err(malformed());
                }
                let mut previous = None;
                for pair in data.as_chunks::<2>().0 {
                    let position = pair[0].into();
                    if previous.is_some_and(|p| p >= position) {
                        return Err(malformed());
                    }
                    previous = Some(position);
                }
                if previous != Some(255) {
                    return Err(malformed());
                }
            }
            start = end;
        }
        Ok(())
    }
}

/// Storage for the action label of each transition occurrence.
#[derive(Clone, Copy, Debug)]
pub enum CsrActions<'a> {
    /// Full-width producer labels. Generated unnamed and stutter entries use `None`.
    Expanded(&'a [Option<u32>]),
    /// Dense action indices 0 through 254; 255 marks the absence of a label.
    Bytes(&'a [u8]),
    /// Dense action indices through 65534, with 65535 marking an unnamed entry.
    Words(&'a [u16]),
    /// Indexed blocks with byte-sized IDs and the same unnamed sentinel as `Bytes`.
    ByteBlocks(CsrActionBlocks<'a, u8>),
    /// Indexed blocks with two-byte IDs and the same unnamed sentinel as `Words`.
    WordBlocks(CsrActionBlocks<'a, u16>),
}

impl CsrActions<'_> {
    fn len(self) -> usize {
        match self {
            Self::Expanded(values) => values.len(),
            Self::Bytes(values) => values.len(),
            Self::Words(values) => values.len(),
            Self::ByteBlocks(b) => b.edges,
            Self::WordBlocks(b) => b.edges,
        }
    }

    // Structural checks bind every access to the shared physical edge range.
    fn at(self, edge: usize) -> Option<u32> {
        match self {
            Self::ByteBlocks(b) => b.get(edge).filter(|&id| id != u32::from(u8::MAX)),
            Self::WordBlocks(b) => b.get(edge).filter(|&id| id != u32::from(u16::MAX)),
            Self::Expanded(values) => values[edge],
            Self::Bytes(values) => (values[edge] != u8::MAX).then(|| u32::from(values[edge])),
            Self::Words(values) => (values[edge] != u16::MAX).then(|| u32::from(values[edge])),
        }
    }

    /// Keeps the first `edges` action entries for a checked graph prefix.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        match self {
            Self::Expanded(values) => Some(Self::Expanded(values.get(..edges)?)),
            Self::Bytes(values) => Some(Self::Bytes(values.get(..edges)?)),
            Self::Words(values) => Some(Self::Words(values.get(..edges)?)),
            Self::ByteBlocks(b) => Some(Self::ByteBlocks(b.prefix(edges)?)),
            Self::WordBlocks(b) => Some(Self::WordBlocks(b.prefix(edges)?)),
        }
    }
}

impl<'a> From<&'a [Option<u32>]> for CsrActions<'a> {
    fn from(values: &'a [Option<u32>]) -> Self {
        Self::Expanded(values)
    }
}

/// Original state rows for generated occurrences and implicit stutters.
#[derive(Clone, Copy, Debug)]
pub enum CsrEdgeRows<'a> {
    /// One exact original row ID per edge, in physical order.
    Expanded(&'a [u32]),
    /// A block uses its targets' representative rows unless it retains explicit IDs.
    /// Directory entries are cumulative counts of explicit blocks. Each increment
    /// adds one block to `rows`, truncated to the actual edge count in the last block.
    Blocks {
        /// Number of physical edges represented, including stutters.
        edges: usize,
        /// Cumulative explicit block count through each physical block.
        directory: &'a [usize],
        /// Original IDs for explicit blocks, concatenated in graph order.
        rows: &'a [u32],
    },
}

impl CsrEdgeRows<'_> {
    /// Number of physical edges in a block.
    pub const BLOCK: usize = 256;

    /// Reads a physical edge's original row from the producer's columns.
    /// Also usable during capture, before the current source has a synthetic stutter.
    #[must_use]
    pub fn row(self, edge: usize, targets: &[u32], representatives: &[u32]) -> Option<u32> {
        self.for_target(edge, *targets.get(edge)?, representatives)
    }

    fn for_target(self, edge: usize, target: u32, representatives: &[u32]) -> Option<u32> {
        let target = target as usize;
        match self {
            Self::Expanded(rows) => rows.get(edge).copied(),
            Self::Blocks {
                edges,
                directory,
                rows,
            } => {
                if edge >= edges {
                    return None;
                }
                let block = edge / Self::BLOCK;
                let before = block
                    .checked_sub(1)
                    .map_or(Some(0), |i| directory.get(i).copied())?;
                if *directory.get(block)? == before {
                    representatives.get(target).copied()
                } else {
                    rows.get(
                        before
                            .checked_mul(Self::BLOCK)?
                            .checked_add(edge % Self::BLOCK)?,
                    )
                    .copied()
                }
            }
        }
    }

    /// Retains exactly the rows used by the first `edges` physical entries.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        match self {
            Self::Expanded(rows) => Some(Self::Expanded(rows.get(..edges)?)),
            Self::Blocks {
                edges: full,
                directory,
                rows,
            } => {
                if edges > full {
                    return None;
                }
                let (directory, count) = block_prefix(edges, directory)?;
                Some(Self::Blocks {
                    edges,
                    directory,
                    rows: rows.get(..count)?,
                })
            }
        }
    }

    fn check(self, edges: usize, control: &EvaluationControl) -> Result<(), Verdict> {
        match self {
            Self::Expanded(rows) if rows.len() == edges => Ok(()),
            Self::Blocks {
                edges: full,
                directory,
                rows,
            } if full == edges => {
                check_blocks(edges, directory, rows.len(), "original row", control)
            }
            _ => Err(bug("inconsistent CSR array dimensions")),
        }
    }
}

fn block_prefix(edges: usize, directory: &[usize]) -> Option<(&[usize], usize)> {
    let directory = directory.get(..edges.div_ceil(CsrEdgeRows::BLOCK))?;
    let count = match directory.split_last() {
        None => 0,
        Some((&end, previous)) => {
            let before = previous.last().copied().unwrap_or(0);
            let width = (edges - 1) % CsrEdgeRows::BLOCK + 1;
            before
                .checked_mul(CsrEdgeRows::BLOCK)?
                .checked_add(if before == end { 0 } else { width })?
        }
    };
    Some((directory, count))
}

fn check_blocks(
    edges: usize,
    directory: &[usize],
    cells: usize,
    column: &str,
    control: &EvaluationControl,
) -> Result<(), Verdict> {
    if directory.len() != edges.div_ceil(CsrEdgeRows::BLOCK) {
        return Err(bug("inconsistent CSR array dimensions"));
    }
    let mut before = 0;
    let mut retained = 0;
    for (block, &end) in directory.iter().enumerate() {
        checkpoint(control, block)?;
        if end != before {
            if end != before + 1 {
                return Err(bug(&format!("invalid CSR {column} block directory")));
            }
            retained += CsrEdgeRows::BLOCK.min(edges - block * CsrEdgeRows::BLOCK);
        }
        before = end;
    }
    if retained == cells {
        Ok(())
    } else {
        Err(bug(&format!("invalid CSR {column} blocks")))
    }
}

impl<'a> From<&'a [u32]> for CsrEdgeRows<'a> {
    fn from(rows: &'a [u32]) -> Self {
        Self::Expanded(rows)
    }
}

/// Borrowed producer arrays with full-width optional action indices.
/// The fixed slice field accepts arrays, mutable slices and other slice-coercible buffers.
pub type CsrCaptureInput<'a, O = &'a [Option<u64>]> = CsrCaptureColumns<'a, O, &'a [Option<u32>]>;

/// Borrowed producer arrays with selectable ordinals and actions and a slice of original row IDs.
pub type CsrCaptureColumns<'a, O = CsrOrdinals<'a>, A = CsrActions<'a>> =
    CsrStorage<'a, O, A, &'a [u32]>;

/// Borrowed producer columns with a dense target slice. The fixed target field
/// accepts arrays and chained slice coercions, including a borrowed `Arc<Vec<u32>>`.
pub type CsrStorage<'a, O = CsrOrdinals<'a>, A = CsrActions<'a>, R = CsrEdgeRows<'a>> =
    GraphColumns<'a, O, A, R, &'a [u32]>;

/// Borrowed producer metadata. Row IDs identify opaque retained state rows.
#[derive(Clone, Copy, Debug)]
pub struct GraphColumns<'a, O, A, R, T> {
    /// One start offset per representative, followed by the total edge count.
    pub offsets: &'a [usize],
    /// Representative destination IDs, including implicit stutters.
    pub targets: T,
    /// Original generation ordinals; only implicit stutters have no ordinal.
    pub ordinals: O,
    /// Index in `action_labels`, or no named action. Stutters have no action.
    pub actions: A,
    /// Exact original state row for each edge, including seen destinations.
    pub edge_rows: R,
    /// First-discovery state row for every dense representative.
    pub representatives: &'a [u32],
    /// Distinct initial representatives, in discovery order.
    pub roots: &'a [usize],
    /// Run-local action labels, in the producer's existing order.
    pub action_labels: &'a [String],
    /// Number of retained opaque state rows.
    pub retained_rows: usize,
    /// Consecutive fully expanded sources. Remaining frontier rows are empty.
    pub closed_rows: usize,
}

/// Borrowed graph with checked-column views including a packed or dense target prefix.
pub type CsrTargetStorage<'a> =
    GraphColumns<'a, CsrOrdinals<'a>, CsrActions<'a>, CsrEdgeRows<'a>, CsrTargets<'a>>;

impl<
    'a,
    O: Into<CsrOrdinals<'a>>,
    A: Into<CsrActions<'a>>,
    R: Into<CsrEdgeRows<'a>>,
    T: Into<CsrTargets<'a>>,
> GraphColumns<'a, O, A, R, T>
{
    fn columns(self) -> CsrTargetStorage<'a> {
        CsrTargetStorage {
            offsets: self.offsets,
            targets: self.targets.into(),
            ordinals: self.ordinals.into(),
            actions: self.actions.into(),
            edge_rows: self.edge_rows.into(),
            representatives: self.representatives,
            roots: self.roots,
            action_labels: self.action_labels,
            retained_rows: self.retained_rows,
            closed_rows: self.closed_rows,
        }
    }
}

impl<'a, T: Copy + Into<CsrTargets<'a>>>
    GraphColumns<'a, CsrOrdinals<'a>, CsrActions<'a>, CsrEdgeRows<'a>, T>
{
    /// Looks up the retained original row for a physical edge, including stutters.
    #[must_use]
    pub fn edge_row(&self, edge: usize) -> Option<u32> {
        self.edge_rows
            .for_target(edge, self.targets.into().get(edge)?, self.representatives)
    }
}

/// Immutable, structurally checked CSR capture, without completion authority.
///
/// Generated duplicates, ordinal gaps, action labels and original row identities
/// remain intact. A successful shape check alone cannot establish exhaustion.
#[derive(Clone, Copy, Debug)]
pub struct CsrCapture<'a> {
    input: CsrTargetStorage<'a>,
    limits: Limits,
    concrete: usize,
}

impl<'a> CsrCapture<'a> {
    /// Checks structural invariants under the actual armed run control.
    /// All closed sources end in exactly one implicit full-state stutter.
    ///
    /// # Errors
    /// Malformed metadata, retained-entry limits, or an actual run interruption.
    pub fn checked(
        input: CsrStorage<
            'a,
            impl Into<CsrOrdinals<'a>>,
            impl Into<CsrActions<'a>>,
            impl Into<CsrEdgeRows<'a>>,
        >,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        Self::checked_targets(input.columns(), limits, control)
    }

    /// Checks a graph with a bounded-read target column under the same limits as [`Self::checked`].
    ///
    /// # Errors
    /// The same malformed input, limits and interruption as [`Self::checked`].
    pub fn checked_targets(
        input: CsrTargetStorage<'a>,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        poll(control)?;
        let states = input.representatives.len();
        let edges = input.targets.len();
        if states > limits.states || edges > limits.edges {
            return Err(limit("CSR entries"));
        }
        if input.offsets.len() != states.checked_add(1).ok_or_else(|| limit("CSR offsets"))?
            || input.offsets.first() != Some(&0)
            || input.offsets.last() != Some(&edges)
            || input.ordinals.len() != edges
            || input.actions.len() != edges
            || input.closed_rows > states
            || (states != 0 && input.roots.is_empty())
        {
            return Err(bug("inconsistent CSR array dimensions"));
        }
        input.edge_rows.check(edges, control)?;
        match input.actions {
            CsrActions::ByteBlocks(blocks) => blocks.check(control)?,
            CsrActions::WordBlocks(blocks) => blocks.check(control)?,
            _ => {}
        }
        input
            .ordinals
            .check(edges, input.offsets, input.closed_rows, control)?;
        let mut previous_root = None;
        for (i, &root) in input.roots.iter().enumerate() {
            checkpoint(control, i)?;
            if root >= states || previous_root.is_some_and(|p| p >= root) {
                return Err(bug("invalid CSR initial roots"));
            }
            previous_root = Some(root);
        }
        let mut concrete = 0usize;
        for (source, &representative) in input.representatives.iter().enumerate() {
            checkpoint(control, source)?;
            if representative as usize >= input.retained_rows {
                return Err(bug("CSR representative row out of range"));
            }
            let start = input.offsets[source];
            let end = input.offsets[source + 1];
            if start > end || end > edges {
                return Err(bug("invalid CSR source offsets"));
            }
            if source >= input.closed_rows {
                if start != end {
                    return Err(bug("unexpanded CSR frontier has outgoing edges"));
                }
                continue;
            }
            let Some(last) = end.checked_sub(1).filter(|&last| last >= start) else {
                return Err(bug("missing CSR source/stutter row"));
            };
            if input.targets.get(last).map(|n| n as usize) != Some(source)
                || input.ordinals.get(last, None) != Some(None)
                || input.actions.at(last).is_some()
                || input.edge_row(last) != Some(representative)
            {
                return Err(bug("invalid CSR implicit stutter"));
            }
            let mut previous_ordinal = None;
            for edge in start..last {
                checkpoint(control, edge)?;
                let Some(Some(ordinal)) = input.ordinals.get(edge, Some((edge - start) as u64))
                else {
                    return Err(bug("CSR generated edge lacks original ordinal"));
                };
                if ordinal >= limits.successors_per_unit {
                    return Err(limit("CSR source successors"));
                }
                if input.targets.get(edge).is_none_or(|n| n as usize >= states)
                    || input
                        .edge_row(edge)
                        .is_none_or(|row| row as usize >= input.retained_rows)
                    || input
                        .actions
                        .at(edge)
                        .is_some_and(|action| action as usize >= input.action_labels.len())
                    || previous_ordinal.is_some_and(|previous| previous >= ordinal)
                {
                    return Err(bug("invalid CSR original transition"));
                }
                previous_ordinal = Some(ordinal);
            }
            concrete = concrete
                .checked_add(last - start)
                .ok_or_else(|| limit("CSR concrete successors"))?;
        }
        poll(control)?;
        Ok(Self {
            input,
            limits,
            concrete,
        })
    }

    /// Number of representative vertices, including the unexpanded frontier.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.input.representatives.len()
    }

    /// Actual entry limits used for this shape check; these are not byte bounds.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Checked adjacency offsets, including the terminal offset.
    #[must_use]
    pub fn offsets(&self) -> &'a [usize] {
        self.input.offsets
    }

    /// Checked dense edge destinations, including implicit stutters. For a packed
    /// producer this explicit slice request materializes and caches the full
    /// column. Use [`Self::target_column`] for bounded reads without a copy.
    #[must_use]
    pub fn targets(&self) -> &'a [u32] {
        self.input.targets.as_slice()
    }

    /// Borrows physical target IDs without materializing a dense array.
    #[must_use]
    pub const fn target_column(&self) -> CsrTargets<'a> {
        self.input.targets
    }

    /// Initial representatives in discovery order.
    #[must_use]
    pub fn roots(&self) -> &'a [usize] {
        self.input.roots
    }

    /// The checked upper bound on all representative and original input row IDs.
    #[must_use]
    pub const fn retained_rows(&self) -> usize {
        self.input.retained_rows
    }

    /// Fully expanded source count at this committed boundary.
    #[must_use]
    pub const fn closed_rows(&self) -> usize {
        self.input.closed_rows
    }

    /// Physical adjacency range for a source, including its implicit stutter.
    #[must_use]
    pub fn outgoing_range(&self, source: usize) -> Option<Range<usize>> {
        Some(*self.input.offsets.get(source)?..*self.input.offsets.get(source.checked_add(1)?)?)
    }

    /// Destination of a physical adjacency entry.
    #[must_use]
    pub fn target(&self, edge: usize) -> Option<usize> {
        self.input.targets.get(edge).map(|target| target as usize)
    }

    /// Generation ordinal of a physical entry; stutters have `Some(None)`.
    #[must_use]
    pub fn ordinal(&self, edge: usize) -> Option<Option<u64>> {
        if let CsrOrdinals::PositionBlocks { sources, .. } = self.input.ordinals {
            // Every closed source has a stutter, so at most 256 sources
            // contribute to a 256-edge block. Known-source consumers call
            // metadata directly and do not search offsets.
            let first = *sources.get(edge / CsrEdgeRows::BLOCK)? as usize;
            let last = first
                .saturating_add(CsrEdgeRows::BLOCK)
                .min(self.input.closed_rows);
            let source = first
                + self
                    .input
                    .offsets
                    .get(first..last)?
                    .partition_point(|&start| start <= edge)
                    .checked_sub(1)?;
            let range = self.outgoing_range(source)?;
            if !range.contains(&edge) {
                return None;
            }
            self.input.ordinals.get(
                edge,
                (edge + 1 < range.end).then_some((edge - range.start) as u64),
            )
        } else {
            self.input.ordinals.get(edge, None)
        }
    }

    /// Action label; generated unnamed actions use the empty label.
    /// Stutters have `Some(None)`, and out-of-range entries have `None`.
    #[must_use]
    pub fn label(&self, edge: usize) -> Option<Option<&'a str>> {
        let ordinal = self.ordinal(edge)?;
        self.edge_label(edge, ordinal)
    }

    #[allow(
        clippy::option_option,
        reason = "an invalid edge or label differs from a valid unlabelled stutter"
    )]
    fn edge_label(&self, edge: usize, ordinal: Option<u64>) -> Option<Option<&'a str>> {
        if ordinal.is_none() {
            return Some(None);
        }
        Some(Some(match self.input.actions.at(edge) {
            Some(action) => self.input.action_labels.get(action as usize)?.as_str(),
            None => "",
        }))
    }

    /// Returns the original ordinal and action label at a source-local
    /// physical position. The implicit stutter has neither.
    #[must_use]
    pub fn metadata(
        &self,
        source: usize,
        position: usize,
    ) -> Option<(Option<u64>, Option<&'a str>)> {
        let range = self.outgoing_range(source)?;
        let edge = range.start.checked_add(position)?;
        if edge >= range.end {
            return None;
        }
        let ordinal = self
            .input
            .ordinals
            .get(edge, (edge + 1 < range.end).then_some(position as u64))?;
        Some((ordinal, self.edge_label(edge, ordinal)?))
    }

    /// Opaque first-discovery row for a representative vertex.
    #[must_use]
    pub fn representative_row(&self, vertex: usize) -> Option<usize> {
        self.input
            .representatives
            .get(vertex)
            .map(|&row| row as usize)
    }

    /// Zero-based producer action for a physical original in a closed source.
    /// Unnamed originals, frontier positions and the implicit stutter have none.
    #[must_use]
    pub fn original_action(&self, source: usize, position: usize) -> Option<u32> {
        let range = self.outgoing_range(source)?;
        if position >= range.len().saturating_sub(1) {
            return None;
        }
        self.input.actions.at(range.start + position)
    }

    /// Generated original row at a source's physical generated-edge position.
    /// Positions exclude the final stutter and do not use generation ordinals.
    #[must_use]
    pub fn original_row(&self, source: usize, position: usize) -> Option<usize> {
        let range = self.outgoing_range(source)?;
        let edge = range.start.checked_add(position)?;
        if position >= range.len().saturating_sub(1) {
            return None;
        }
        self.input.edge_row(edge).map(|row| row as usize)
    }

    /// Generated originals in one source row, excluding the implicit stutter.
    #[must_use]
    pub fn original_count(&self, source: usize) -> Option<usize> {
        let range = self.outgoing_range(source)?;
        Some(range.len().saturating_sub(1))
    }
}

/// Checked CSR and sealed facts from a successful, exhausted producer.
#[derive(Clone, Debug)]
pub struct CsrComplete<'a> {
    graph: CsrCapture<'a>,
    evidence: CompleteEvidence,
}

impl<'a> CsrComplete<'a> {
    /// Finalizes successful producer EOF without constructing retained values.
    ///
    /// # Errors
    /// The original non-success safety verdict, missing/inconsistent statistics,
    /// malformed or incomplete CSR, a capture limit, or actual run interruption.
    pub fn from_producer(
        input: CsrStorage<
            'a,
            impl Into<CsrOrdinals<'a>>,
            impl Into<CsrActions<'a>>,
            impl Into<CsrEdgeRows<'a>>,
        >,
        safety: CheckResult,
        options: CheckOptions,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        Self::from_target_producer(input.columns(), safety, options, limits, control)
    }

    /// Finalizes producer EOF with a bounded-read target column.
    ///
    /// # Errors
    /// The same malformed input, limits and interruption as [`Self::from_producer`].
    pub fn from_target_producer(
        input: CsrTargetStorage<'a>,
        safety: CheckResult,
        options: CheckOptions,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        if safety.verdict != Verdict::Success {
            return Err(safety.verdict);
        }
        let stats = safety
            .stats
            .ok_or_else(|| bug("CSR EOF did not return statistics"))?;
        GraphCompletion::check_header(
            input.representatives.len(),
            input.roots,
            stats,
            limits,
            Some(control),
        )?;
        let graph = CsrCapture::checked_targets(input, limits, control)?;
        if graph.closed_rows() != graph.state_count() {
            return Err(bug("CSR EOF has unexpanded sources"));
        }
        let completion = GraphCompletion {
            states: graph.state_count(),
            edges: graph.target_column().len(),
            concrete_successors: graph.concrete,
            roots: graph.roots().len(),
            limits,
        };
        Ok(Self {
            graph,
            evidence: CompleteEvidence::new(safety, options, stats, completion),
        })
    }

    /// Checked adjacency; state values stay with the original producer.
    #[must_use]
    pub const fn graph(&self) -> &CsrCapture<'a> {
        &self.graph
    }

    /// Original successful producer facts, independent of adjacency storage.
    #[must_use]
    pub const fn evidence(&self) -> &CompleteEvidence {
        &self.evidence
    }

    /// Detaches completion facts, releasing the borrowed adjacency.
    #[must_use]
    pub fn into_evidence(self) -> CompleteEvidence {
        self.evidence
    }
}

/// Checked committed CSR prefix with an explicitly unfinished frontier.
#[derive(Clone, Debug)]
pub struct CsrPrefix<'a> {
    graph: CsrCapture<'a>,
    evidence: PrefixEvidence,
}

impl<'a> CsrPrefix<'a> {
    /// Finalizes a successful committed prefix. This never proves completion.
    ///
    /// # Errors
    /// The original safety failure, missing/inconsistent statistics, exhausted
    /// frontier, malformed CSR, a capture limit, or actual run interruption.
    pub fn from_producer(
        input: CsrStorage<
            'a,
            impl Into<CsrOrdinals<'a>>,
            impl Into<CsrActions<'a>>,
            impl Into<CsrEdgeRows<'a>>,
        >,
        level: u64,
        safety: CheckResult,
        options: CheckOptions,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        Self::from_target_producer(input.columns(), level, safety, options, limits, control)
    }

    /// Finalizes a committed prefix with a bounded-read target column.
    ///
    /// # Errors
    /// The same malformed input, limits and interruption as [`Self::from_producer`].
    pub fn from_target_producer(
        input: CsrTargetStorage<'a>,
        level: u64,
        safety: CheckResult,
        options: CheckOptions,
        limits: Limits,
        control: &EvaluationControl,
    ) -> Result<Self, Verdict> {
        if safety.verdict != Verdict::Success {
            return Err(safety.verdict);
        }
        let stats = safety
            .stats
            .ok_or_else(|| bug("CSR prefix did not return statistics"))?;
        if stats.generated > limits.generated {
            return Err(limit("CSR prefix generated states"));
        }
        if stats.left_on_queue == 0
            || stats.distinct
                != u64::try_from(input.representatives.len())
                    .map_err(|_| bug("state count range"))?
            || input.closed_rows >= input.representatives.len()
            || stats.left_on_queue
                != u64::try_from(
                    input
                        .representatives
                        .len()
                        .saturating_sub(input.closed_rows),
                )
                .map_err(|_| bug("CSR frontier count range"))?
        {
            return Err(bug("inconsistent CSR prefix frontier/statistics"));
        }
        let graph = CsrCapture::checked_targets(input, limits, control)?;
        Ok(Self {
            graph,
            evidence: PrefixEvidence::new(safety, options, stats, level),
        })
    }

    /// Checked adjacency, including empty outgoing rows for the frontier.
    #[must_use]
    pub const fn graph(&self) -> &CsrCapture<'a> {
        &self.graph
    }

    /// Original committed-prefix facts; these cannot be promoted to completion.
    #[must_use]
    pub const fn evidence(&self) -> &PrefixEvidence {
        &self.evidence
    }

    /// Detaches prefix facts, releasing the borrowed adjacency.
    #[must_use]
    pub fn into_evidence(self) -> PrefixEvidence {
        self.evidence
    }
}

#[doc(hidden)]
pub fn checkpoint(control: &EvaluationControl, position: usize) -> Result<(), Verdict> {
    if position.is_multiple_of(256) {
        poll(control)?;
    }
    Ok(())
}

#[doc(hidden)]
pub fn poll(control: &EvaluationControl) -> Result<(), Verdict> {
    control.check().map_err(|interrupted| {
        control
            .verdict(&dive_eval::EvalError::interrupted(interrupted))
            .unwrap_or_else(|| bug("CSR control interrupted without matching exhaustion"))
    })
}
