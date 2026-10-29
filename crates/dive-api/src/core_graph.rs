//! The core's behavior graph in the reference engine's form
//! (`docs/developer/core/11-liveness.md` §3–§4).
//!
//! [`Graph`] receives the explorer's graph events ([`GraphSink`]), converts
//! each new state's handles to reference values and builds the
//! `BehaviorGraph` with the reference capture's own collector
//! ([`GraphBuilder`]), so its limits, messages and layout are the
//! reference's. Compound values are converted once per handle: equal
//! persistent handles are equal values, so the memo shares the converted
//! value between states.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dive_check::Verdict;
use dive_check::capture::{BehaviorGraph, GraphBuilder, Limits};
use dive_core::{Handle, Heap};
use dive_eval::next::StateVals;
use dive_explore::graph::{GraphSink, Step, Stopped};
use dive_value::Value;

/// What a completed level asks of the search: called with the level, the
/// graph so far and the counts; returns whether the search stops there.
pub(crate) type OnLevel<'a> = dyn FnMut(u64, &BehaviorGraph, dive_check::Stats) -> bool + 'a;

/// Conversion metrics (`11-liveness.md` §4).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GraphMetrics {
    /// Time spent converting handles to values.
    pub(crate) convert: Duration,
    /// Handles converted (memo misses).
    pub(crate) converted: u64,
    /// Entries in the conversion memo at the end.
    pub(crate) memo_entries: usize,
}

/// Builds the reference `BehaviorGraph` from the core's graph events.
pub(crate) struct Graph<'a> {
    model: &'a dive_model::Model,
    ctx: dive_value::Ctx,
    builder: GraphBuilder,
    limits: Limits,
    memo: HashMap<Handle, Value>,
    on_level: Option<&'a mut OnLevel<'a>>,
    /// Why the sink stopped the search.
    error: Option<Verdict>,
    metrics: GraphMetrics,
}

impl<'a> Graph<'a> {
    /// An empty graph of `model`'s states with the capture limits `limits`.
    pub(crate) fn new(
        model: &'a dive_model::Model,
        limits: Limits,
        on_level: Option<&'a mut OnLevel<'a>>,
    ) -> Self {
        Self {
            model,
            ctx: model.evaluator.options().ctx,
            builder: GraphBuilder::new(limits),
            limits,
            memo: HashMap::new(),
            on_level,
            error: None,
            metrics: GraphMetrics::default(),
        }
    }

    /// Why the sink stopped the search, if it did.
    pub(crate) fn take_error(&mut self) -> Option<Verdict> {
        self.error.take()
    }

    /// The graph so far, its builder and the conversion metrics.
    pub(crate) fn finish(self) -> (GraphBuilder, GraphMetrics) {
        let mut metrics = self.metrics;
        metrics.memo_entries = self.memo.len();
        (self.builder, metrics)
    }

    fn stop(&mut self, v: Verdict) -> Stopped {
        self.error.get_or_insert(v);
        Stopped
    }

    fn convert(&mut self, heap: &mut Heap, values: &[Handle]) -> Result<StateVals, String> {
        let started = Instant::now();
        let mut out = Vec::with_capacity(values.len());
        for &h in values {
            let scalar = h.as_bool().is_some()
                || h.as_int().is_some()
                || h.as_str_id().is_some()
                || h.as_model_id().is_some();
            let v = if let Some(v) = self.memo.get(&h) {
                v.clone()
            } else {
                let v = dive_core::print::to_value(
                    heap,
                    &self.model.names,
                    h,
                    self.ctx,
                    dive_core::print::Conversion::Graph,
                )?;
                self.metrics.converted += 1;
                if !scalar
                    && self.memo.len()
                        < self
                            .limits
                            .states
                            .saturating_mul(self.model.ir.vars().len())
                {
                    self.memo.insert(h, v.clone());
                }
                v
            };
            out.push(Some(v));
        }
        self.metrics.convert += started.elapsed();
        Ok(out.into())
    }
}

fn to_check(s: dive_explore::Stats) -> dive_check::Stats {
    dive_check::Stats {
        generated: s.generated,
        distinct: s.distinct,
        left_on_queue: s.left_on_queue,
        depth: s.depth,
        initial_generated: s.initial_generated,
    }
}

impl GraphSink for Graph<'_> {
    fn max_expansion_units(&self) -> usize {
        dive_check::capture::MAX_EXPANSION_UNITS
    }

    fn max_successors_per_unit(&self) -> u64 {
        self.limits.successors_per_unit
    }

    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        self.builder
            .allow_emitted(generated, ordinal)
            .map_err(|v| self.stop(v))
    }

    fn state(&mut self, heap: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        let vals = self.convert(heap, step.original).map_err(|message| {
            self.stop(Verdict::Unsupported {
                feature: format!(
                    "a state value outside the reference values (core liveness): {message}"
                ),
            })
        })?;
        let id = step.new.map(|(id, _)| id);
        let label = match step.action {
            Some(i) => match self.model.actions.get(i) {
                Some(a) => a.label(),
                None => {
                    return Err(self.stop(Verdict::InternalError {
                        message: format!("core graph: action {i} out of range"),
                    }));
                }
            },
            None => "",
        };
        self.builder
            .record(step.key, vals, id, step.parent, step.ordinal, label)
            .map_err(|v| self.stop(v))
    }

    fn stutter(&mut self, source: u64) -> Result<(), Stopped> {
        self.builder.stutter(source).map_err(|v| self.stop(v))
    }

    fn level_done(&mut self, level: u64, stats: dive_explore::Stats) -> Result<bool, Stopped> {
        let Some(on_level) = self.on_level.as_deref_mut() else {
            return Ok(false);
        };
        Ok(on_level(level, self.builder.graph(), to_check(stats)))
    }
}

#[cfg(test)]
#[path = "core_graph_tests.rs"]
mod tests;
