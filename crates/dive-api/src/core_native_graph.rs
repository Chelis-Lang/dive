//! Native original-value retention and ordered CSR capture.
//!
//! No reference value graph is retained. The converter admits every original
//! before graph admission, including seen occurrences, in event order, unless
//! the run deferred admission to reads (see `core_native_values`).

use std::collections::HashMap;
use std::sync::Arc;

use dive_core::{Handle, Heap, Store};
use dive_eval::HostControl;
use dive_explore::graph::{GraphSink, SourceRow, Step, Stopped};
use dive_liveness::analyze::AnalysisError;
use dive_runtime::budget::{EvaluationControl, Phase};
use dive_runtime::capture::{
    CsrActionBlocks, CsrActions, CsrEdgeRows, CsrOrdinals, CsrTargetStorage, CsrTargets, Limits,
    MAX_EXPANSION_UNITS, PackedTargets,
};
use dive_runtime::{CheckOptions, Stats, Verdict};

use crate::core_native_values::NativeValues;

pub(crate) const SOURCE_ROW_STOP: &str =
    "native source-row execution stopped at a retained temporal error";

pub(crate) struct SourceRowSetup {
    pub(crate) ir: Arc<dive_ir::Ir>,
    pub(crate) config: dive_config::ModelConfig,
    pub(crate) options: CheckOptions,
    pub(crate) limits: dive_liveness::analyze::Limits,
    pub(crate) graph_labels: dive_liveness::analyze::GraphLabelPolicy,
}

impl SourceRowSetup {
    fn bind(self) -> Result<Option<dive_liveness::analyze::owned::SourceRowDriver>, Verdict> {
        use dive_liveness::analyze::owned::{self, Dispatch};
        let control = EvaluationControl::new(&self.options.budget, Phase::Binding)
            .map_err(|error| bug(&error.to_string()))?;
        let Some(bound) =
            owned::dispatch_controlled(&self.ir, &self.config, &self.options, control.clone())
        else {
            return Ok(None);
        };
        match bound {
            Ok(Dispatch::Temporal(runtime)) => runtime
                .source_row_driver(self.limits, self.graph_labels)
                .map_err(source_error),
            Ok(Dispatch::Safety) => Ok(None),
            Err(dive_model::ControlledBindError::Interrupted { error, .. }) => Err(control
                .verdict(&error)
                .unwrap_or_else(|| bug("native source-row foreign binding interruption"))),
            Err(dive_model::ControlledBindError::EvaluationFailure { error, .. }) => {
                Err(if error.class() == dive_eval::ErrorClass::ResourceLimit {
                    Verdict::ResourceLimit {
                        message: error.to_string(),
                    }
                } else {
                    bug(&error.to_string())
                })
            }
            Err(error) => Err(bug(&format!(
                "native source-row source binding failed: {error}"
            ))),
        }
    }
}

pub(crate) fn source_error(error: AnalysisError) -> Verdict {
    match error {
        AnalysisError::Stopped(stopped) => Verdict::Stopped(stopped),
        AnalysisError::BudgetMeasurement(error) => Verdict::BudgetMeasurement { error },
        AnalysisError::Limit(bound) => Verdict::ResourceLimit {
            message: bound.into(),
        },
        AnalysisError::Unsupported(_) | AnalysisError::UnsupportedValue(_) => {
            Verdict::Unsupported {
                feature: error.to_string(),
            }
        }
        _ => bug(&error.to_string()),
    }
}

pub(crate) type OnLevel<'model, 'callback> = dyn FnMut(u64, &NativeGraph, &mut NativeValues<'model>, Stats) -> Result<bool, Verdict>
    + 'callback;

/// Capture-local buffer accounting; opaque hash allocation remains separate.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NativeMemory {
    pub(crate) fixed_bytes: usize,
    pub(crate) buffer_bytes: usize,
    pub(crate) growth_overlap_upper_bound_bytes: usize,
    pub(crate) key_entries: usize,
    pub(crate) key_capacity: usize,
}

impl NativeMemory {
    fn growth(&mut self, before: usize, after: usize) -> Result<(), Verdict> {
        if after != before {
            self.buffer_bytes = self
                .buffer_bytes
                .checked_add(after - before)
                .ok_or_else(|| limit("native retained buffer bytes"))?;
            // A realloc may temporarily retain its old buffer beside its new one.
            self.growth_overlap_upper_bound_bytes = self
                .growth_overlap_upper_bound_bytes
                .max(self.buffer_bytes.saturating_add(before));
        }
        Ok(())
    }
}

#[derive(Default)]
struct ActionBlocks<T> {
    directory: Vec<usize>,
    encoded: Vec<T>,
    tail: Vec<T>,
}

impl<T: Copy + Eq + From<u8> + Into<u32>> ActionBlocks<T> {
    fn view(&self) -> CsrActionBlocks<'_, T> {
        CsrActionBlocks {
            edges: self.directory.len() * CsrActionBlocks::<T>::BLOCK + self.tail.len(),
            directory: &self.directory,
            encoded: &self.encoded,
            tail: &self.tail,
        }
    }

    /// Seals the previous block before accepting another occurrence. A dense
    /// block costs exactly one ID per edge plus its single directory entry.
    fn reserve(
        &mut self,
        memory: &mut NativeMemory,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        if self.tail.len() == CsrActionBlocks::<T>::BLOCK {
            let runs = 1 + self.tail.windows(2).filter(|p| p[0] != p[1]).count();
            let cells = self.tail.len().min(runs * 2);
            reserve(&mut self.directory, 1, memory, control)?;
            reserve(&mut self.encoded, cells, memory, control)?;
            if cells == self.tail.len() {
                self.encoded.extend_from_slice(&self.tail);
            } else {
                for (position, &id) in self.tail.iter().enumerate() {
                    if self.tail.get(position + 1) != Some(&id) {
                        let end = u8::try_from(position)
                            .map_err(|_| bug("native action block position"))?;
                        self.encoded.extend_from_slice(&[T::from(end), id]);
                    }
                }
            }
            self.directory.push(self.encoded.len());
            self.tail.clear();
        }
        reserve(&mut self.tail, 1, memory, control)
    }
}

enum ActionIds {
    Bytes(ActionBlocks<u8>),
    Words(ActionBlocks<u16>),
    Expanded(Vec<Option<u32>>),
}

impl ActionIds {
    fn for_labels(count: usize) -> Self {
        if u8::try_from(count).is_ok() {
            Self::Bytes(ActionBlocks::default())
        } else if u16::try_from(count).is_ok() {
            Self::Words(ActionBlocks::default())
        } else {
            Self::Expanded(Vec::new())
        }
    }

    fn view(&self) -> CsrActions<'_> {
        match self {
            Self::Bytes(ids) if ids.directory.is_empty() => CsrActions::Bytes(&ids.tail),
            Self::Bytes(ids) => CsrActions::ByteBlocks(ids.view()),
            Self::Words(ids) if ids.directory.is_empty() => CsrActions::Words(&ids.tail),
            Self::Words(ids) => CsrActions::WordBlocks(ids.view()),
            Self::Expanded(ids) => CsrActions::Expanded(ids),
        }
    }

    fn reserve(
        &mut self,
        memory: &mut NativeMemory,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        match self {
            Self::Bytes(ids) => ids.reserve(memory, control),
            Self::Words(ids) => ids.reserve(memory, control),
            Self::Expanded(ids) => reserve(ids, 1, memory, control),
        }
    }

    fn push(&mut self, action: Option<u32>) -> Result<(), Verdict> {
        match self {
            Self::Bytes(ids) => {
                let id = match action {
                    Some(id) => u8::try_from(id)
                        .ok()
                        .filter(|&n| n != u8::MAX)
                        .ok_or_else(|| bug("native action index exceeds compact label range"))?,
                    None => u8::MAX,
                };
                ids.tail.push(id);
            }
            Self::Words(ids) => {
                let id = match action {
                    Some(id) => u16::try_from(id)
                        .ok()
                        .filter(|&n| n != u16::MAX)
                        .ok_or_else(|| bug("native action index exceeds compact label range"))?,
                    None => u16::MAX,
                };
                ids.tail.push(id);
            }
            Self::Expanded(ids) => ids.push(action),
        }
        Ok(())
    }
}

#[derive(Default)]
struct OriginalRows {
    directory: Vec<usize>,
    rows: Vec<u32>,
}

impl OriginalRows {
    fn view(&self, edges: usize) -> CsrEdgeRows<'_> {
        CsrEdgeRows::Blocks {
            edges,
            directory: &self.directory,
            rows: &self.rows,
        }
    }

    fn append(
        &mut self,
        targets: CsrTargets<'_>,
        representatives: &[u32],
        target: u32,
        original: u32,
        memory: &mut NativeMemory,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        let position = targets.len() % CsrEdgeRows::BLOCK;
        if position == 0 {
            let previous = self.directory.last().copied().unwrap_or(0);
            reserve(&mut self.directory, 1, memory, control)?;
            self.directory.push(previous);
        }
        // Every previous explicit block is full. A partial explicit block is the current one.
        if self.rows.len().is_multiple_of(CsrEdgeRows::BLOCK) {
            if original == representatives[target as usize] {
                return Ok(());
            }
            // Only this block's earlier entries need materialization. Until now
            // every one used its destination's first-discovery original.
            reserve(&mut self.rows, CsrEdgeRows::BLOCK, memory, control)?;
            for edge in targets.len() - position..targets.len() {
                let to = targets
                    .get(edge)
                    .ok_or_else(|| bug("missing native target in original row block"))?;
                self.rows.push(representatives[to as usize]);
            }
            let end = self
                .directory
                .last_mut()
                .ok_or_else(|| bug("missing native original row block"))?;
            *end += 1;
        }
        self.rows.push(original);
        Ok(())
    }
}

/// Persistent exact originals and one CSR, kept alive through analysis.
pub(crate) struct NativeGraph {
    store: Arc<Store>,
    nvars: usize,
    rows: Vec<Handle>,
    retained_rows: usize,
    representatives: Vec<u32>,
    offsets: Vec<usize>,
    targets: PackedTargets,
    ordinal_sources: Vec<u32>,
    ordinal_blocks: Vec<usize>,
    ordinals: Vec<u16>,
    large_ordinals: Vec<(usize, u64)>,
    actions: ActionIds,
    edge_rows: OriginalRows,
    roots: Vec<usize>,
    action_labels: Vec<String>,
    closed_rows: usize,
    limits: Limits,
    memory: NativeMemory,
}

impl NativeGraph {
    pub(crate) fn owns_store(&self, store: &Arc<Store>) -> bool {
        Arc::ptr_eq(&self.store, store)
    }

    pub(crate) fn input(&self) -> CsrTargetStorage<'_> {
        CsrTargetStorage {
            offsets: &self.offsets,
            targets: (&self.targets).into(),
            ordinals: CsrOrdinals::PositionBlocks {
                edges: self.targets.len(),
                sources: &self.ordinal_sources,
                directory: &self.ordinal_blocks,
                words: &self.ordinals,
                large: &self.large_ordinals,
            },
            actions: self.actions.view(),
            edge_rows: self.edge_rows.view(self.targets.len()),
            representatives: &self.representatives,
            roots: &self.roots,
            action_labels: &self.action_labels,
            retained_rows: self.retained_rows,
            closed_rows: self.closed_rows,
        }
    }

    pub(crate) fn row(&self, id: usize) -> Result<&[Handle], AnalysisError> {
        // Every atom demand reads its rows here: no error value is built
        // (and dropped) on the success path.
        if id >= self.retained_rows {
            return Err(AnalysisError::Inconsistent("native retained state row"));
        }
        let Some(start) = id.checked_mul(self.nvars) else {
            return Err(AnalysisError::Inconsistent("native state row offset"));
        };
        let Some(end) = start.checked_add(self.nvars) else {
            return Err(AnalysisError::Inconsistent("native state row end"));
        };
        let Some(row) = self.rows.get(start..end) else {
            return Err(AnalysisError::Inconsistent("native state row width"));
        };
        Ok(row)
    }

    pub(crate) fn state_count(&self) -> usize {
        self.representatives.len()
    }

    pub(crate) const fn limits(&self) -> Limits {
        self.limits
    }

    pub(crate) const fn memory(&self) -> NativeMemory {
        self.memory
    }

    fn retain_row(
        &mut self,
        original: &[Handle],
        control: &EvaluationControl,
    ) -> Result<u32, Verdict> {
        let row =
            u32::try_from(self.retained_rows).map_err(|_| limit("native retained row ids"))?;
        reserve(&mut self.rows, original.len(), &mut self.memory, control)?;
        for (i, &handle) in original.iter().enumerate() {
            checkpoint(control, i)?;
            if handle.is_transient() || handle.is_empty() {
                return Err(bug("native original is not a complete persistent row"));
            }
            self.rows.push(handle);
        }
        self.retained_rows = self
            .retained_rows
            .checked_add(1)
            .ok_or_else(|| limit("native retained rows"))?;
        Ok(row)
    }

    fn reserve_edge(&mut self, control: &EvaluationControl) -> Result<(), Verdict> {
        if self.targets.len() >= self.limits.edges {
            return Err(limit("edges"));
        }
        poll(control)?;
        self.actions.reserve(&mut self.memory, control)
    }

    fn push_edge(
        &mut self,
        to: u32,
        ordinal: Option<u64>,
        action: Option<u32>,
        row: u32,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        self.edge_rows.append(
            (&self.targets).into(),
            &self.representatives,
            to,
            row,
            &mut self.memory,
            control,
        )?;
        self.actions.push(action)?;
        self.push_ordinal(ordinal, control)?;
        let before = self.targets.capacities();
        let pushed = self.targets.try_push(to);
        for (old, new) in before.into_iter().zip(self.targets.capacities()) {
            self.memory.growth(old, new)?;
        }
        pushed.map_err(|_| limit("native target allocation"))?;
        Ok(())
    }

    fn push_ordinal(
        &mut self,
        ordinal: Option<u64>,
        control: &EvaluationControl,
    ) -> Result<(), Verdict> {
        let edge = self.targets.len();
        let position = edge % CsrEdgeRows::BLOCK;
        let previous = self.ordinal_blocks.last().copied().unwrap_or(0);
        if position == 0 {
            reserve(&mut self.ordinal_blocks, 1, &mut self.memory, control)?;
            reserve(&mut self.ordinal_sources, 1, &mut self.memory, control)?;
            let source = u32::try_from(self.closed_rows).map_err(|_| limit("native vertex ids"))?;
            self.ordinal_blocks.push(previous);
            self.ordinal_sources.push(source);
        }
        let implicit = ordinal.is_none_or(|n| n == (edge - self.offsets[self.closed_rows]) as u64);
        if self.ordinals.len().is_multiple_of(CsrEdgeRows::BLOCK) {
            if implicit {
                return Ok(());
            }
            reserve(
                &mut self.ordinals,
                CsrEdgeRows::BLOCK,
                &mut self.memory,
                control,
            )?;
            self.ordinals
                .resize(self.ordinals.len() + position, u16::MAX);
            *self
                .ordinal_blocks
                .last_mut()
                .ok_or_else(|| bug("missing ordinal block"))? += 1;
        }
        let entry = match ordinal.filter(|_| !implicit) {
            None => u16::MAX,
            Some(n) => match u16::try_from(n) {
                Ok(word) if word < CsrOrdinals::EXTENDED => word,
                _ => {
                    reserve(&mut self.large_ordinals, 1, &mut self.memory, control)?;
                    self.large_ordinals.push((edge, n));
                    CsrOrdinals::EXTENDED
                }
            },
        };
        self.ordinals.push(entry);
        Ok(())
    }
}

/// The actual explorer sink: converts eagerly, retains native rows, closes CSR
/// sources in discovery order, and exposes only completed levels to analysis.
pub(crate) struct NativeSink<'model, 'callback> {
    graph: NativeGraph,
    values: NativeValues<'model>,
    /// Fingerprint mode: discovery ids by state key (equal keys are one
    /// vertex). Unused when the explorer names seen targets by id.
    ids: HashMap<u64, u32>,
    /// The explorer's exact seen set names every seen target by its
    /// discovery id ([`Step::seen`]); keys are then never identities.
    exact: bool,
    control: Arc<EvaluationControl>,
    on_level: Option<&'callback mut OnLevel<'model, 'callback>>,
    error: Option<Verdict>,
    last_level: u64,
    expanding: bool,
    source_setup: Option<SourceRowSetup>,
    source_rows: Option<dive_liveness::analyze::owned::SourceRowDriver>,
    source_error: Option<Verdict>,
}

impl<'model, 'callback> NativeSink<'model, 'callback> {
    pub(crate) fn new(
        model: &'model dive_model::Model,
        store: Arc<Store>,
        limits: Limits,
        options: &CheckOptions,
        on_level: Option<&'callback mut OnLevel<'model, 'callback>>,
    ) -> Result<Self, Verdict> {
        let control = EvaluationControl::new(&options.budget, Phase::Capture)
            .map_err(|error| bug(&error.to_string()))?;
        let values = NativeValues::new(model, Arc::clone(&store), &options.budget)?;
        let mut graph = NativeGraph {
            store,
            nvars: model.ir.vars().len(),
            rows: Vec::new(),
            retained_rows: 0,
            representatives: Vec::new(),
            offsets: Vec::new(),
            targets: PackedTargets::default(),
            ordinal_sources: Vec::new(),
            ordinal_blocks: Vec::new(),
            ordinals: Vec::new(),
            large_ordinals: Vec::new(),
            actions: ActionIds::for_labels(model.actions.len()),
            edge_rows: OriginalRows::default(),
            roots: Vec::new(),
            action_labels: Vec::new(),
            closed_rows: 0,
            limits,
            memory: NativeMemory {
                fixed_bytes: std::mem::size_of::<NativeGraph>(),
                ..NativeMemory::default()
            },
        };
        reserve(&mut graph.offsets, 1, &mut graph.memory, &control)?;
        graph.offsets.push(0);
        reserve(
            &mut graph.action_labels,
            model.actions.len(),
            &mut graph.memory,
            &control,
        )?;
        for (i, action) in model.actions.iter().enumerate() {
            checkpoint(&control, i)?;
            let label = action.label();
            let mut retained = String::new();
            poll(&control)?;
            retained
                .try_reserve(label.len())
                .map_err(|_| limit("native action label allocation"))?;
            graph.memory.buffer_bytes = graph
                .memory
                .buffer_bytes
                .checked_add(retained.capacity())
                .ok_or_else(|| limit("native retained bytes"))?;
            graph.memory.growth_overlap_upper_bound_bytes = graph
                .memory
                .growth_overlap_upper_bound_bytes
                .max(graph.memory.buffer_bytes);
            for (i, character) in label.chars().enumerate() {
                checkpoint(&control, i)?;
                retained.push(character);
            }
            graph.action_labels.push(retained);
        }
        poll(&control)?;
        Ok(Self {
            graph,
            values,
            ids: HashMap::new(),
            exact: false,
            control,
            on_level,
            error: None,
            last_level: 0,
            expanding: false,
            source_setup: None,
            source_rows: None,
            source_error: None,
        })
    }

    pub(crate) fn take_error(&mut self) -> Option<Verdict> {
        self.error.take()
    }

    /// Converts rows only when they are read; see
    /// [`NativeValues::defer_admission`] for the caller's obligations.
    pub(crate) fn defer_admission(&mut self) {
        self.values.defer_admission();
    }

    /// Resolves seen targets by the discovery ids of the explorer's exact
    /// seen set instead of by state key. The search must then run with
    /// exact storage: a seen step without an id is an internal error, as is
    /// one with an id when this is not called.
    pub(crate) fn use_exact_identities(&mut self) {
        self.exact = true;
    }

    pub(crate) fn prepare_source_rows(&mut self, setup: SourceRowSetup) {
        self.source_setup = Some(setup);
    }

    pub(crate) fn take_source_rows(
        &mut self,
    ) -> Option<dive_liveness::analyze::owned::SourceRowDriver> {
        self.source_rows.take()
    }

    pub(crate) fn take_source_error(&mut self) -> Option<Verdict> {
        self.source_error.take()
    }

    pub(crate) fn finish(self) -> (NativeGraph, NativeValues<'model>) {
        // The discovery index is no longer required by the immutable graph.
        // Its opaque allocation is released before final temporal analysis.
        (self.graph, self.values)
    }

    fn stop(&mut self, verdict: Verdict) -> Stopped {
        self.error.get_or_insert(verdict);
        Stopped
    }

    fn record(&mut self, heap: &Heap, step: Step<'_>) -> Result<(), Verdict> {
        // Preserve the converter-before-state/edge-admission precedence of the
        // value-graph capture,
        // including seen originals and run-local name interning order. A
        // deferred admission happens at the next read, in retention order.
        // A seen root that need not be retained is still admitted at once,
        // after the pending rows, as it may differ from its representative.
        let admitted = self
            .values
            .admits_now(step.parent.is_none() && step.new.is_none());
        if admitted {
            self.values
                .catch_up(&self.graph)
                .map_err(crate::core_native_values::capture_error)?;
            self.values.admit(step.original)?;
        }
        self.record_admitted(heap, step)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "ordered admission and exact original retention"
    )]
    fn record_admitted(&mut self, heap: &Heap, step: Step<'_>) -> Result<(), Verdict> {
        if !Arc::ptr_eq(heap.store(), &self.graph.store) || step.original.len() != self.graph.nvars
        {
            return Err(bug("native original store or row width differs"));
        }
        poll(&self.control)?;
        let parent = step
            .parent
            .map(usize::try_from)
            .transpose()
            .map_err(|_| bug("native source exceeds address space"))?;
        if parent.is_some_and(|source| source != self.graph.closed_rows)
            || (parent.is_none() && self.expanding)
        {
            return Err(bug("native sources are not committed in discovery order"));
        }
        self.expanding |= parent.is_some();
        let action = step
            .action
            .map(u32::try_from)
            .transpose()
            .map_err(|_| bug("native action index exceeds 32 bits"))?;
        if action.is_some_and(|id| id as usize >= self.graph.action_labels.len()) {
            return Err(bug("native action index out of range"));
        }
        // Conversion has already happened. Match the capture's state-before-edge
        // limit precedence before adding capture-specific retention buffers.
        if step.new.is_some() && self.graph.state_count() >= self.graph.limits.states {
            return Err(limit("states"));
        }
        if parent.is_some() && self.graph.targets.len() >= self.graph.limits.edges {
            return Err(limit("edges"));
        }
        let (target, original) = if let Some((id, canonical)) = step.new {
            let target = u32::try_from(id).map_err(|_| limit("native vertex ids"))?;
            if target as usize != self.graph.state_count()
                || step.seen.is_some()
                || (!self.exact && self.ids.contains_key(&step.key))
                || canonical.len() != self.graph.nvars
            {
                return Err(bug("native discovery ids differ from trace log"));
            }
            reserve(
                &mut self.graph.representatives,
                1,
                &mut self.graph.memory,
                &self.control,
            )?;
            reserve(
                &mut self.graph.offsets,
                1,
                &mut self.graph.memory,
                &self.control,
            )?;
            poll(&self.control)?;
            if !self.exact {
                self.ids
                    .try_reserve(1)
                    .map_err(|_| limit("native discovery index allocation"))?;
                self.graph.memory.key_capacity = self.ids.capacity();
            }
            poll(&self.control)?;
            let row = self.graph.retain_row(step.original, &self.control)?;
            self.graph.representatives.push(row);
            self.graph.offsets.push(self.graph.targets.len());
            if !self.exact {
                self.ids.insert(step.key, target);
                self.graph.memory.key_entries = self.ids.len();
                self.graph.memory.key_capacity = self.ids.capacity();
            }
            (target, row)
        } else {
            let target = match (self.exact, step.seen) {
                (true, Some(id)) => u32::try_from(id)
                    .ok()
                    .filter(|&id| (id as usize) < self.graph.state_count())
                    .ok_or_else(|| bug("native exact seen id is not a discovered vertex"))?,
                (true, None) => return Err(bug("native exact seen state has no discovery id")),
                (false, Some(_)) => {
                    return Err(bug("native fingerprint seen state carries a discovery id"));
                }
                (false, None) => *self
                    .ids
                    .get(&step.key)
                    .ok_or_else(|| bug("native seen state has no discovery id"))?,
            };
            let representative = self.graph.representatives[target as usize];
            let original = if parent.is_none()
                || same_rows(
                    self.graph
                        .row(representative as usize)
                        .map_err(|_| bug("native representative row missing"))?,
                    step.original,
                    &self.control,
                )? {
                representative
            } else {
                self.graph.retain_row(step.original, &self.control)?
            };
            (target, original)
        };
        if parent.is_some() {
            self.graph.reserve_edge(&self.control)?;
            self.graph
                .push_edge(target, Some(step.ordinal), action, original, &self.control)?;
        } else {
            if self.graph.closed_rows != 0 || !self.graph.targets.is_empty() {
                return Err(bug("native root emitted after source expansion"));
            }
            let root = target as usize;
            if let Err(at) = self.graph.roots.binary_search(&root) {
                reserve(
                    &mut self.graph.roots,
                    1,
                    &mut self.graph.memory,
                    &self.control,
                )?;
                self.graph.roots.insert(at, root);
            }
        }
        Ok(())
    }
}

impl GraphSink for NativeSink<'_, '_> {
    fn max_expansion_units(&self) -> usize {
        MAX_EXPANSION_UNITS
    }

    fn max_successors_per_unit(&self) -> u64 {
        self.graph.limits.successors_per_unit
    }

    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        let result = if generated > self.graph.limits.generated {
            Err(limit("generated states"))
        } else if ordinal.is_some_and(|ordinal| ordinal >= self.graph.limits.successors_per_unit) {
            Err(limit("successors per source (scratch admission)"))
        } else {
            Ok(())
        };
        result.map_err(|error| self.stop(error))
    }

    fn state(&mut self, heap: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        self.record(heap, step).map_err(|error| self.stop(error))
    }

    fn stutter(&mut self, source: u64) -> Result<(), Stopped> {
        let result = (|| {
            poll(&self.control)?;
            let source = u32::try_from(source).map_err(|_| limit("native vertex ids"))?;
            if source as usize != self.graph.closed_rows {
                return Err(bug("native stutter source out of order"));
            }
            self.expanding = true;
            let row = *self
                .graph
                .representatives
                .get(source as usize)
                .ok_or_else(|| bug("native stutter source not discovered"))?;
            self.graph.reserve_edge(&self.control)?;
            self.graph
                .push_edge(source, None, None, row, &self.control)?;
            self.graph.closed_rows += 1;
            self.graph.offsets[self.graph.closed_rows] = self.graph.targets.len();
            Ok(())
        })();
        result.map_err(|error| self.stop(error))
    }

    fn source_row(&mut self, _heap: &mut Heap, row: SourceRow<'_>) -> Result<(), Stopped> {
        if self.source_setup.is_none() && self.source_rows.is_none() {
            return Ok(());
        }
        if row.source != 0
            || row.stats.initial_generated != 1
            || self.graph.roots.len() != 1
            || row.original.iter().any(|&value| {
                !matches!(
                    value.tag(),
                    dive_core::handle::Tag::Bool
                        | dive_core::handle::Tag::Int
                        | dive_core::handle::Tag::Str
                        | dive_core::handle::Tag::Model
                )
            })
        {
            self.source_setup = None;
            self.source_rows = None;
            return Ok(());
        }
        let result = (|| {
            if let Some(setup) = self.source_setup.take() {
                self.source_rows = setup.bind()?;
            }
            if self.source_rows.is_none() {
                return Ok(());
            }
            let source = usize::try_from(row.source).map_err(|_| bug("native source row index"))?;
            let original = *self
                .graph
                .representatives
                .get(source)
                .ok_or_else(|| bug("native source row representative"))?
                as usize;
            let state = self
                .values
                .read_retained_as(&self.graph, original, row.original, &self.control)
                .map_err(source_error)?;
            let start = *self
                .graph
                .offsets
                .get(source)
                .ok_or_else(|| bug("native source row offset"))?;
            // A completed source ends in its implicit stutter. A generator
            // failure leaves an open row containing only admitted successors.
            let end = if row.next_error.is_some() {
                self.graph.targets.len()
            } else {
                self.graph
                    .targets
                    .len()
                    .checked_sub(1)
                    .filter(|&end| end >= start)
                    .ok_or_else(|| bug("missing native source row stutter"))?
            };
            let mut targets = Vec::new();
            for index in start..end {
                let retained = self
                    .graph
                    .input()
                    .edge_row(index)
                    .ok_or_else(|| bug("native source row original"))?
                    as usize;
                targets.try_reserve(1).map_err(|_| Verdict::ResourceLimit {
                    message: "native source-row targets allocation".into(),
                })?;
                targets.push(
                    self.values
                        .read_retained(&self.graph, retained, &self.control)
                        .map_err(source_error)?,
                );
            }
            let counts = Stats {
                generated: row.stats.generated,
                distinct: row.stats.distinct,
                depth: row.stats.depth,
                left_on_queue: row.stats.left_on_queue,
                initial_generated: row.stats.initial_generated,
            };
            let Some(driver) = self.source_rows.as_mut() else {
                return Ok(());
            };
            let failed = if row.next_error.is_some() {
                driver.failed_source(source, &state, &targets, row.emitted, counts)
            } else {
                driver.completed_source(source, &state, &targets, row.emitted, counts)
            }
            .map_err(source_error)?;
            if failed {
                return Err(Verdict::Unsupported {
                    feature: SOURCE_ROW_STOP.into(),
                });
            }
            self.source_rows = None;
            Ok(())
        })();
        result.map_err(|error| {
            self.source_error = Some(error.clone());
            self.stop(error)
        })
    }

    fn level_done(&mut self, level: u64, stats: dive_explore::Stats) -> Result<bool, Stopped> {
        let result = (|| {
            if self.last_level.checked_add(1) != Some(level) {
                return Err(bug("native completed levels out of order"));
            }
            for (i, offset) in self
                .graph
                .offsets
                .iter_mut()
                .skip(self.graph.closed_rows + 1)
                .enumerate()
            {
                checkpoint(&self.control, i)?;
                *offset = self.graph.targets.len();
            }
            poll(&self.control)?;
            self.last_level = level;
            let Some(on_level) = self.on_level.as_deref_mut() else {
                return Ok(false);
            };
            on_level(
                level,
                &self.graph,
                &mut self.values,
                Stats {
                    generated: stats.generated,
                    distinct: stats.distinct,
                    left_on_queue: stats.left_on_queue,
                    depth: stats.depth,
                    initial_generated: stats.initial_generated,
                },
            )
        })();
        result.map_err(|error| self.stop(error))
    }
}

fn same_rows(a: &[Handle], b: &[Handle], control: &EvaluationControl) -> Result<bool, Verdict> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        checkpoint(control, i)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reserve<T>(
    buffer: &mut Vec<T>,
    additional: usize,
    memory: &mut NativeMemory,
    control: &EvaluationControl,
) -> Result<(), Verdict> {
    poll(control)?;
    let before = buffer.capacity() * std::mem::size_of::<T>();
    buffer
        .len()
        .checked_add(additional)
        .ok_or_else(|| limit("native buffer entries"))?;
    buffer
        .try_reserve(additional)
        .map_err(|_| limit("native buffer allocation"))?;
    let after = buffer.capacity() * std::mem::size_of::<T>();
    memory.growth(before, after)?;
    poll(control)
}

fn checkpoint(control: &EvaluationControl, position: usize) -> Result<(), Verdict> {
    if position.is_multiple_of(256) {
        poll(control)?;
    }
    Ok(())
}

fn poll(control: &EvaluationControl) -> Result<(), Verdict> {
    control.check().map_err(|interrupted| {
        control
            .verdict(&dive_eval::EvalError::interrupted(interrupted))
            .unwrap_or_else(|| bug("native control interrupted without matching exhaustion"))
    })
}

fn bug(message: &str) -> Verdict {
    Verdict::InternalError {
        message: format!("native core graph: {message}"),
    }
}

fn limit(feature: &str) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("behavior graph capture {feature}"),
    }
}

#[cfg(test)]
#[path = "core_native_graph_tests.rs"]
mod tests;
