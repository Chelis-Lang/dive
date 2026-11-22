//! Certify label swaps against every vertex and edge of one completed, reachable safety graph.

use crate::{AnalyzeOptions, core_engine};
use dive_core::{Handle, Heap, Opts, symmetry::Symmetry};
use dive_explore::graph::{GraphSink, SourceRow, Step, Stopped};
use std::{collections::HashMap, path::Path, sync::Arc};

#[derive(Default)]
struct Graph {
    rows: Vec<Handle>,
    initial: Vec<u32>,
    targets: Vec<u32>,
    ends: Vec<usize>,
    width: usize,
    emitted: u64,
    completed: usize,
}

impl GraphSink for Graph {
    fn max_expansion_units(&self) -> usize {
        1024
    }
    fn max_successors_per_unit(&self) -> u64 {
        u64::from(u32::MAX)
    }

    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        if ordinal.is_some_and(|i| i >= self.max_successors_per_unit())
            || generated != self.emitted + 1
        {
            return Err(Stopped);
        }
        self.emitted = generated;
        Ok(())
    }

    fn state(&mut self, _: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        let id = match (step.new, step.seen) {
            (Some((id, row)), None)
                if row.len() == self.width
                    && usize::try_from(id).ok() == Some(self.rows.len() / self.width) =>
            {
                self.rows.extend_from_slice(row);
                id
            }
            (None, Some(id))
                if usize::try_from(id).is_ok_and(|i| i < self.rows.len() / self.width) =>
            {
                id
            }
            _ => return Err(Stopped),
        };
        let target = u32::try_from(id).map_err(|_| Stopped)?;
        if let Some(parent) = step.parent {
            if usize::try_from(parent).ok() != Some(self.ends.len()) {
                return Err(Stopped);
            }
            self.targets.push(target);
        } else {
            if !self.ends.is_empty() || !self.targets.is_empty() {
                return Err(Stopped);
            }
            self.initial.push(target);
        }
        Ok(())
    }

    fn stutter(&mut self, source: u64) -> Result<(), Stopped> {
        if usize::try_from(source).ok() != Some(self.ends.len()) {
            return Err(Stopped);
        }
        self.ends.push(self.targets.len());
        Ok(())
    }

    fn source_row(&mut self, _: &mut Heap, row: SourceRow<'_>) -> Result<(), Stopped> {
        let start = self.ends.iter().rev().nth(1).copied().unwrap_or(0);
        if row.next_error.is_some()
            || self.completed + 1 != self.ends.len()
            || usize::try_from(row.source).ok() != Some(self.completed)
            || usize::try_from(row.emitted).ok() != Some(self.targets.len() - start)
        {
            return Err(Stopped);
        }
        self.completed += 1;
        Ok(())
    }

    fn level_done(&mut self, _: u64, _: dive_explore::Stats) -> Result<bool, Stopped> {
        Ok(false)
    }
}

impl Graph {
    fn edges(&self, i: usize) -> &[u32] {
        let start = i.checked_sub(1).map_or(0, |prev| self.ends[prev]);
        &self.targets[start..self.ends[i]]
    }

    fn verify(&mut self, heap: &mut Heap, sym: &Symmetry) -> Result<(), String> {
        let index: HashMap<_, _> = self
            .rows
            .chunks_exact(self.width)
            .enumerate()
            .map(|(i, row)| u32::try_from(i).map(|id| (row, id)))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        if index.len() != self.completed {
            return Err("incomplete exact state identities".into());
        }
        let mut start = 0;
        for &end in &self.ends {
            self.targets[start..end].sort_unstable();
            start = end;
        }
        self.initial.sort_unstable();
        let mut image = Vec::with_capacity(self.width);
        let mut mapped = Vec::new();
        for generator in 0..sym.len() {
            let mut swap = Vec::with_capacity(index.len());
            for row in self.rows.chunks_exact(self.width) {
                sym.image_row(heap, generator, row, &mut image)
                    .map_err(|e| e.message().to_owned())?;
                swap.push(
                    *index
                        .get(image.as_slice())
                        .ok_or("a generator leaves the reachable graph")?,
                );
                heap.reset();
            }
            if swap
                .iter()
                .enumerate()
                .any(|(i, &v)| swap[v as usize] as usize != i)
            {
                return Err("generator does not act bijectively on exact identities".into());
            }
            mapped.clear();
            mapped.extend(self.initial.iter().map(|&v| swap[v as usize]));
            mapped.sort_unstable();
            if self.initial != mapped {
                return Err("asymmetric initial occurrence multiset".into());
            }
            for (i, &v) in swap.iter().enumerate() {
                mapped.clear();
                mapped.extend(self.edges(i).iter().map(|&dest| swap[dest as usize]));
                mapped.sort_unstable();
                if mapped != self.edges(v as usize) {
                    return Err("asymmetric successor occurrence multiset".into());
                }
            }
        }
        Ok(())
    }
}

fn is_carrier(entry: &dive_config::ConstantEntry, atoms: &[String]) -> bool {
    let dive_config::ConstantEntry::Assignment {
        args,
        module: None,
        value,
        ..
    } = entry
    else {
        return false;
    };
    let dive_config::ConfigValue::Set(elements) = &value.value else {
        return false;
    };
    args.is_empty()
        && elements.len() == atoms.len()
        && atoms.iter().all(|name| {
            elements.iter().any(|member| {
            member.value.model_value_type().is_none() &&
                matches!(&member.value, dive_config::ConfigValue::ModelValue(atom) if atom == name)
        })
        })
}

fn prepare(
    file: &Path,
    config: &Path,
    atoms: &[String],
    workers: usize,
) -> Result<(core_engine::Prepared, Symmetry), String> {
    let analysis =
        crate::analyze_file(file, &AnalyzeOptions::default()).map_err(|e| e.to_string())?;
    let parsed = crate::parse_config_file(config).map_err(|e| e.to_string())?;
    if !analysis.accepted() || !parsed.parse().diagnostics().is_empty() {
        return Err("source or CFG did not pass analysis".into());
    }
    let cfg = parsed.config().ok_or("invalid CFG")?;
    if !cfg.constants.iter().any(|entry| is_carrier(entry, atoms)) {
        return Err(
            "carrier must be a configured literal set of these untyped model values".into(),
        );
    }
    let ir = Arc::new(dive_ir::lower(analysis.analysis()).map_err(|e| format!("lowering: {e:?}"))?);
    if let Some(reason) = core_engine::edit_input_refusal(&ir, cfg) {
        return Err(reason.into());
    }
    let mut options = dive_runtime::CheckOptions::tlc(0, 0);
    options.store = dive_store::StoreMode::Exact;
    options.workers = workers;
    let opts = Opts::default();
    let prepared = core_engine::prepare(&ir, cfg, &options, true, opts, false)
        .map_err(|r| format!("original program could not be prepared: {:?}", r.verdict))?;
    let plan = &prepared.plan;
    if prepared.start.has_diagnostics()
        || plan.vars.is_empty()
        || plan.effects.any
        || plan.effects.level
        || plan.symmetry.is_some()
        || plan.view.is_some()
        || !plan.constraints.is_empty()
        || !plan.action_constraints.is_empty()
        || prepared.model.stateful_checking().is_some()
        || prepared.model.runtime_name_order
        || !prepared.model.temporals.is_empty()
        || !prepared.model.implied_temporals.is_empty()
        || !plan.implied_inits.is_empty()
        || cfg.periodic.is_some()
        || !cfg.postconditions.is_empty()
        || !cfg.possible.is_empty()
        || cfg.alias.is_some()
        || cfg.rl_reward.is_some()
    {
        return Err("reachable orbit proof requires pure unconstrained original safety with no configured group or extra roles".into());
    }
    let ids: Vec<_> = atoms
        .iter()
        .map(|a| {
            prepared
                .store
                .strings()
                .lookup(a)
                .ok_or("unknown carrier atom")
        })
        .collect::<Result<_, _>>()?;
    let swaps = Symmetry::new(
        ids.windows(2)
            .map(|w| vec![(w[0], w[1]), (w[1], w[0])])
            .collect(),
    );
    Ok((prepared, swaps))
}

/// Check adjacent transpositions of the named untyped model values on the exact reachable graph.
/// Every configured invariant and step predicate must hold on the complete original capture.
/// No state or transition constraints, prior reduction, liveness or model effects are admitted.
///
/// # Errors
/// The inputs cannot be bound, the search did not succeed completely, or a swap failed.
pub fn certify(
    file: &Path,
    config: &Path,
    atoms: &[String],
    workers: usize,
) -> Result<serde_json::Value, String> {
    if !(2..=4).contains(&atoms.len())
        || !atoms.windows(2).all(|p| p[0] < p[1])
        || !(1..=256).contains(&workers)
    {
        return Err(
            "expected two to four sorted distinct carrier atoms and 1 to 256 workers".into(),
        );
    }
    let (prepared, swaps) = prepare(file, config, atoms, workers)?;
    let plan = &prepared.plan;
    let search = dive_explore::Config {
        opts: Opts::default(),
        workers,
        warn: prepared.model.evaluator.options().warnings,
        max_states: Some(u64::from(u32::MAX)),
        max_depth: None,
        sample: None,
    };
    let storage = dive_explore::storage::Storage {
        exact: true,
        ..Default::default()
    };
    let mut graph = Graph {
        width: plan.vars.len(),
        ..Graph::default()
    };
    let outcome = dive_explore::explore_with_storage(
        plan,
        &prepared.store,
        search,
        &|| false,
        Some(&mut graph),
        None,
        &storage,
    );
    let stats = outcome.stats;
    if outcome.verdict != dive_explore::Verdict::Success
        || stats.left_on_queue != 0
        || stats.distinct != graph.completed as u64
        || stats.initial_generated != graph.initial.len() as u64
        || stats.generated != graph.emitted
        || stats.generated != (graph.initial.len() + graph.targets.len()) as u64
    {
        return Err(format!(
            "incomplete original safety graph: {:?}, {} captured, {} completed, {} generated",
            outcome.verdict, graph.emitted, graph.completed, stats.generated
        ));
    }
    graph.verify(&mut Heap::new(prepared.store, true, true), &swaps)?;
    Ok(
        serde_json::json!({"method":"reachable-graph","distinct":stats.distinct,"generated":stats.generated,
        "depth":stats.depth,"initial_generated":stats.initial_generated,"generators":swaps.len(),"edges":graph.targets.len()}),
    )
}
