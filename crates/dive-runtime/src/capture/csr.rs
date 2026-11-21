//! Checked borrowed adjacency for a producer retaining its own state rows.

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
}

impl CsrOrdinals<'_> {
    /// First ordinal stored in the full-width side table.
    pub const EXTENDED: u16 = u16::MAX - 1;

    fn len(self) -> usize {
        match self {
            Self::Expanded(v) => v.len(),
            Self::Compact(v, _) => v.len(),
        }
    }

    #[allow(
        clippy::option_option,
        reason = "distinguishes missing entries, stutters and generated transitions"
    )]
    fn get(self, index: usize) -> Option<Option<u64>> {
        match self {
            Self::Expanded(v) => v.get(index).copied(),
            Self::Compact(v, large) => match *v.get(index)? {
                u16::MAX => Some(None),
                Self::EXTENDED => large
                    .binary_search_by_key(&index, |&(edge, _)| edge)
                    .ok()
                    .map(|i| Some(large[i].1)),
                n => Some(Some(u64::from(n))),
            },
        }
    }

    /// Keeps the first `edges` entries without expanding their representation.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        match self {
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

/// Storage for the action label of each transition occurrence.
#[derive(Clone, Copy, Debug)]
pub enum CsrActions<'a> {
    /// Full-width producer labels. Generated unnamed and stutter entries use `None`.
    Expanded(&'a [Option<u32>]),
    /// Dense action indices 0 through 254; 255 marks the absence of a label.
    Bytes(&'a [u8]),
}

impl CsrActions<'_> {
    fn len(self) -> usize {
        match self {
            Self::Expanded(values) => values.len(),
            Self::Bytes(values) => values.len(),
        }
    }

    // Structural checks bind every access to the shared physical edge range.
    fn at(self, edge: usize) -> Option<u32> {
        match self {
            Self::Expanded(values) => values[edge],
            Self::Bytes(values) => (values[edge] != u8::MAX).then(|| u32::from(values[edge])),
        }
    }

    /// Keeps the first `edges` action entries for a checked graph prefix.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        match self {
            Self::Expanded(values) => Some(Self::Expanded(values.get(..edges)?)),
            Self::Bytes(values) => Some(Self::Bytes(values.get(..edges)?)),
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
        let target = *targets.get(edge)? as usize;
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
                let directory = directory.get(..edges.div_ceil(Self::BLOCK))?;
                let count = match directory.split_last() {
                    None => 0,
                    Some((&end, previous)) => {
                        let before = previous.last().copied().unwrap_or(0);
                        let width = (edges - 1) % Self::BLOCK + 1;
                        before
                            .checked_mul(Self::BLOCK)?
                            .checked_add(if before == end { 0 } else { width })?
                    }
                };
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
            } if full == edges && directory.len() == edges.div_ceil(Self::BLOCK) => {
                let mut previous = 0usize;
                let mut retained = 0usize;
                for (block, &end) in directory.iter().enumerate() {
                    checkpoint(control, block)?;
                    if end != previous {
                        if end != previous + 1 {
                            return Err(bug("invalid CSR original row block directory"));
                        }
                        retained += Self::BLOCK.min(edges - block * Self::BLOCK);
                    }
                    previous = end;
                }
                if retained == rows.len() {
                    Ok(())
                } else {
                    Err(bug("invalid CSR original row blocks"))
                }
            }
            _ => Err(bug("inconsistent CSR array dimensions")),
        }
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

/// Borrowed producer arrays with selectable ordinal, action and original row storage.
///
/// Row ids identify opaque retained state rows, not semantically equal values.
/// This metadata neither evaluates states nor proves a temporal property.
#[derive(Clone, Copy, Debug)]
pub struct CsrStorage<'a, O = CsrOrdinals<'a>, A = CsrActions<'a>, R = CsrEdgeRows<'a>> {
    /// One start offset per representative, followed by the total edge count.
    pub offsets: &'a [usize],
    /// Dense representative destination ids, including implicit stutters.
    pub targets: &'a [u32],
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

impl<'a, O: Into<CsrOrdinals<'a>>, A: Into<CsrActions<'a>>, R: Into<CsrEdgeRows<'a>>>
    CsrStorage<'a, O, A, R>
{
    fn columns(self) -> CsrStorage<'a> {
        CsrStorage {
            offsets: self.offsets,
            targets: self.targets,
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

impl CsrStorage<'_> {
    /// Looks up the retained original row for a physical edge, including stutters.
    #[must_use]
    pub fn edge_row(&self, edge: usize) -> Option<u32> {
        self.edge_rows.row(edge, self.targets, self.representatives)
    }
}

/// Immutable, structurally checked CSR capture, without completion authority.
///
/// Generated duplicates, ordinal gaps, action labels and original row identities
/// remain intact. A successful shape check alone cannot establish exhaustion.
#[derive(Clone, Copy, Debug)]
pub struct CsrCapture<'a> {
    input: CsrStorage<'a>,
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
        let input = input.columns();
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
        if let CsrOrdinals::Compact(words, large) = input.ordinals {
            let mut previous = None;
            for (i, &(edge, ordinal)) in large.iter().enumerate() {
                checkpoint(control, i)?;
                if previous.is_some_and(|p| p >= edge)
                    || words.get(edge) != Some(&CsrOrdinals::EXTENDED)
                    || ordinal < u64::from(CsrOrdinals::EXTENDED)
                {
                    return Err(bug("invalid extended CSR ordinal"));
                }
                previous = Some(edge);
            }
        }
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
            if input.targets[last] as usize != source
                || input.ordinals.get(last) != Some(None)
                || input.actions.at(last).is_some()
                || input.edge_row(last) != Some(representative)
            {
                return Err(bug("invalid CSR implicit stutter"));
            }
            let mut previous_ordinal = None;
            for edge in start..last {
                checkpoint(control, edge)?;
                let Some(Some(ordinal)) = input.ordinals.get(edge) else {
                    return Err(bug("CSR generated edge lacks original ordinal"));
                };
                if ordinal >= limits.successors_per_unit {
                    return Err(limit("CSR source successors"));
                }
                if input.targets[edge] as usize >= states
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

    /// Checked dense edge destinations, including implicit stutters.
    #[must_use]
    pub fn targets(&self) -> &'a [u32] {
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
        self.input.targets.get(edge).map(|&target| target as usize)
    }

    /// Generation ordinal of a physical entry; stutters have `Some(None)`.
    #[must_use]
    pub fn ordinal(&self, edge: usize) -> Option<Option<u64>> {
        self.input.ordinals.get(edge)
    }

    /// Action label; generated unnamed actions use the empty label.
    /// Stutters have `Some(None)`, and out-of-range entries have `None`.
    #[must_use]
    pub fn label(&self, edge: usize) -> Option<Option<&'a str>> {
        let ordinal = self.input.ordinals.get(edge)?;
        if ordinal.is_none() {
            return Some(None);
        }
        Some(Some(match self.input.actions.at(edge) {
            Some(action) => self.input.action_labels.get(action as usize)?.as_str(),
            None => "",
        }))
    }

    /// Opaque first-discovery row for a representative vertex.
    #[must_use]
    pub fn representative_row(&self, vertex: usize) -> Option<usize> {
        self.input
            .representatives
            .get(vertex)
            .map(|&row| row as usize)
    }

    /// Generated original row at a source's physical generated-edge position.
    /// Positions exclude the final stutter and do not use generation ordinals.
    #[must_use]
    pub fn original_row(&self, source: usize, position: usize) -> Option<usize> {
        let range = self.outgoing_range(source)?;
        let edge = range.start.checked_add(position)?;
        if edge >= range.end || self.input.ordinals.get(edge)?.is_none() {
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
        let input = input.columns();
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
        let graph = CsrCapture::checked(input, limits, control)?;
        if graph.closed_rows() != graph.state_count() {
            return Err(bug("CSR EOF has unexpanded sources"));
        }
        let completion = GraphCompletion {
            states: graph.state_count(),
            edges: graph.targets().len(),
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
        let input = input.columns();
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
        let graph = CsrCapture::checked(input, limits, control)?;
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
