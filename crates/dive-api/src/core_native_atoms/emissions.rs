//! Reuse of closed ordinary Core rows for the exact bound ENABLED query.
mod images;
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
    first: usize,
    allocation_failed: bool,
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
        self.get(graph.closed_rows(), source, |row| {
            if row.is_multiple_of(512) {
                check(owner, control)?;
            }
            let Some(count) = graph.original_count(row) else {
                return Ok(1 << 63);
            };
            let mut mask = 0;
            for position in 0..count {
                if position != 0 && position.is_multiple_of(1024) {
                    check(owner, control)?;
                }
                let action = graph.original_action(row, position).unwrap_or(63).min(63);
                mask |= 1u64 << action;
            }
            Ok(mask)
        })
    }

    fn get(
        &mut self,
        size: usize,
        source: usize,
        mut build: impl FnMut(usize) -> Result<u64, AnalysisError>,
    ) -> Result<Option<u64>, AnalysisError> {
        const FULL_ROWS: usize = (8 << 20) / size_of::<u64>();
        const WINDOW_ROWS: usize = 16 * 1024;
        if source >= size || self.allocation_failed {
            return Ok(None);
        }
        if source
            .checked_sub(self.first)
            .is_none_or(|offset| offset >= self.masks.len())
        {
            // Small captures keep their full index. Larger ones are consumed a
            // column at a time, with consecutive source IDs inside each column.
            let width = if size <= FULL_ROWS { size } else { WINDOW_ROWS };
            self.first = (source / width) * width;
            let count = width.min(size - self.first);
            self.masks.clear();
            if self.masks.try_reserve_exact(count).is_err() {
                self.allocation_failed = true;
                return Ok(None);
            }
            for row in self.first..self.first + count {
                match build(row) {
                    Ok(mask) => self.masks.push(mask),
                    Err(stop) => {
                        self.masks.clear();
                        return Err(stop);
                    }
                }
            }
        }
        Ok(self
            .masks
            .get(source - self.first)
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
        if !self.atoms.live_emissions {
            return Ok(None);
        }
        let Some(original) = self.emission_capture.as_ref() else {
            return Ok(None);
        };
        if self.prefix.is_none()
            || source >= graph.closed_rows()
            || graph.representative_row(source) != Some(before)
            || self.atoms.fuel != super::NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || !original.same_validation(graph)
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
            .mask(original, source, &self.atoms.control, control)?
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
            let Some(count) = original.original_count(source) else {
                return Ok(None);
            };
            for position in 0..count {
                if position.is_multiple_of(1024) {
                    check(&self.atoms.control, control)?;
                }
                if original
                    .original_action(source, position)
                    .map(|id| id as usize)
                    != Some(index)
                {
                    continue;
                }
                let Some(after) = original.original_row(source, position) else {
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

#[cfg(test)]
mod tests {
    use super::Rows;

    #[test]
    fn an_interrupted_fill_cannot_publish_a_partial_window() {
        let mut rows = Rows::default();
        let result = rows.get((1 << 20) + 2, 0, |id| {
            if id == 1 {
                Err(super::AnalysisError::Limit("test budget"))
            } else {
                Ok(1)
            }
        });
        assert!(result.is_err());
        let mut rebuilt = 0;
        let result = rows.get((1 << 20) + 2, 0, |_| {
            rebuilt += 1;
            Ok(2)
        });
        assert!(matches!(result, Ok(Some(2))));
        assert_eq!(rebuilt, 16 * 1024);
    }

    #[test]
    fn large_closed_captures_rebuild_only_the_requested_window() {
        let mut rows = Rows::default();
        let total = (1 << 20) + 3;
        let mut demanded = Vec::new();
        for source in [total - 1, total - 3, total - 2] {
            let result = rows.get(total, source, |id| {
                demanded.push(id);
                Ok(if id == total - 2 { 1 << 63 } else { id as u64 })
            });
            assert!(
                matches!(result, Ok(value) if value == (source != total - 2).then_some(source as u64))
            );
        }
        assert_eq!(demanded, [total - 3, total - 2, total - 1]);
        assert!(matches!(
            rows.get(total, 21, |id| Ok(id as u64)),
            Ok(Some(21))
        ));
        assert!(rows.bytes() <= 128 * 1024);
        assert!(matches!(rows.get(total, total, |_| Ok(0)), Ok(None)));
        assert!(matches!(
            rows.get(total, total - 2, |_| Ok(1 << 63)),
            Ok(None)
        ));
    }
}
