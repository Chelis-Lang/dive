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
    /// `Some(reason)` if the module is excluded (`[[corpus.exclude]]`): it is
    /// reported as `out-of-scope`, never run.
    #[serde(default)]
    pub out_of_scope: Option<String>,
}

/// An enumerated corpus.
#[derive(Debug, Clone)]
pub struct CorpusIndex {
    /// Corpus id.
    pub id: String,
    /// Pinned commit, or for an in-repository corpus its
    /// [`tree_digest`] (`sha256:<hex>`). Goldens record it.
    pub commit: String,
    /// Whether the corpus lives in this repository (`path` in
    /// `corpora.toml`) rather than in a cached git checkout.
    pub in_repo: bool,
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
    isolation(path).0
}

/// [`isolation_dir`], and whether that directory is needed recursively. An
/// Examples spec directory is copied whole (models may read data files in
/// subdirectories). Any other module needs only the files of its own
/// directory: SANY resolves modules in the module's directory and on the
/// classpath, never in subdirectories. Copying only those files keeps
/// isolation cheap for large test trees (tlaplus `test-model/`).
#[must_use]
pub fn isolation(path: &str) -> (String, bool) {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "specifications" {
        return (format!("specifications/{}", parts[1]), true);
    }
    (parts[..parts.len().saturating_sub(1)].join("/"), false)
}

/// Names of modules that `text` extends or instantiates (`EXTENDS A, B`,
/// `INSTANCE C`). A plain scan; comments are not stripped, so it may find
/// extra names, which only widens the search.
#[must_use]
pub fn referenced_modules(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let ident = |w: &str| {
        w.chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
    };
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("EXTENDS") {
            names.extend(
                rest.split(',')
                    .map(|n| ident(n.trim()))
                    .filter(|n| !n.is_empty()),
            );
        }
        let mut rest = line;
        while let Some(i) = rest.find("INSTANCE ") {
            rest = &rest[i + "INSTANCE ".len()..];
            let n = ident(rest.trim_start());
            if !n.is_empty() {
                names.push(n);
            }
        }
    }
    names
}

/// Whether `module` or any module it extends or instantiates from the same
/// directory (transitively) reads `TLCGet("config")`. Examples
/// `MCDieHardest` reads it through `DieHardest`.
#[must_use]
pub fn module_closure_reads_tlc_config(module: &Path) -> bool {
    let Some(dir) = module.parent() else {
        return false;
    };
    let mut todo = vec![module.to_path_buf()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(path) = todo.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if reads_tlc_config(&text) {
            return true;
        }
        for name in referenced_modules(&text) {
            let next = dir.join(format!("{name}.tla"));
            if next.is_file() {
                todo.push(next);
            }
        }
    }
    false
}

/// Whether a module reads TLC's configuration (`TLCGet("config")`), which
/// includes the worker count, so its behaviour may legitimately depend on
/// `-workers` (Examples `MCDieHardest` asserts `TLCGet("config").worker = 1`).
#[must_use]
pub fn reads_tlc_config(module_text: &str) -> bool {
    let squashed: String = module_text.chars().filter(|c| !c.is_whitespace()).collect();
    squashed.contains("TLCGet(\"config\")")
}

/// The identity of an in-repository corpus: `sha256:<hex>` over every file
/// under `root` (except `.DS_Store`), in path order, each as its
/// `/`-separated relative path, a NUL byte, its length as 8 little-endian
/// bytes, and its contents. Any added, removed, renamed or edited file
/// changes it, so goldens generated before an edit are refused until they
/// are regenerated.
///
/// # Errors
/// Fails on an unreadable file, or on a symbolic link or other special file
/// (a corpus must be plain files).
pub fn tree_digest(root: &Path) -> Result<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
        let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, &e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(dir, &e))?;
            let path = entry.path();
            if entry.file_name() == ".DS_Store" {
                continue;
            }
            let ft = entry.file_type().map_err(|e| Error::io(&path, &e))?;
            if ft.is_dir() {
                walk(root, &path, out)?;
            } else if ft.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|_| Error::Io(format!("{} escapes the corpus", path.display())))?;
                out.push((rel.to_string_lossy().replace('\\', "/"), path));
            } else {
                return Err(Error::Config(format!(
                    "{}: in-repository corpora hold plain files only",
                    path.display()
                )));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    let mut buf = Vec::new();
    for (rel, path) in &files {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path, &e))?;
        buf.extend_from_slice(rel.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(&bytes);
    }
    Ok(format!("sha256:{}", crate::hash::sha256_bytes(&buf)))
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
            out_of_scope: None,
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

/// Collects the `.tla` files under `dir`, as `root`-relative paths. Hidden
/// entries are skipped unless `hidden` is set; `.git` always is.
fn tla_files(root: &Path, dir: &Path, hidden: bool, out: &mut Vec<String>) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, &e))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || (!hidden && name.starts_with('.')) {
            continue;
        }
        let ft = entry.file_type().map_err(|e| Error::io(&path, &e))?;
        if ft.is_dir() {
            tla_files(root, &path, hidden, out)?;
        } else if path.extension().is_some_and(|e| e == "tla")
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// The modules of a `modules-only` corpus: every `.tla` file matching
/// `include`, with `exclude` entries marked out of scope.
fn modules_only(corpus: &Corpus, root: &Path) -> Result<Vec<ModuleEntry>> {
    let mut files = Vec::new();
    // Hidden directories count: CommunityModules keeps its smoke tests in
    // `.smoke/`.
    tla_files(root, root, true, &mut files)?;
    files.retain(|f| crate::util::matches_any(&corpus.include, f));
    for x in &corpus.exclude {
        if !files.contains(&x.path) {
            return Err(Error::Config(format!(
                "corpus `{}`: excluded module `{}` is not an enumerated module",
                corpus.id, x.path
            )));
        }
    }
    Ok(files
        .into_iter()
        .map(|path| ModuleEntry {
            out_of_scope: corpus
                .exclude
                .iter()
                .find(|x| x.path == path)
                .map(|x| format!("excluded in corpora.toml: {}", x.reason)),
            path,
            features: Vec::new(),
        })
        .collect())
}

/// Enumerates a fetched corpus.
///
/// # Errors
/// Fails for the `tbd` layout ("not yet supported"), on unreadable files or
/// on manifest schema violations.
pub fn enumerate(corpus: &Corpus, root: &Path) -> Result<CorpusIndex> {
    enumerate_with(corpus, root, None)
}

/// [`enumerate`] for a repository-pinned corpus whose modules and models a
/// dive-owned manifest lists (`manifest` in `corpora.toml`). Every listed
/// file must exist in the checkout. `[[corpus.exclude]]` entries name model
/// ids (`.cfg` paths) that are reported as `out-of-scope` with their reason,
/// for example models the pinned TLC cannot finish within the corpus cap.
/// Files the manifest does not list are not reported as uncovered: such a
/// corpus is a selection from its upstream repository.
///
/// # Errors
/// As [`enumerate`]; also fails if a listed file is missing or an exclusion
/// names no listed model.
pub fn enumerate_manifest(
    corpus: &Corpus,
    root: &Path,
    manifest: &Manifest,
) -> Result<CorpusIndex> {
    let mut index = enumerate_with(corpus, root, Some(manifest))?;
    for path in index
        .modules
        .iter()
        .map(|m| &m.path)
        .chain(index.models.iter().map(|m| &m.id))
    {
        if !root.join(path).is_file() {
            return Err(Error::Config(format!(
                "corpus `{}`: manifest lists `{path}`, which is not a file in the checkout",
                corpus.id
            )));
        }
    }
    for x in &corpus.exclude {
        let Some(model) = index.models.iter_mut().find(|m| m.id == x.path) else {
            return Err(Error::Config(format!(
                "corpus `{}`: excluded model `{}` is not listed in its manifest",
                corpus.id, x.path
            )));
        };
        model.out_of_scope = Some(format!("excluded in corpora.toml: {}", x.reason));
    }
    Ok(index)
}

fn enumerate_with(
    corpus: &Corpus,
    root: &Path,
    external: Option<&Manifest>,
) -> Result<CorpusIndex> {
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
        Layout::ModulesOnly => modules = modules_only(corpus, root)?,
        Layout::ExamplesManifest if external.is_some() => {
            if let Some(manifest) = external {
                (modules, models) = models_of(manifest)?;
            }
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
    if corpus.layout == Layout::ExamplesManifest && external.is_none() {
        let mut files = Vec::new();
        let specs = root.join("specifications");
        tla_files(root, &specs, false, &mut files)?;
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
        m.worker_introspecting = module_closure_reads_tlc_config(&root.join(&m.module));
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
        in_repo: corpus.path.is_some(),
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
    fn dive_owned_manifest_selects_from_a_checkout()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("manifest")?;
        std::fs::create_dir_all(dir.path().join("tla"))?;
        for f in ["tla/M.tla", "tla/M.cfg", "tla/Big.cfg", "tla/Other.tla"] {
            std::fs::write(dir.path().join(f), "x")?;
        }
        let manifest: Manifest = serde_json::from_str(
            r#"{"sources": [], "authors": ["A"], "tags": [], "modules": [
              {"path": "tla/M.tla", "features": [], "models": [
                {"path": "tla/M.cfg", "runtime": "00:00:05", "mode": "exhaustive search", "result": "success"},
                {"path": "tla/Big.cfg", "runtime": "unknown", "mode": "exhaustive search", "result": "unknown"}
              ]}]}"#,
        )?;
        let toml = |exclude: &str| {
            format!(
                "[[corpus]]\nid = \"h\"\nrepo = \"https://x\"\ncommit = \"{}\"\nlicense = \"MIT\"\nlayout = \"examples-manifest\"\nmanifest = \"conformance/h.json\"\n{exclude}",
                "0".repeat(40)
            )
        };
        let excl = "[[corpus.exclude]]\npath = \"tla/Big.cfg\"\nreason = \"TLC exceeds the cap\"\n";
        let file = crate::config::CorporaFile::parse(&toml(excl))?;
        let index = enumerate_manifest(&file.corpus[0], dir.path(), &manifest)?;
        assert_eq!(index.modules.len(), 1);
        assert!(
            index.uncovered.is_empty(),
            "unlisted upstream files are not uncovered"
        );
        let big = index.models.iter().find(|m| m.id == "tla/Big.cfg");
        assert!(big.is_some_and(|m| m.out_of_scope.as_deref().is_some_and(|r| r.contains("cap"))));
        let small = index.models.iter().find(|m| m.id == "tla/M.cfg");
        assert!(small.is_some_and(|m| m.out_of_scope.is_none() && m.tier == Tier::Small));
        let bad = toml(&excl.replace("tla/Big.cfg", "tla/None.cfg"));
        let file = crate::config::CorporaFile::parse(&bad)?;
        assert!(enumerate_manifest(&file.corpus[0], dir.path(), &manifest).is_err());
        std::fs::remove_file(dir.path().join("tla/M.cfg"))?;
        let file = crate::config::CorporaFile::parse(&toml(""))?;
        assert!(enumerate_manifest(&file.corpus[0], dir.path(), &manifest).is_err());
        Ok(())
    }

    #[test]
    fn tree_digest_covers_names_and_contents() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let dir = crate::util::TempDir::new("digest")?;
        std::fs::create_dir_all(dir.path().join("specifications/a"))?;
        std::fs::write(dir.path().join("specifications/a/A.tla"), "x")?;
        let d0 = tree_digest(dir.path())?;
        assert!(d0.starts_with("sha256:"));
        assert_eq!(tree_digest(dir.path())?, d0);
        std::fs::write(dir.path().join(".DS_Store"), "ignored")?;
        assert_eq!(tree_digest(dir.path())?, d0);
        std::fs::write(dir.path().join("specifications/a/A.tla"), "y")?;
        let d1 = tree_digest(dir.path())?;
        assert_ne!(d1, d0);
        std::fs::rename(
            dir.path().join("specifications/a/A.tla"),
            dir.path().join("specifications/a/B.tla"),
        )?;
        assert_ne!(tree_digest(dir.path())?, d1);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/", dir.path().join("link"))?;
            assert!(tree_digest(dir.path()).is_err());
        }
        Ok(())
    }

    #[test]
    fn worker_introspection_follows_extends() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let dir = crate::util::TempDir::new("introspect")?;
        std::fs::write(
            dir.path().join("MC.tla"),
            "---- MODULE MC ----\nEXTENDS Base, Naturals\n====\n",
        )?;
        std::fs::write(
            dir.path().join("Base.tla"),
            "---- MODULE Base ----\nEXTENDS TLC\nASSUME TLCGet( \"config\" ).worker = 1\n====\n",
        )?;
        std::fs::write(
            dir.path().join("Plain.tla"),
            "---- MODULE Plain ----\nX == INSTANCE MC\n====\n",
        )?;
        assert!(module_closure_reads_tlc_config(&dir.path().join("MC.tla")));
        assert!(module_closure_reads_tlc_config(
            &dir.path().join("Plain.tla")
        ));
        std::fs::write(
            dir.path().join("Other.tla"),
            "---- MODULE Other ----\nEXTENDS Naturals\n====\n",
        )?;
        assert!(!module_closure_reads_tlc_config(
            &dir.path().join("Other.tla")
        ));
        assert_eq!(
            referenced_modules("EXTENDS A, B_1\nY == INSTANCE C WITH x <- 1"),
            ["A", "B_1", "C"]
        );
        Ok(())
    }

    #[test]
    fn modules_only_include_exclude_and_hidden()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("modonly")?;
        for p in [
            "t/A.tla",
            "t/sub/B.tla",
            ".smoke/S.tla",
            "other/O.tla",
            ".git/G.tla",
        ] {
            let f = dir.path().join(p);
            std::fs::create_dir_all(f.parent().ok_or("no parent")?)?;
            std::fs::write(&f, "---- MODULE X ----\n====\n")?;
        }
        let toml = |extra: &str| {
            format!(
                "[[corpus]]\nid = \"c\"\nrepo = \"x\"\ncommit = \"{}\"\nlicense = \"MIT\"\nlayout = \"modules-only\"\n{extra}",
                "a".repeat(40)
            )
        };
        let all = crate::config::CorporaFile::parse(&toml(""))?;
        let paths = |c: &Corpus| -> Result<Vec<String>> {
            Ok(modules_only(c, dir.path())?
                .into_iter()
                .map(|m| m.path)
                .collect())
        };
        let mut got = paths(&all.corpus[0])?;
        got.sort();
        assert_eq!(
            got,
            [".smoke/S.tla", "other/O.tla", "t/A.tla", "t/sub/B.tla"]
        );
        let some = crate::config::CorporaFile::parse(&toml(
            "include = [\"t/**\"]\n[[corpus.exclude]]\npath = \"t/A.tla\"\nreason = \"crashes\"\n",
        ))?;
        let mods = modules_only(&some.corpus[0], dir.path())?;
        assert_eq!(mods.len(), 2);
        assert!(mods.iter().any(|m| {
            m.path == "t/A.tla"
                && m.out_of_scope
                    .as_deref()
                    .is_some_and(|r| r.contains("crashes"))
        }));
        let stale = crate::config::CorporaFile::parse(&toml(
            "[[corpus.exclude]]\npath = \"gone/X.tla\"\nreason = \"r\"\n",
        ))?;
        assert!(modules_only(&stale.corpus[0], dir.path()).is_err());
        assert!(
            crate::config::CorporaFile::parse(&toml(
                "[[corpus.exclude]]\npath = \"t/A.tla\"\nreason = \" \"\n"
            ))
            .is_err()
        );
        assert_eq!(
            isolation("specifications/x/sub/M.tla"),
            ("specifications/x".to_owned(), true)
        );
        assert_eq!(isolation("t/sub/B.tla"), ("t/sub".to_owned(), false));
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
