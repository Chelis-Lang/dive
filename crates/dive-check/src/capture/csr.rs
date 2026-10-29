//! Checked borrowed adjacency for a producer retaining its own state rows.

use std::ops::Range;

use dive_eval::HostControl;

use super::{CompleteEvidence, GraphCompletion, Limits, PrefixEvidence, bug, limit};
use crate::budget::EvaluationControl;
use crate::{CheckOptions, CheckResult, Verdict};

/// Borrowed producer arrays before structural checking.
///
/// Row ids identify opaque retained state rows, not semantically equal values.
/// This metadata neither evaluates states nor proves a temporal property.
#[derive(Clone, Copy, Debug)]
pub struct CsrCaptureInput<'a> {
    /// One start offset per representative, followed by the total edge count.
    pub offsets: &'a [usize],
    /// Dense representative destination ids, including implicit stutters.
    pub targets: &'a [u32],
    /// Original generation ordinals; only implicit stutters have no ordinal.
    pub ordinals: &'a [Option<u64>],
    /// Index in `action_labels`, or no named action. Stutters have no action.
    pub actions: &'a [Option<u32>],
    /// Exact original state row for each edge, including seen destinations.
    pub edge_rows: &'a [u32],
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

/// Immutable, structurally checked CSR capture, without completion authority.
///
/// Generated duplicates, ordinal gaps, action labels and original row identities
/// remain intact. A successful shape check alone cannot establish exhaustion.
#[derive(Clone, Copy, Debug)]
pub struct CsrCapture<'a> {
    input: CsrCaptureInput<'a>,
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
        input: CsrCaptureInput<'a>,
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
            || input.edge_rows.len() != edges
            || input.closed_rows > states
            || (states != 0 && input.roots.is_empty())
        {
            return Err(bug("inconsistent CSR array dimensions"));
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
                || input.ordinals[last].is_some()
                || input.actions[last].is_some()
                || input.edge_rows[last] != representative
            {
                return Err(bug("invalid CSR implicit stutter"));
            }
            let mut previous_ordinal = None;
            for edge in start..last {
                checkpoint(control, edge)?;
                let Some(ordinal) = input.ordinals[edge] else {
                    return Err(bug("CSR generated edge lacks original ordinal"));
                };
                if ordinal >= limits.successors_per_unit {
                    return Err(limit("CSR source successors"));
                }
                if input.targets[edge] as usize >= states
                    || input.edge_rows[edge] as usize >= input.retained_rows
                    || input.actions[edge]
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
        self.input.ordinals.get(edge).copied()
    }

    /// Action label; generated unnamed actions use the legacy empty label.
    /// Stutters have `Some(None)`, and out-of-range entries have `None`.
    #[must_use]
    pub fn label(&self, edge: usize) -> Option<Option<&'a str>> {
        let ordinal = self.input.ordinals.get(edge)?;
        if ordinal.is_none() {
            return Some(None);
        }
        Some(Some(match self.input.actions.get(edge)? {
            Some(action) => self.input.action_labels.get(*action as usize)?.as_str(),
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
        self.input.edge_rows.get(edge).map(|&row| row as usize)
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
        input: CsrCaptureInput<'a>,
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
        input: CsrCaptureInput<'a>,
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

pub(super) fn checkpoint(control: &EvaluationControl, position: usize) -> Result<(), Verdict> {
    if position.is_multiple_of(256) {
        poll(control)?;
    }
    Ok(())
}

pub(super) fn poll(control: &EvaluationControl) -> Result<(), Verdict> {
    control.check().map_err(|interrupted| {
        control
            .verdict(&dive_eval::EvalError::interrupted(interrupted))
            .unwrap_or_else(|| bug("CSR control interrupted without matching exhaustion"))
    })
}
