//! Aligned full-variable ENABLED words from complete native source rows.
use super::{AnalysisError, CsrCapture, EvaluationControl, PredicateAtom, Reader, check};

/// One group of words per 64 sources. The first word records whether all
/// their originals were admitted, followed by presence and change per branch.
#[derive(Default)]
pub(in crate::core_native_atoms) struct PackedRows {
    attempted: bool,
    words: Vec<u64>,
    stride: usize,
}

impl PackedRows {
    pub(in crate::core_native_atoms) fn bytes(&self) -> usize {
        self.words.capacity() * size_of::<u64>()
    }

    fn prepare(&mut self, closed: usize, branches: usize) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        if branches >= 64 {
            return;
        }
        let stride = 1 + 2 * branches;
        let Some(len) = (closed / 64).checked_mul(stride) else {
            return;
        };
        // Leave capacity for the existing per-source presence mask, including
        // when a tail or a partial subscript needs the ordinary query path.
        let available_words = (1_usize << 20).saturating_sub(closed);
        if len <= available_words && self.words.try_reserve_exact(len).is_ok() {
            if self.words.capacity() <= available_words {
                self.words.resize(len, 0);
                self.stride = stride;
            } else {
                self.words = Vec::new();
            }
        }
    }
}

impl Reader<'_, '_> {
    pub(crate) fn emitted_word(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        first: usize,
        control: &EvaluationControl,
    ) -> Result<Option<u64>, AnalysisError> {
        if !self.atoms.live_emissions
            || self.prefix.is_none()
            || !first.is_multiple_of(64)
            || graph.closed_rows().saturating_sub(first) < 64
            || self.atoms.fuel != super::super::NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || !self
                .emission_capture
                .as_ref()
                .is_some_and(|owner| owner.same_validation(graph))
            || !self
                .proofs
                .graph
                .owns_store(self.atoms.machine.heap.store())
        {
            return Ok(None);
        }
        let Some(position) = self.atoms.position(atom) else {
            return Ok(None);
        };
        let compiled = &self.atoms.compiled[position];
        let Some((branch, cert)) = compiled.emission else {
            return Ok(None);
        };
        if compiled.exhausted_in_a_row != 0
            || ((!cert.quiet_guards || !cert.quiet_suffix)
                && self.atoms.machine.heap.store().saw_missing_update())
        {
            return Ok(None);
        }
        let negate = compiled.negate;
        let retained = self.proofs.graph;
        let before = graph
            .representative_row(first)
            .ok_or(AnalysisError::Inconsistent("enabled word representative"))?;
        let width = retained.row(before)?.len();
        if cert.subscript.is_some_and(|mask| {
            mask.count_ones() as usize != width || mask.trailing_ones() as usize != width
        }) {
            return Ok(None);
        }
        check(&self.atoms.control, control)?;
        self.packed_emissions
            .prepare(graph.closed_rows(), retained.input().action_labels.len());
        let stride = self.packed_emissions.stride;
        if stride == 0 {
            return Ok(None);
        }
        let base = first / 64 * stride;
        let block = &mut self.packed_emissions.words[base..base + stride];
        if block[0] == 0 {
            block[0] = if fill_word_block(
                &mut self.proofs,
                &self.atoms.machine.heap,
                graph,
                first,
                block,
                &self.atoms.control,
                control,
            )? {
                1
            } else {
                2
            };
        }
        if block[0] != 1 {
            return Ok(None);
        }
        let present = block[1 + 2 * branch];
        let mut word = if cert.subscript.is_some() {
            block[2 + 2 * branch]
        } else {
            present
        };
        let present_count = u64::from(present.count_ones());
        check(&self.atoms.control, control)?;
        if negate {
            word = !word;
        }
        if self.atoms.audit {
            for offset in 0..64 {
                let before = graph
                    .representative_row(first + offset)
                    .ok_or(AnalysisError::Inconsistent("enabled word representative"))?;
                self.audit(atom, before, None, word & (1 << offset) != 0, control)?;
            }
        }
        self.atoms.emission_hits[0] += 64 - present_count;
        self.atoms.emission_hits[1] += present_count;
        Ok(Some(word))
    }
}

/// Compute all branch columns at this aligned block once. A missing producer
/// label or unmaterialized value leaves the whole block on ordinary evaluation.
fn fill_word_block(
    proofs: &mut super::super::RowProofs<'_>,
    heap: &dive_core::Heap,
    graph: &CsrCapture<'_>,
    first: usize,
    block: &mut [u64],
    owner: &EvaluationControl,
    control: &EvaluationControl,
) -> Result<bool, AnalysisError> {
    let retained = proofs.graph;
    let all_closed = proofs.metrics.certified == graph.retained_rows() as u64;
    for offset in 0..64 {
        let source = first + offset;
        let before = graph
            .representative_row(source)
            .ok_or(AnalysisError::Inconsistent("enabled word representative"))?;
        if !all_closed && !proofs.closed(heap, before)? {
            return Ok(false);
        }
        let bit = 1 << offset;
        let src = retained.row(before)?;
        let count = graph
            .original_count(source)
            .ok_or(AnalysisError::Inconsistent("enabled word source"))?;
        for edge in 0..count {
            if edge.is_multiple_of(1024) {
                check(owner, control)?;
            }
            let Some(action) = graph.original_action(source, edge) else {
                return Ok(false);
            };
            let i = 1 + 2 * action as usize;
            let Some(pair) = block.get_mut(i..i + 2) else {
                return Ok(false);
            };
            pair[0] |= bit;
            if pair[1] & bit != 0 {
                continue;
            }
            let after = graph
                .original_row(source, edge)
                .ok_or(AnalysisError::Inconsistent("enabled word original"))?;
            if !all_closed && !proofs.closed(heap, after)? {
                return Ok(false);
            }
            if before != after
                && src
                    .iter()
                    .zip(retained.row(after)?)
                    .any(|(&a, &b)| a != b && !heap.eq(a, b))
            {
                pair[1] |= bit;
            }
        }
    }
    Ok(true)
}
