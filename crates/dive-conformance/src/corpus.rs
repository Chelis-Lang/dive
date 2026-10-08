//! Corpus enumeration: modules and models, tiers and scope.
//!
//! For the `examples-manifest` layout every `specifications/*/manifest.json`
//! is read with the schema of tlaplus/Examples' `manifest-schema.json`
//! (unknown fields are an error, so an upstream schema change is noticed).
//! Tiers and scope follow ADR 0005:
//! - *small*: exhaustive search with a recorded runtime of 30 s or less;
//! - *deep*: everything else that is runnable;
//! - `symbolic` (Apalache) and `generate` models are `out-of-scope` with a
//!   reason; they are never dropped silently.
//!
//! Simulation models are in scope but their state counts are not comparable.

use crate::config::{Corpus, Layout};
use crate::error::{Error, Result};
use crate::fetch::submodule_paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Recorded runtimes at or below this many seconds put an exhaustive model
/// in the small tier.
pub const SMALL_TIER_SECONDS: u64 = 30;

/// A manifest file (`manifest-schema.json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Sources.
    pub sources: Vec<String>,
    /// Authors.
    pub authors: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Modules.
    pub modules: Vec<ManifestModule>,
}

/// A module entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestModule {
    /// Corpus-relative path of the `.tla` file.
    pub path: String,
    /// Features (`pluscal`, `action composition`).
    pub features: Vec<String>,
    /// Proof metadata, if the module carries a TLAPS proof.
    #[serde(default)]
    pub proof: Option<serde_json::Value>,
    /// Models of this module.
    pub models: Vec<ManifestModel>,
}

/// A model mode.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ManifestMode {
    /// `"exhaustive search"`, `"generate"` or `"symbolic"`.
    Named(String),
    /// `{"simulate": {"traceCount": n}}`.
    Simulate {
        /// Simulation parameters.
        simulate: SimulateMode,
    },
}

/// Simulation parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulateMode {
    /// Number of traces (the schema types this as `number`).
    #[serde(rename = "traceCount")]
    pub trace_count: f64,
}

/// A model entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestModel {
    /// Corpus-relative path of the `.cfg` file.
    pub path: String,
    /// `HH:MM:SS` or `unknown`.
    pub runtime: String,
    /// Mode.
    pub mode: ManifestMode,
    /// Expected result.
    pub result: String,
    /// Recorded distinct states.
    #[serde(rename = "distinctStates")]
    pub distinct_states: Option<u64>,
    /// Recorded generated states.
    #[serde(rename = "totalStates")]
    pub total_states: Option<u64>,
    /// Recorded depth.
    #[serde(rename = "stateDepth")]
    pub state_depth: Option<u64>,
    /// Worker count the model was recorded with (`auto` or an integer).
    #[serde(default)]
    pub workers: Option<serde_json::Value>,
}

/// The normalised mode of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ModelMode {
    /// Exhaustive breadth-first search.
    Exhaustive,
    /// Simulation with `traces` traces.
    Simulate {
        /// Trace count.
        traces: u64,
    },
    /// TLC `-generate`.
    Generate,
    /// Apalache symbolic checking.
    Symbolic,
}

/// Test tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Standard tier.
    Small,
    /// Deep tier.
    Deep,
}

/// What the manifest records about a model; used for the drift cross-check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExpectation {
    /// `result`.
    pub result: String,
    /// `runtime` in seconds, `None` if `unknown`.
    pub runtime_seconds: Option<u64>,
    /// `distinctStates`.
    pub distinct_states: Option<u64>,
    /// `totalStates`.
    pub total_states: Option<u64>,
    /// `stateDepth`.
    pub state_depth: Option<u64>,
    /// `workers`, verbatim.
    pub workers: Option<String>,
}

/// A model to check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Model id: the corpus-relative `.cfg` path (unique per corpus).
    pub id: String,
    /// Corpus-relative module path.
    pub module: String,
    /// Mode.
    pub mode: ModelMode,
    /// Module features.
    pub features: Vec<String>,
    /// Tier.
    pub tier: Tier,
    /// `Some(reason)` if the model is out of scope.
    pub out_of_scope: Option<String>,
    /// Whether state counts are comparable (false in simulation).
    pub counts_comparable: bool,
    /// Manifest expectations.
    pub expected: ManifestExpectation,
    /// Whether the model's module reads `TLCGet("config")` (worker count
    /// visible to the spec); see [`reads_tlc_config`].
    #[serde(default)]
    pub worker_introspecting: bool,
}

/// A module to parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleEntry {
    /// Corpus-relative path.
    pub path: String,
    /// Features.
    pub features: Vec<String>,
}

/// An enumerated corpus.
#[derive(Debug, Clone)]
pub struct CorpusIndex {
    /// Corpus id.
    pub id: String,
    /// Pinned commit.
    pub commit: String,
    /// Checkout root.
    pub root: PathBuf,
    /// Modules, sorted by path.
    pub modules: Vec<ModuleEntry>,
    /// Models, sorted by id.
    pub models: Vec<ModelEntry>,
    /// Submodules declared but not fetched.
    pub unfetched_submodules: Vec<String>,
    /// Spec directories without a manifest, and `.tla` files that no manifest
    /// lists. Reported as warnings: they are not covered by any golden.
    pub uncovered: Vec<String>,
}

/// The part of a corpus a job on `path` needs, as a corpus-relative
/// directory: `specifications/<spec>` for tlaplus/Examples (specs never
/// reference files outside their own directory), else the file's directory.
#[must_use]
pub fn isolation_dir(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "specifications" {
        return format!("specifications/{}", parts[1]);
    }
    parts[..parts.len().saturating_sub(1)].join("/")
}

/// Whether a module reads TLC's configuration (`TLCGet("config")`), which
/// includes the worker count, so its behaviour may legitimately depend on
/// `-workers` (Examples `MCDieHardest` asserts `TLCGet("config").worker = 1`).
#[must_use]
pub fn reads_tlc_config(module_text: &str) -> bool {
    let squashed: String = module_text.chars().filter(|c| !c.is_whitespace()).collect();
    squashed.contains("TLCGet(\"config\")")
}

/// Parses `HH:MM:SS` (or `unknown`) to seconds.
///
/// # Errors
/// Fails on any other format.
pub fn parse_runtime(s: &str) -> Result<Option<u64>> {
    if s == "unknown" {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split(':').collect();
    if let [h, m, sec] = parts.as_slice()
        && [h, m, sec].iter().all(|p| p.len() == 2)
        && let (Ok(h), Ok(m), Ok(sec)) = (h.parse::<u64>(), m.parse::<u64>(), sec.parse::<u64>())
    {
        return Ok(Some(h * 3600 + m * 60 + sec));
    }
    Err(Error::Config(format!("bad manifest runtime `{s}`")))
}

fn normalise_mode(mode: &ManifestMode) -> Result<ModelMode> {
    match mode {
        ManifestMode::Named(n) => match n.as_str() {
            "exhaustive search" => Ok(ModelMode::Exhaustive),
            "generate" => Ok(ModelMode::Generate),
            "symbolic" => Ok(ModelMode::Symbolic),
            other => Err(Error::Config(format!("unknown manifest mode `{other}`"))),
        },
        ManifestMode::Simulate { simulate } => {
            let n = simulate.trace_count;
            if n.fract() != 0.0 || n < 1.0 || n > 1e12 {
                return Err(Error::Config(format!("bad simulate traceCount {n}")));
            }
            // Checked above: a positive integer well inside u64.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Ok(ModelMode::Simulate { traces: n as u64 })
        }
    }
}

const RESULTS: &[&str] = &[
    "success",
    "assumption failure",
    "deadlock failure",
    "safety failure",
    "liveness failure",
    "unknown",
];

/// Builds the model entries of one manifest.
///
/// # Errors
/// Fails on schema violations.
pub fn models_of(manifest: &Manifest) -> Result<(Vec<ModuleEntry>, Vec<ModelEntry>)> {
    let mut modules = Vec::new();
    let mut models = Vec::new();
    for module in &manifest.modules {
        modules.push(ModuleEntry {
            path: module.path.clone(),
            features: module.features.clone(),
        });
        for m in &module.models {
            if !RESULTS.contains(&m.result.as_str()) {
                return Err(Error::Config(format!(
                    "{}: unknown result `{}`",
                    m.path, m.result
                )));
            }
            let runtime = parse_runtime(&m.runtime)?;
            let mode = normalise_mode(&m.mode)?;
            let tier = match (mode, runtime) {
                (ModelMode::Exhaustive, Some(s)) if s <= SMALL_TIER_SECONDS => Tier::Small,
                _ => Tier::Deep,
            };
            let out_of_scope = match mode {
                ModelMode::Symbolic => Some(
                    "symbolic (Apalache) model; Apalache-style checking is out of scope (charter)"
                        .to_owned(),
                ),
                ModelMode::Generate => Some(
                    "TLC -generate model; random behaviour generation is out of scope (ADR 0005)"
                        .to_owned(),
                ),
                _ => None,
            };
            models.push(ModelEntry {
                id: m.path.clone(),
                module: module.path.clone(),
                mode,
                features: module.features.clone(),
                tier,
                out_of_scope,
                counts_comparable: mode == ModelMode::Exhaustive,
                expected: ManifestExpectation {
                    result: m.result.clone(),
                    runtime_seconds: runtime,
                    distinct_states: m.distinct_states,
                    total_states: m.total_states,
                    state_depth: m.state_depth,
                    workers: m.workers.as_ref().map(|w| match w {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    }),
                },
                worker_introspecting: false,
            });
        }
    }
    Ok((modules, models))
}

fn tla_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, &e))?;
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let ft = entry.file_type().map_err(|e| Error::io(&path, &e))?;
        if ft.is_dir() {
            tla_files(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "tla")
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Enumerates a fetched corpus.
///
/// # Errors
/// Fails for the `tbd` layout ("not yet supported"), on unreadable files or
/// on manifest schema violations.
pub fn enumerate(corpus: &Corpus, root: &Path) -> Result<CorpusIndex> {
    let mut modules = Vec::new();
    let mut models = Vec::new();
    let mut uncovered = Vec::new();
    match corpus.layout {
        Layout::Tbd => {
            return Err(Error::Config(format!(
                "corpus `{}` has layout `tbd`: not yet supported",
                corpus.id
            )));
        }
        Layout::ModulesOnly => {
            let mut files = Vec::new();
            tla_files(root, root, &mut files)?;
            modules = files
                .into_iter()
                .map(|path| ModuleEntry {
                    path,
                    features: Vec::new(),
                })
                .collect();
        }
        Layout::ExamplesManifest => {
            let specs = root.join("specifications");
            let entries = std::fs::read_dir(&specs).map_err(|e| Error::io(&specs, &e))?;
            for entry in entries {
                let entry = entry.map_err(|e| Error::io(&specs, &e))?;
                let manifest_path = entry.path().join("manifest.json");
                if !manifest_path.is_file() {
                    if entry.path().is_dir() {
                        let rel = format!("specifications/{}", entry.file_name().to_string_lossy());
                        if !submodule_paths(root).contains(&rel) {
                            uncovered.push(format!("{rel}/ (no manifest.json)"));
                        }
                    }
                    continue;
                }
                let text = std::fs::read_to_string(&manifest_path)
                    .map_err(|e| Error::io(&manifest_path, &e))?;
                let manifest: Manifest = serde_json::from_str(&text)
                    .map_err(|e| Error::Config(format!("{}: {e}", manifest_path.display())))?;
                let (mods, mdls) = models_of(&manifest)?;
                modules.extend(mods);
                models.extend(mdls);
            }
        }
    }
    modules.sort_by(|a, b| a.path.cmp(&b.path));
    modules.dedup_by(|a, b| a.path == b.path);
    models.sort_by(|a, b| a.id.cmp(&b.id));
    if corpus.layout == Layout::ExamplesManifest {
        let mut files = Vec::new();
        let specs = root.join("specifications");
        tla_files(root, &specs, &mut files)?;
        let listed: std::collections::BTreeSet<&str> =
            modules.iter().map(|m| m.path.as_str()).collect();
        let subs = submodule_paths(root);
        for f in files {
            if !listed.contains(f.as_str()) && !subs.iter().any(|s| f.starts_with(&format!("{s}/")))
            {
                uncovered.push(f);
            }
        }
        uncovered.sort();
    }
    for m in &mut models {
        let text = std::fs::read_to_string(root.join(&m.module)).unwrap_or_default();
        m.worker_introspecting = reads_tlc_config(&text);
    }
    for pair in models.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(Error::Config(format!(
                "model `{}` listed twice",
                pair[0].id
            )));
        }
    }
    for path in modules
        .iter()
        .map(|m| &m.path)
        .chain(models.iter().map(|m| &m.id))
    {
        if path.starts_with('/')
            || path.split('/').any(|c| c == ".." || c.is_empty())
            || path.starts_with("sany/")
        {
            return Err(Error::Config(format!("unsafe corpus path `{path}`")));
        }
    }
    Ok(CorpusIndex {
        id: corpus.id.clone(),
        commit: corpus.commit.clone(),
        root: root.to_path_buf(),
        modules,
        models,
        unfetched_submodules: submodule_paths(root),
        uncovered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
  "sources": [], "authors": ["A"], "tags": [],
  "modules": [
    {"path": "specifications/x/X.tla", "features": ["pluscal"], "models": [
      {"path": "specifications/x/X.cfg", "runtime": "00:00:05", "mode": "exhaustive search",
       "result": "success", "distinctStates": 6, "totalStates": 15, "stateDepth": 2},
      {"path": "specifications/x/Big.cfg", "runtime": "00:01:00", "mode": "exhaustive search", "result": "safety failure"},
      {"path": "specifications/x/U.cfg", "runtime": "unknown", "mode": "exhaustive search", "result": "unknown", "workers": 4},
      {"path": "specifications/x/Apa.cfg", "runtime": "00:00:01", "mode": "symbolic", "result": "success"},
      {"path": "specifications/x/Gen.cfg", "runtime": "00:00:01", "mode": "generate", "result": "success"},
      {"path": "specifications/x/Sim.cfg", "runtime": "00:00:01", "mode": {"simulate": {"traceCount": 100}}, "result": "success"}
    ]},
    {"path": "specifications/x/XProof.tla", "features": [], "proof": {"maxRuntimeMinutes": 1}, "models": []}
  ]
}"#;

    #[test]
    fn manifest_tiers_and_scope() -> Result<()> {
        let m: Manifest =
            serde_json::from_str(MANIFEST).map_err(|e| Error::Config(e.to_string()))?;
        let (modules, models) = models_of(&m)?;
        assert_eq!(modules.len(), 2);
        let by = |id: &str| models.iter().find(|m| m.id.ends_with(id)).cloned();
        let x = by("/X.cfg").ok_or_else(|| Error::Config("X".into()))?;
        assert_eq!(x.tier, Tier::Small);
        assert_eq!(x.expected.distinct_states, Some(6));
        assert_eq!(x.features, vec!["pluscal".to_owned()]);
        assert!(x.out_of_scope.is_none());
        assert_eq!(by("/Big.cfg").map(|m| m.tier), Some(Tier::Deep));
        let u = by("/U.cfg").ok_or_else(|| Error::Config("U".into()))?;
        assert_eq!(
            (u.tier, u.expected.workers.as_deref()),
            (Tier::Deep, Some("4"))
        );
        assert!(
            by("/Apa.cfg").is_some_and(|m| m.out_of_scope.is_some_and(|r| r.contains("Apalache")))
        );
        assert!(by("/Gen.cfg").is_some_and(|m| m.out_of_scope.is_some()));
        let sim = by("/Sim.cfg").ok_or_else(|| Error::Config("Sim".into()))?;
        assert_eq!(sim.mode, ModelMode::Simulate { traces: 100 });
        assert!(sim.out_of_scope.is_none() && !sim.counts_comparable);
        assert_eq!(sim.tier, Tier::Deep);
        Ok(())
    }

    #[test]
    fn schema_is_strict() {
        let bad = MANIFEST.replace("\"tags\": []", "\"tags\": [], \"extra\": 1");
        assert!(serde_json::from_str::<Manifest>(&bad).is_err());
        assert!(parse_runtime("1:00:00").is_err());
        assert!(matches!(parse_runtime("01:02:03"), Ok(Some(3723))));
        assert!(matches!(parse_runtime("unknown"), Ok(None)));
    }
}
