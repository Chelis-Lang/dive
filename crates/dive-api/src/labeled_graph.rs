//! Export complete concrete Core paths with their original action bindings.

use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dive_core::{Handle, Heap, Opts};
use dive_explore::graph::{GraphSink, Step, Stopped};
use serde_json::{Map, Value, json};

/// Limits and output projection for one complete concrete graph capture.
#[derive(Clone, Debug)]
pub struct Request {
    /// Module lookup before the bundled standard library.
    pub library_paths: Vec<PathBuf>,
    /// Variables to write with each state. An empty list chooses all variables.
    pub fields: Vec<String>,
    /// Wall-time budget observed at Core capture checkpoints.
    pub time_limit: std::time::Duration,
    /// Maximum distinct accepted states, including Init.
    pub max_states: u64,
    /// Maximum counted successor occurrences, including Init.
    pub max_generated: u64,
    /// Number of Core search workers requested.
    pub workers: usize,
}

/// Write the graph to a new directory and return its completion metadata.
///
/// Every path uses concrete discovery IDs and named source parameter values.
/// When the CFG includes symmetry, the concrete graph is still explored and
/// the capture separately records an actual reachable member of each class.
///
/// # Errors
/// Refuses a non-finite or effectful graph, an incomplete safety run, an
/// unknown output projection, and any resource or write failure.
pub fn export(
    module: &Path,
    config: &Path,
    output: &Path,
    request: &Request,
) -> Result<Value, String> {
    let started = Instant::now();
    let analysis = crate::analyze_file(
        module,
        &crate::AnalyzeOptions {
            library_paths: request.library_paths.clone(),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let parsed = crate::parse_config_file(config).map_err(|e| e.to_string())?;
    let cfg = parsed.config().ok_or("configuration rejected")?;
    if !analysis.accepted() || cfg.view.is_some() || !cfg.properties.is_empty() {
        return Err("graph capture requires an accepted source without VIEW or PROPERTY".into());
    }
    let ir = Arc::new(dive_ir::lower(analysis.analysis()).map_err(|e| format!("lowering: {e:?}"))?);
    let options = dive_runtime::CheckOptions::dive(0);
    let opts = Opts::default();
    let mut prepared = crate::core_engine::prepare(&ir, cfg, &options, false, opts, false)
        .map_err(|e| format!("binding: {:?}", e.verdict))?;
    if prepared.plan.effects.any || prepared.plan.effects.level || prepared.plan.vars.is_empty() {
        return Err("labeled graph capture requires a pure stateful model".into());
    }
    let fields = if request.fields.is_empty() {
        (0..prepared.plan.vars.len()).collect()
    } else {
        request
            .fields
            .iter()
            .map(|name| {
                prepared
                    .plan
                    .vars
                    .iter()
                    .position(|v| v == name)
                    .ok_or_else(|| format!("unknown graph output variable {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let symmetry = prepared.plan.symmetry.take();
    if cfg.symmetry.is_some() && symmetry.is_none() {
        return Err("configured symmetry failed admission".into());
    }
    let actions = action_labels(&prepared.model)?;
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut writer = Writer {
        model: &prepared.model,
        fields,
        actions,
        states: Array::new(&output.join("states.json"), "states")?,
        edges: Array::new(&output.join("edges.json"), "edges")?,
        representatives: Array::new(&output.join("representatives.json"), "states")?,
        symmetry,
        classes: HashSet::new(),
        roots: Vec::new(),
        limits: (request.max_states, request.max_generated),
        count: 0,
        error: None,
    };
    // Explore concrete states even when the CFG supplies a class map. The
    // representative writer never feeds quotient states into exploration.
    let result = dive_explore::explore_with_storage(
        &prepared.plan,
        &prepared.store,
        dive_explore::Config {
            opts,
            workers: request.workers,
            warn: true,
            max_states: None,
            max_depth: None,
            sample: None,
        },
        &|| started.elapsed() >= request.time_limit,
        Some(&mut writer),
        None,
        &dive_explore::storage::Storage {
            exact: true,
            ..Default::default()
        },
    );
    if let Some(error) = writer.error {
        return Err(error);
    }
    if result.verdict != dive_explore::Verdict::Success || result.stats.left_on_queue != 0 {
        return Err(format!("incomplete labeled graph: {:?}", result.verdict));
    }
    writer.states.finish()?;
    writer.edges.finish()?;
    writer.representatives.finish()?;
    let report = json!({"format":"dive-labeled-concrete-graph-v1", "roots":writer.roots,
        "states":result.stats.distinct, "generated":result.stats.generated,
        "edges":writer.edges.written, "classes":writer.representatives.written, "depth":result.stats.depth,
        "requested_workers":request.workers, "seconds":started.elapsed().as_secs_f64()});
    let summary = serde_json::to_vec(&report).map_err(|e| e.to_string())?;
    std::fs::write(output.join("complete.json"), summary).map_err(|e| e.to_string())?;
    Ok(report)
}

fn action_labels(model: &dive_model::Model) -> Result<Vec<(String, Value)>, String> {
    let ctx = model.evaluator.options().ctx;
    model
        .actions
        .iter()
        .map(|action| {
            let name = crate::core_engine::action_name(model, action)
                .unwrap_or_else(|| action.label().to_owned());
            let params = action
                .params()
                .iter()
                .map(|(key, val)| Ok((key.clone(), dive_trace::value_json::to_json(val, ctx)?)))
                .collect::<Result<Map<_, _>, String>>()?;
            Ok((name, Value::Object(params)))
        })
        .collect()
}

struct Array {
    file: BufWriter<std::fs::File>,
    written: u64,
}

impl Array {
    fn new(path: &Path, key: &str) -> Result<Self, String> {
        let f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let mut file = BufWriter::with_capacity(1 << 20, f);
        write!(file, "{{\"{key}\":[").map_err(|e| e.to_string())?;
        Ok(Self { file, written: 0 })
    }

    fn push(&mut self, value: &Value) -> Result<(), String> {
        if self.written > 0 {
            self.file.write_all(b",").map_err(|e| e.to_string())?;
        }
        serde_json::to_writer(&mut self.file, value).map_err(|e| e.to_string())?;
        self.written += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        self.file
            .write_all(b"]}\n")
            .and_then(|()| self.file.flush())
            .map_err(|e| e.to_string())
    }
}

struct Writer<'a> {
    model: &'a dive_model::Model,
    fields: Vec<usize>,
    actions: Vec<(String, Value)>,
    states: Array,
    edges: Array,
    representatives: Array,
    symmetry: Option<dive_core::symmetry::Symmetry>,
    classes: HashSet<Vec<u8>>,
    roots: Vec<u64>,
    limits: (u64, u64),
    count: u64,
    error: Option<String>,
}

impl Writer<'_> {
    fn value(&self, heap: &mut Heap, h: Handle) -> Result<Value, String> {
        let ctx = self.model.evaluator.options().ctx;
        let val = crate::core_engine::to_value(heap, self.model, h, ctx)?;
        dive_trace::value_json::to_json(&val, ctx)
    }

    fn add(&mut self, heap: &mut Heap, step: Step<'_>) -> Result<(), String> {
        let id = if let Some((id, identity_row)) = step.new {
            if self.count == self.limits.0 {
                return Err("labeled graph state limit".into());
            }
            self.count += 1;
            let mut data = Map::new();
            for &field in &self.fields {
                let decl = self.model.ir.decl(self.model.ir.vars()[field]);
                data.insert(decl.name.clone(), self.value(heap, step.original[field])?);
            }
            self.states
                .push(&json!({"fp":id,"val":data,"initial":step.parent.is_none()}))?;
            let new_class = if let Some(symmetry) = &self.symmetry {
                let mut canonical = Vec::new();
                symmetry
                    .canonical(heap, identity_row, &mut canonical)
                    .map_err(|e| e.to_string())?;
                for handle in &mut canonical {
                    *handle = heap.intern(*handle).map_err(|e| e.to_string())?;
                }
                let key = dive_core::key::canonical(heap, &canonical, usize::MAX, &|| true)
                    .map_err(|e| format!("canonical symmetry key: {e:?}"))?;
                self.classes.insert(key)
            } else {
                true
            };
            if new_class {
                self.representatives.push(&json!({"fp":id}))?;
            }
            id
        } else {
            step.seen
                .ok_or("exact concrete state has no discovery ID")?
        };
        if let Some(parent) = step.parent {
            let (action, params) = step
                .action
                .and_then(|i| self.actions.get(i))
                .ok_or("concrete transition has no original action")?;
            self.edges
                .push(&json!({"from":parent,"to":id,"act":action,"params":params,
                "ordinal":step.ordinal}))?;
        } else if step.new.is_some() {
            self.roots.push(id);
        }
        Ok(())
    }
}

impl GraphSink for Writer<'_> {
    fn max_expansion_units(&self) -> usize {
        8
    }
    fn max_successors_per_unit(&self) -> u64 {
        self.limits.1
    }

    fn emitted(&mut self, generated: u64, _: Option<u64>) -> Result<(), Stopped> {
        if generated > self.limits.1 {
            self.error = Some("labeled graph generated limit".into());
            return Err(Stopped);
        }
        Ok(())
    }

    fn state(&mut self, heap: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        self.add(heap, step).map_err(|error| {
            self.error = Some(error);
            Stopped
        })
    }

    fn stutter(&mut self, _: u64) -> Result<(), Stopped> {
        Ok(())
    }

    fn level_done(&mut self, _: u64, _: dive_explore::Stats) -> Result<bool, Stopped> {
        Ok(false)
    }
}
