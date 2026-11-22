//! Complete deterministic action images on one closed source's physical edges.

use super::{AnalysisError, CsrCapture, EvaluationControl, PredicateAtom, Reader, check};

pub(in crate::core_native_atoms) struct Images {
    pub(in crate::core_native_atoms) enabled: bool,
    rows: u64,
    edges: u64,
    refused_rows: u64,
    refused_edges: u64,
    high_fanout: u64,
    cached_guards: u64,
}

impl Images {
    pub(in crate::core_native_atoms) fn new() -> Self {
        Self {
            enabled: std::env::var_os("DIVE_EXPERIMENT_PROJECTED_ACTION_ROWS")
                .is_some_and(|s| s == "1"),
            rows: 0,
            edges: 0,
            refused_rows: 0,
            refused_edges: 0,
            high_fanout: 0,
            cached_guards: 0,
        }
    }

    pub(in crate::core_native_atoms) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled, "rows": self.rows, "edges": self.edges,
            "refused_rows": self.refused_rows, "refused_edges": self.refused_edges,
            "high_fanout": self.high_fanout, "cached_guards": self.cached_guards,
        })
    }
}

impl Reader<'_, '_> {
    #[allow(
        clippy::too_many_lines,
        reason = "keep capture, guard, target, and audit checks in the same row transaction"
    )]
    pub(in crate::core_native_atoms) fn projected_image_row(
        &mut self,
        atom: &PredicateAtom,
        graph: &CsrCapture<'_>,
        source: usize,
        words: &mut [u64],
        control: &EvaluationControl,
    ) -> Result<Option<usize>, AnalysisError> {
        let retained = self.proofs.graph;
        if !self.atoms.projected_images.enabled
            || !self.atoms.live_emissions
            || self.prefix.is_none()
            || source >= graph.closed_rows()
            || self.atoms.fuel != super::super::NATIVE_FUEL
            || self.atoms.machine.heap.poisoned()
            || self.proofs.metrics.certified != graph.retained_rows() as u64
            || !retained.owns_store(self.atoms.machine.heap.store())
            || !self
                .emission_capture
                .as_ref()
                .is_some_and(|c| c.same_validation(graph))
        {
            return Ok(None);
        }
        let Some(compiled) = self.atoms.position(atom).map(|i| &self.atoms.compiled[i]) else {
            return Ok(None);
        };
        let (Some(id), Some(batch)) = (compiled.producer, &compiled.batch) else {
            return Ok(None);
        };
        let Some(cert) = self.atoms.producer_images.get(id).copied().flatten() else {
            return Ok(None);
        };
        if !batch.admits_projected_image()
            || (!cert.boolean_guard_prefix && compiled.image_guard_key.is_none())
            || compiled.exhausted_in_a_row != 0
            || ((!cert.quiet_guards || !cert.quiet_suffix)
                && self.atoms.machine.heap.store().saw_missing_update())
        {
            return Ok(None);
        }
        let range = graph
            .outgoing_range(source)
            .ok_or(AnalysisError::Inconsistent("projected image source edges"))?;
        let length = range.len();
        if length <= 1 {
            return Ok(None);
        }
        let count = length.div_ceil(64);
        let metrics = &mut self.atoms.projected_images;
        if count > words.len() {
            metrics.refused_rows += 1;
            metrics.refused_edges += length as u64;
            metrics.high_fanout += 1;
            return Ok(None);
        }
        let pending = &mut words[..count];
        pending.fill(0);
        let before = graph
            .representative_row(source)
            .ok_or(AnalysisError::Inconsistent("projected image source"))?;
        let src = retained.row(before)?;
        check(&self.atoms.control, control)?;
        let classified = (|| {
            let Some(mask) = self
                .emissions
                .mask(graph, source, &self.atoms.control, control)?
            else {
                return Ok(false);
            };
            let present = mask & (1u64 << id) != 0;
            if !cert.boolean_guard_prefix {
                let key = compiled
                    .image_guard_key
                    .filter(|key| self.atoms.projected.active(key.site))
                    .and_then(|site| site.bind(src));
                let guard = if let Some(value) = key
                    .as_ref()
                    .and_then(|key| key.get(&mut self.atoms.projected))
                {
                    metrics.cached_guards += 1;
                    value
                } else {
                    let Some(value) = batch.original_boolean_prefix(
                        &compiled.plan,
                        &mut self.atoms.machine,
                        src,
                        self.atoms.fuel,
                    ) else {
                        return Ok(false);
                    };
                    if let Some(key) = key {
                        key.put(&mut self.atoms.projected, value);
                    }
                    value
                };
                if !guard {
                    return Ok(true);
                }
                if !present {
                    return Ok(false);
                }
            }
            if !present {
                // The ordered generator prefix exactly refutes the Boolean
                // action; its tuple subscript cannot diagnose on these rows.
                return Ok(true);
            }
            let action = u32::try_from(id).ok();
            let Some(position) =
                (0..length - 1).find(|&p| graph.original_action(source, p) == action)
            else {
                return Ok(false);
            };
            let image = retained.row(
                graph
                    .original_row(source, position)
                    .ok_or(AnalysisError::Inconsistent("projected image row"))?,
            )?;
            for p in 0..length - 1 {
                let original = graph
                    .original_row(source, p)
                    .ok_or(AnalysisError::Inconsistent("projected image target"))?;
                if graph
                    .target(range.start + p)
                    .and_then(|dst| graph.representative_row(dst))
                    != Some(original)
                {
                    return Ok(false);
                }
                let Some(truth) = batch.original_image_target(
                    &self.atoms.machine.heap,
                    src,
                    image,
                    retained.row(original)?,
                ) else {
                    return Ok(false);
                };
                if truth {
                    pending[p / 64] |= 1 << (p % 64);
                }
            }
            Ok(true)
        })()?;
        check(&self.atoms.control, control)?;
        if !classified {
            metrics.refused_rows += 1;
            metrics.refused_edges += length as u64;
            return Ok(None);
        }
        metrics.rows += 1;
        metrics.edges += length as u64;
        if self.atoms.audit {
            for p in 0..length {
                let after = if p + 1 == length {
                    before
                } else {
                    graph
                        .original_row(source, p)
                        .ok_or(AnalysisError::Inconsistent("projected image audit target"))?
                };
                self.audit(
                    atom,
                    before,
                    Some(after),
                    pending[p / 64] & (1 << (p % 64)) != 0,
                    control,
                )?;
            }
        }
        Ok(Some(count))
    }
}
