//! Completed native columns from earlier checks on the same growing capture.
use super::{AnalysisError, EvaluationControl, NATIVE_FUEL, PredicateAtom, Reader, check};
use dive_liveness::analyze::periodic::external::LabelPrefix;
use dive_liveness::formula::AtomKind;
use dive_runtime::capture::CsrCapture;

pub(super) struct Column {
    sources: usize,
    cells: usize,
    retained: usize,
    words: Vec<u64>,
}

impl Reader<'_, '_> {
    /// Called only by the live explorer's level callback and its final check.
    /// They share the run's `NativeAtoms` and one append-only `NativeSink`. Edit
    /// reuse, simulation, external readers and reconstructed inputs never opt in.
    pub(crate) fn live_prefix(&mut self, save: bool) {
        self.prefix = Some(save);
    }

    fn column_index(&self, atom: &PredicateAtom, graph: &CsrCapture<'_>) -> Option<usize> {
        if self.prefix.is_none()
            || self.atoms.audit
            || self.atoms.fuel != NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || !self
                .proofs
                .graph
                .owns_store(self.atoms.machine.heap.store())
            || !std::ptr::eq(graph.offsets(), self.proofs.graph.input().offsets)
        {
            return None;
        }
        self.atoms
            .position(atom)
            .filter(|&i| self.atoms.compiled[i].exhausted_in_a_row == 0)
    }

    pub(super) fn previous_column(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        control: &EvaluationControl,
    ) -> Result<Option<LabelPrefix<'_>>, AnalysisError> {
        let Some(i) = self.column_index(atom, graph) else {
            return Ok(None);
        };
        let Some(column) = self.atoms.compiled[i].column.as_ref() else {
            return Ok(None);
        };
        check(&self.atoms.control, control)?;
        if column.retained > graph.retained_rows() {
            return Ok(None);
        }
        for row in self.prefix_validated..column.retained {
            if row.is_multiple_of(1024) {
                check(&self.atoms.control, control)?;
            }
            if !self.proofs.closed(&self.atoms.machine.heap, row)? {
                return Ok(None);
            }
            self.prefix_validated = row + 1;
        }
        self.atoms.prefix_cells_reused += column.cells as u64;
        Ok(Some(LabelPrefix {
            sources: column.sources,
            cells: column.cells,
            words: &column.words,
        }))
    }

    pub(super) fn store_column(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        input: &mut dyn ExactSizeIterator<Item = u64>,
        cap: usize,
        control: &EvaluationControl,
    ) -> Result<(), AnalysisError> {
        if self.prefix != Some(true) {
            return Ok(());
        }
        let Some(i) = self.column_index(atom, graph) else {
            return Ok(());
        };
        if self.proofs.graph.input().retained_rows as u64 != self.proofs.metrics.certified {
            return Ok(());
        }
        let (sources, cells) = match atom.kind {
            AtomKind::State => (graph.state_count(), graph.state_count()),
            AtomKind::Action => (
                graph.closed_rows(),
                graph.offsets().last().copied().unwrap_or(0),
            ),
        };
        let count = input.len();
        if count != cells.div_ceil(64)
            || count.saturating_mul(8) > cap.saturating_sub(self.atoms.prefix_bytes)
        {
            return Ok(());
        }
        let mut words = Vec::new();
        if words.try_reserve_exact(count).is_err()
            || words.capacity().saturating_mul(8) > cap.saturating_sub(self.atoms.prefix_bytes)
        {
            return Ok(());
        }
        for (i, w) in input.enumerate() {
            if i.is_multiple_of(1024) {
                check(&self.atoms.control, control)?;
            }
            words.push(w);
        }
        self.atoms.prefix_bytes += words.capacity() * 8;
        if let Some(old) = self.atoms.compiled[i].column.replace(Column {
            sources,
            cells,
            retained: graph.retained_rows(),
            words,
        }) {
            self.atoms.prefix_bytes -= old.words.capacity() * 8;
        }
        Ok(())
    }
}
