//! Reuse of closed ordinary Core rows for the exact bound ENABLED query.
use super::{AnalysisError, EvaluationControl, NativeAtoms, PredicateAtom, Reader, check};
use dive_runtime::capture::CsrCapture;

impl NativeAtoms {
    /// Only the ordinary live explorer promises that every generated original
    /// in this run was available to the capture.
    pub(crate) fn enable_live_emissions(
        &mut self,
        model: &dive_model::Model,
        options: &dive_runtime::CheckOptions,
    ) {
        self.live_emissions = options.workers == 1
            && options.profile == dive_eval::Profile::Dive
            && options.eval == dive_eval::Options::dive()
            && !model
                .output
                .iter()
                .any(|line| matches!(line, dive_model::Message::Warning { .. }))
            && model.constraints.is_empty()
            && model.action_constraints.is_empty()
            && model.view.is_none()
            && model.symmetry.is_none()
            && model.symmetry_reduction.is_none()
            && model.actions.len() < 64
            && self.compiled.iter().any(|c| c.emission.is_some());
    }
}

#[derive(Default)]
pub(super) struct Rows {
    attempted: bool,
    /// Bit 63 marks a source with at least one missing original action ID.
    masks: Vec<u64>,
}

impl Rows {
    pub(super) fn bytes(&self) -> usize {
        self.masks.capacity() * size_of::<u64>()
    }

    fn mask(
        &mut self,
        graph: &CsrCapture<'_>,
        source: usize,
        owner: &EvaluationControl,
        control: &EvaluationControl,
    ) -> Result<Option<u64>, AnalysisError> {
        if !self.attempted {
            self.attempted = true;
            let size = graph.closed_rows();
            if size > (8 << 20) / size_of::<u64>() || self.masks.try_reserve_exact(size).is_err() {
                return Ok(None);
            }
            for row in 0..size {
                if row.is_multiple_of(512) {
                    check(owner, control)?;
                }
                let Some(count) = graph.original_count(row) else {
                    self.masks.push(1 << 63);
                    continue;
                };
                let mut mask = 0;
                for position in 0..count {
                    if position != 0 && position.is_multiple_of(1024) {
                        check(owner, control)?;
                    }
                    let action = graph.original_action(row, position).unwrap_or(63).min(63);
                    mask |= 1u64 << action;
                }
                self.masks.push(mask);
            }
        }
        Ok(self
            .masks
            .get(source)
            .copied()
            .filter(|mask| mask >> 63 == 0))
    }
}

impl Reader<'_, '_> {
    pub(super) fn emitted_truth(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        source: usize,
        before: usize,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        if !self.atoms.live_emissions
            || self.prefix.is_none()
            || source >= graph.closed_rows()
            || graph.representative_row(source) != Some(before)
            || self.atoms.fuel != super::NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || !std::ptr::eq(self.proofs.graph.input().offsets, graph.offsets())
            || !self
                .proofs
                .graph
                .owns_store(self.atoms.machine.heap.store())
        {
            return Ok(None);
        }
        let Some(pos) = self.atoms.position(atom) else {
            return Ok(None);
        };
        let compiled = &self.atoms.compiled[pos];
        let Some((index, cert)) = compiled.emission else {
            return Ok(None);
        };
        let negate = compiled.negate;
        if compiled.exhausted_in_a_row != 0
            || !self.proofs.closed(&self.atoms.machine.heap, before)?
        {
            return Ok(None);
        }
        check(&self.atoms.control, control)?;
        let Some(mask) = self
            .emissions
            .mask(graph, source, &self.atoms.control, control)?
        else {
            return Ok(None);
        };
        let present = mask & (1u64 << index) != 0;
        if (!cert.quiet_guards || (present && !cert.quiet_suffix))
            && self.atoms.machine.heap.store().saw_missing_update()
        {
            return Ok(None);
        }
        let value = if !present {
            false
        } else if let Some(subscript) = cert.subscript {
            let src = self.proofs.graph.row(before)?;
            let mut changed = false;
            let Some(count) = graph.original_count(source) else {
                return Ok(None);
            };
            for position in 0..count {
                if position.is_multiple_of(1024) {
                    check(&self.atoms.control, control)?;
                }
                if graph
                    .original_action(source, position)
                    .map(|id| id as usize)
                    != Some(index)
                {
                    continue;
                }
                let Some(after) = graph.original_row(source, position) else {
                    return Ok(None);
                };
                if !self.proofs.closed(&self.atoms.machine.heap, after)? {
                    return Ok(None);
                }
                let dst = self.proofs.graph.row(after)?;
                let mut leaves = subscript;
                while leaves != 0 {
                    let v = leaves.trailing_zeros() as usize;
                    if !self.atoms.machine.heap.eq(src[v], dst[v]) {
                        changed = true;
                        break;
                    }
                    leaves &= leaves - 1;
                }
                if changed {
                    break;
                }
            }
            changed
        } else {
            true
        };
        let truth = value != negate;
        self.atoms.emission_hits[usize::from(present)] += 1;
        if self.atoms.audit {
            self.audit(atom, before, None, truth, control)?;
        }
        Ok(Some(truth))
    }
}
