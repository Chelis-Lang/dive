//! Complete deterministic nonstuttering images on original physical transitions.
use super::{AnalysisError, CsrCapture, EvaluationControl, PredicateAtom, Reader, check};
use dive_liveness::formula::AtomKind;

impl Reader<'_, '_> {
    pub(crate) fn original_image_column(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        control: &EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        if atom.kind != AtomKind::Action
            || !self.atoms.live_emissions
            || self.prefix.is_none()
            || !self
                .emission_capture
                .as_ref()
                .is_some_and(|c| c.same_validation(graph))
            || self.atoms.fuel != super::super::NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || !self
                .proofs
                .graph
                .owns_store(self.atoms.machine.heap.store())
        {
            return Ok(false);
        }
        let Some(index) = self.atoms.position(atom) else {
            return Ok(false);
        };
        let compiled = &self.atoms.compiled[index];
        let Some(id) = compiled.producer else {
            return Ok(false);
        };
        let Some(cert) = self.atoms.producer_images.get(id).copied().flatten() else {
            return Ok(false);
        };
        if (!cert.boolean_guard_prefix && compiled.image_guard_key.is_none())
            || compiled.exhausted_in_a_row != 0
            || ((!cert.quiet_guards || !cert.quiet_suffix)
                && self.atoms.machine.heap.store().saw_missing_update())
        {
            return Ok(false);
        }
        let retained = self.proofs.graph;
        if self.proofs.metrics.certified != graph.retained_rows() as u64 {
            for row in 0..graph.retained_rows() {
                if row.is_multiple_of(1024) {
                    check(&self.atoms.control, control)?;
                }
                if !self.proofs.closed(&self.atoms.machine.heap, row)? {
                    return Ok(false);
                }
            }
        }
        let Some(first) = self.prepare_image_words(atom, graph, control)? else {
            return Ok(false);
        };
        let compiled = &self.atoms.compiled[index];
        let Some(batch) = compiled.batch.as_ref() else {
            return Ok(false);
        };
        let action = u32::try_from(id).ok();
        let mut positive = 0;
        for source in first..graph.closed_rows() {
            if source.is_multiple_of(256) {
                check(&self.atoms.control, control)?;
            }
            let Some(mask) = self
                .emissions
                .mask(graph, source, &self.atoms.control, control)?
            else {
                return Ok(false);
            };
            let present = mask & (1u64 << id) != 0;
            let before = graph
                .representative_row(source)
                .ok_or(AnalysisError::Inconsistent("image source"))?;
            let src = retained.row(before)?;
            if !cert.boolean_guard_prefix {
                let key = compiled
                    .image_guard_key
                    .filter(|key| self.atoms.projected.active(key.site))
                    .and_then(|site| site.bind(src));
                let value = if let Some(cached) = key
                    .as_ref()
                    .and_then(|key| key.get(&mut self.atoms.projected))
                {
                    self.atoms.direct_guard_cached += 1;
                    cached
                } else {
                    let timer = std::time::Instant::now();
                    let outcome = batch.original_boolean_prefix(
                        &compiled.plan,
                        &mut self.atoms.machine,
                        src,
                        self.atoms.fuel,
                    );
                    self.atoms.direct_guard_time += timer.elapsed();
                    let Some(value) = outcome else {
                        return Ok(false);
                    };
                    if let Some(key) = key {
                        key.put(&mut self.atoms.projected, value);
                    }
                    value
                };
                if !value {
                    continue;
                }
                if !present
                    || ((!cert.quiet_guards || !cert.quiet_suffix)
                        && self.atoms.machine.heap.store().saw_missing_update())
                {
                    return Ok(false);
                }
            }
            if !present {
                continue;
            }
            let range = graph
                .outgoing_range(source)
                .ok_or(AnalysisError::Inconsistent("image source edges"))?;
            let n = range.len().saturating_sub(1);
            if n > 1024 {
                return Ok(false);
            }
            let Some(pos) = (0..n).find(|&p| graph.original_action(source, p) == action) else {
                return Ok(false);
            };
            let img = retained.row(
                graph
                    .original_row(source, pos)
                    .ok_or(AnalysisError::Inconsistent("generated image"))?,
            )?;
            for p in 0..n {
                let original = graph
                    .original_row(source, p)
                    .ok_or(AnalysisError::Inconsistent("image target"))?;
                let edge = range.start + p;
                if graph
                    .target(edge)
                    .and_then(|dst| graph.representative_row(dst))
                    != Some(original)
                {
                    return Ok(false);
                }
                let Some(truth) = batch.original_image_target(
                    &self.atoms.machine.heap,
                    src,
                    img,
                    retained.row(original)?,
                ) else {
                    return Ok(false);
                };
                if truth {
                    self.image_words[edge / 64] |= 1u64 << (edge % 64);
                }
            }
            positive += 1;
        }
        if self.atoms.audit {
            for source in 0..graph.closed_rows() {
                let before = graph
                    .representative_row(source)
                    .ok_or(AnalysisError::Inconsistent("image audit source"))?;
                let range = graph
                    .outgoing_range(source)
                    .ok_or(AnalysisError::Inconsistent("image audit edge"))?;
                for (p, edge) in range.clone().enumerate() {
                    let after = if edge + 1 == range.end {
                        before
                    } else {
                        graph
                            .original_row(source, p)
                            .ok_or(AnalysisError::Inconsistent("image audit original"))?
                    };
                    self.audit(
                        atom,
                        before,
                        Some(after),
                        self.image_words[edge / 64] & (1 << (edge % 64)) != 0,
                        control,
                    )?;
                }
            }
        }
        check(&self.atoms.control, control)?;
        self.atoms.image_hits += positive;
        self.atoms.direct_image_columns += 1;
        self.atoms.direct_image_sources += (graph.closed_rows() - first) as u64;
        Ok(true)
    }

    fn prepare_image_words(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        control: &EvaluationControl,
    ) -> Result<Option<usize>, AnalysisError> {
        let edges = graph.offsets().last().copied().unwrap_or(0);
        let needed = edges.div_ceil(64);
        if needed > (8 << 20) / size_of::<u64>() {
            return Ok(None);
        }
        let mut words = std::mem::take(&mut self.image_words);
        words.clear();
        if words.try_reserve_exact(needed).is_err() {
            self.image_words = words;
            return Ok(None);
        }
        words.resize(needed, 0);
        let mut start = 0;
        if let Some(prefix) = self.previous_column(atom, graph, control)?
            && prefix.sources <= graph.closed_rows()
            && graph.offsets().get(prefix.sources) == Some(&prefix.cells)
            && prefix.words.len() == prefix.cells.div_ceil(64)
        {
            words[..prefix.words.len()].copy_from_slice(prefix.words);
            let remainder = prefix.cells % 64;
            if remainder != 0 {
                words[prefix.cells / 64] &= (1 << remainder) - 1;
            }
            start = prefix.sources;
        }
        self.image_words = words;
        Ok(Some(start))
    }
}
