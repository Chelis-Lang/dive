//! The commands: fetching, golden generation and runs.

use crate::cache::Cache;
use crate::compare::{
    CompareContext, ComparePolicy, FieldDiff, Outcome, WorkerPoint, crosscheck_manifest,
    diff_results_with, diff_sany, diff_verdict_and_class, worker_invariance,
};
use crate::config::{Config, Layout};
use crate::corpus::{CorpusIndex, ModelEntry, ModelMode, ModuleEntry, Tier, enumerate};
use crate::error::{Error, Result};
use crate::fetch::{FetchStatus, Fetcher, dirty_paths, verified_checkout};
use crate::golden::{
    GOLDEN_SCHEMA, GoldenModel, ModelGolden, Provenance, SanyGolden, WriteStatus, golden_root,
    model_golden_path, read_model_golden, read_sany_golden, sany_golden_path, to_pretty,
    worker_golden_path, write_golden,
};
use crate::implementation::{
    ImplResult, Implementation, ModelRequest, OracleModelRun, run_oracle_model_traced,
    run_oracle_sany,
};
use crate::oracle::OracleEnv;
use crate::report::{
    CorpusInfo, Host, ModelReport, ModuleReport, Report, RunRecord, Settings, render_markdown,
    summarise,
};
use crate::sany::{SanyPhase, SanyVerdict};
use crate::tlc::result::Verdict;
use crate::util::{compact_stamp, matches_any, rfc3339, unix_now, write_atomic};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The harness version string recorded in goldens and reports.
#[must_use]
pub fn generator() -> String {
    format!("dive-conformance {}", env!("CARGO_PKG_VERSION"))
}

fn log(msg: &str) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr().lock(), "{msg}");
}

/// Runs `f` over `items` on `jobs` threads, preserving order.
pub fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    jobs: usize,
    f: impl Fn(usize, &T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1).min(items.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = f(i, item);
                    if let Ok(mut guard) = results.lock()
                        && let Some(slot) = guard.get_mut(i)
                    {
                        *slot = Some(r);
                    }
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

/// `oracle fetch`: downloads and verifies the gating oracles (or only
/// `selected`, which may be a reference oracle), the classpath jars, the JRE
/// for this host and the module libraries.
///
/// # Errors
/// Fails on the first download or verification failure, or if `selected`
/// is not a pinned oracle.
pub fn oracle_fetch(config: &Config, cache: &Cache, selected: Option<&str>) -> Result<()> {
    let fetcher = Fetcher::new(cache.clone());
    let report = |what: &str, status: &FetchStatus, path: &Path| {
        log(&format!(
            "{:<11} {what} -> {}",
            match status {
                FetchStatus::Cached => "verified",
                FetchStatus::Downloaded => "downloaded",
            },
            path.display()
        ));
    };
    for o in config.oracles.selected_or_gating(selected)? {
        let (p, s) =
            fetcher.fetch_artifact(&format!("TLC oracle `{}`", o.id), &o.url, &o.sha256)?;
        report(&o.id, &s, &p);
    }
    for a in &config.oracles.classpath {
        let (p, s) =
            fetcher.fetch_artifact(&format!("classpath jar `{}`", a.id), &a.url, &a.sha256)?;
        report(&a.id, &s, &p);
    }
    let jre = config.oracles.host_jre()?;
    let (p, s) = fetcher.fetch_jre(&jre.url, &jre.sha256)?;
    report(
        &format!("JRE {} ({})", config.oracles.jre.version, jre.id),
        &s,
        &p,
    );
    for lib in &config.oracles.module_library {
        let (p, s) = fetcher.fetch_library(lib)?;
        report(&format!("{} @ {}", lib.id, lib.commit), &s, &p);
    }
    Ok(())
}

/// `corpus fetch`: checks out each selected corpus at its pinned commit.
/// Submodules are not fetched; they are listed as unfetched.
///
/// # Errors
/// Fails on the first git failure.
pub fn corpus_fetch(config: &Config, cache: &Cache, ids: &[String]) -> Result<()> {
    let fetcher = Fetcher::new(cache.clone());
    for c in &config.corpora.corpus {
        if !ids.is_empty() && !ids.contains(&c.id) {
            continue;
        }
        if let Some(path) = &c.path {
            log(&format!(
                "corpus `{}` is in this repository ({path}); nothing to fetch",
                c.id
            ));
            continue;
        }
        let (dir, status) = fetcher.fetch_checkout(&c.id, &c.repo, &c.commit)?;
        log(&format!(
            "{:?} corpus `{}` @ {} -> {}",
            status,
            c.id,
            c.commit,
            dir.display()
        ));
        let subs = crate::fetch::submodule_paths(&dir);
        if !subs.is_empty() {
            log(&format!("  unfetched submodules: {}", subs.join(", ")));
        }
    }
    Ok(())
}

/// Loads a fetched corpus: verified at its commit, unmodified, enumerated.
///
/// # Errors
/// Fails if the corpus is missing, modified or cannot be enumerated.
pub fn load_corpus(config: &Config, cache: &Cache, id: &str) -> Result<CorpusIndex> {
    let c = config.corpora.corpus(id)?;
    let index = if let Some(path) = &c.path {
        // A dive-owned corpus in this repository: identified by the digest
        // of its files, which goldens record in place of a commit.
        let dir = config.repo_root.join(path);
        if !dir.is_dir() {
            return Err(Error::Config(format!(
                "corpus `{id}`: {} is not a directory",
                dir.display()
            )));
        }
        let digest = crate::corpus::tree_digest(&dir)?;
        let mut index = enumerate(c, &dir)?;
        index.commit = digest;
        index
    } else {
        let dir = cache.checkout_dir(&c.id, &c.commit);
        verified_checkout(&dir, &c.commit, &format!("corpus `{id}`"))?;
        enumerate(c, &dir)?
    };
    if !index.uncovered.is_empty() {
        log(&format!(
            "warning: {} path(s) in corpus `{id}` are not covered by any manifest (no goldens): {}{}",
            index.uncovered.len(),
            index
                .uncovered
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            if index.uncovered.len() > 5 {
                ", ..."
            } else {
                ""
            }
        ));
    }
    Ok(index)
}

/// Which models and modules a command works on.
#[derive(Debug, Clone)]
pub struct Selection {
    /// `small`, `deep` or `all`.
    pub tier: String,
    /// `--filter` globs over model and module paths.
    pub filters: Vec<String>,
}

impl Selection {
    fn tier_ok(&self, t: Tier) -> Result<bool> {
        match self.tier.as_str() {
            "all" => Ok(true),
            "small" => Ok(t == Tier::Small),
            "deep" => Ok(t == Tier::Deep),
            other => Err(Error::Usage(format!(
                "--tier must be small, deep or all (got `{other}`)"
            ))),
        }
    }

    /// The selected models, including out-of-scope ones (which are reported,
    /// never run).
    ///
    /// # Errors
    /// Fails on a bad tier name.
    pub fn models<'a>(&self, corpus: &'a CorpusIndex) -> Result<Vec<&'a ModelEntry>> {
        let mut out = Vec::new();
        for m in &corpus.models {
            if self.tier_ok(m.tier)? && matches_any(&self.filters, &m.id) {
                out.push(m);
            }
        }
        Ok(out)
    }

    /// The selected modules: all modules without filters; with filters, the
    /// modules that match one, plus the modules of selected models. Excluded
    /// modules are included (they are reported, never run).
    ///
    /// # Errors
    /// Fails on a bad tier name.
    pub fn modules<'a>(&self, corpus: &'a CorpusIndex) -> Result<Vec<&'a ModuleEntry>> {
        let models = self.models(corpus)?;
        let mut out: Vec<&ModuleEntry> = corpus
            .modules
            .iter()
            .filter(|m| {
                self.filters.is_empty()
                    || matches_any(&self.filters, &m.path)
                    || models.iter().any(|x| x.module == m.path)
            })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out.dedup_by(|a, b| a.path == b.path);
        Ok(out)
    }
}

/// Resource limits for oracle runs.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Parallel jobs.
    pub jobs: usize,
    /// `-Xmx`.
    pub xmx: String,
    /// Fixed timeout overriding the derived one.
    pub timeout: Option<Duration>,
    /// Cap on derived timeouts.
    pub timeout_cap: Duration,
}

impl Limits {
    /// The timeout for a model.
    ///
    /// - The *derived* timeout from the manifest's recorded runtime is
    ///   `20 × runtime + 120 s`, at least 300 s, capped by `timeout_cap`; the
    ///   cap when the runtime is unknown. The factor is generous because
    ///   manifest runtimes were measured with several workers and goldens run
    ///   with one (Examples `BufferedRandomAccessFile`: 9 s recorded, 102 s
    ///   single-worker).
    /// - When a golden exists, its recorded single-worker time also counts:
    ///   the timeout is at least `4 × golden elapsed + 60 s`, even above the
    ///   cap, since that run is evidence of what the model needs.
    /// - `--timeout` overrides both.
    #[must_use]
    pub fn timeout_for(&self, m: &ModelEntry, golden_elapsed_ms: Option<u64>) -> Duration {
        if let Some(t) = self.timeout {
            return t;
        }
        let cap = self.timeout_cap;
        let derived = m.expected.runtime_seconds.map_or(cap, |s| {
            Duration::from_secs(s.saturating_mul(20).saturating_add(120))
                .max(Duration::from_secs(300))
                .min(cap)
        });
        let from_golden = golden_elapsed_ms.map_or(Duration::ZERO, |ms| {
            Duration::from_millis(ms.saturating_mul(4)).saturating_add(Duration::from_secs(60))
        });
        derived.max(from_golden)
    }

    /// The SANY timeout.
    #[must_use]
    pub fn sany_timeout(&self) -> Duration {
        self.timeout.unwrap_or(Duration::from_secs(300))
    }

    /// Human description of the timeout policy.
    #[must_use]
    pub fn policy(&self) -> String {
        self.timeout.map_or_else(
            || {
                format!(
                    "max(20 x recorded runtime + 120 s [min 300 s, cap {} s, cap when unknown], 4 x golden elapsed + 60 s)",
                    self.timeout_cap.as_secs()
                )
            },
            |t| format!("fixed {} s", t.as_secs()),
        )
    }
}

/// Options of `golden generate`.
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Oracle id.
    pub oracle: String,
    /// Corpus id.
    pub corpus: String,
    /// Selection.
    pub selection: Selection,
    /// Recorded reason; required to overwrite.
    pub reason: Option<String>,
    /// Limits.
    pub limits: Limits,
    /// Generate TLC goldens.
    pub models: bool,
    /// Generate SANY goldens.
    pub sany: bool,
    /// TLC's `-workers` for the TLC goldens: 1 writes the goldens. A count
    /// above 1 writes worker-specific goldens ([`worker_golden_path`]),
    /// only for worker-introspecting models and only with a reason; it
    /// never touches a single-worker golden (ADR 0004).
    pub workers: Vec<u32>,
}

fn provenance(
    env: &OracleEnv,
    corpus: &CorpusIndex,
    command: Vec<String>,
    timeout: Duration,
    reason: Option<&str>,
    elapsed: Duration,
) -> Provenance {
    Provenance {
        oracle: env.provenance(),
        corpus_commit: corpus.commit.clone(),
        command,
        trace_command: None,
        timeout_seconds: timeout.as_secs(),
        reason: reason.map(str::to_owned),
        generator: generator(),
        generated_at: rfc3339(unix_now()),
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        uncompared_counts: None,
    }
}

/// Checks that oracle runs left the corpus checkout untouched.
fn assert_pristine(corpus: &CorpusIndex) -> Result<()> {
    if corpus.in_repo {
        let now = crate::corpus::tree_digest(&corpus.root)?;
        return if now == corpus.commit {
            Ok(())
        } else {
            Err(Error::Command(format!(
                "the in-repository corpus {} changed during the run ({} -> {now})",
                corpus.root.display(),
                corpus.commit
            )))
        };
    }
    let dirty = dirty_paths(&corpus.root)?;
    if dirty.is_empty() {
        Ok(())
    } else {
        Err(Error::Command(format!(
            "oracle runs modified the corpus checkout {}: {}",
            corpus.root.display(),
            dirty.join("; ")
        )))
    }
}

/// Fails if two inputs would map to the same golden file, compared
/// case-insensitively (macOS and Windows file systems fold case).
fn check_path_collisions<'a>(paths: impl Iterator<Item = (&'a str, PathBuf)>) -> Result<()> {
    let mut seen: std::collections::BTreeMap<String, &'a str> = std::collections::BTreeMap::new();
    for (id, path) in paths {
        let key = path.display().to_string().to_lowercase();
        if let Some(other) = seen.insert(key, id) {
            return Err(Error::Golden(format!(
                "`{other}` and `{id}` map to the same golden file {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// `golden generate`. Returns the number of failures (refused overwrites,
/// timeouts, harness errors); goldens that could be written are written.
///
/// # Errors
/// Fails if the oracle or corpus cannot be prepared.
#[allow(
    clippy::too_many_lines,
    reason = "one linear pass over models then modules; splitting obscures it"
)]
pub fn golden_generate(
    config: &Config,
    cache: &Cache,
    env: &OracleEnv,
    opts: &GenerateOptions,
) -> Result<usize> {
    let corpus = load_corpus(config, cache, &opts.corpus)?;
    let root = golden_root(&config.repo_root);
    let failures = AtomicUsize::new(0);
    let fail = |msg: String| {
        failures.fetch_add(1, Ordering::Relaxed);
        log(&msg);
    };
    let models = if opts.models {
        opts.selection.models(&corpus)?
    } else {
        Vec::new()
    };
    let (runnable, skipped): (Vec<&ModelEntry>, Vec<&ModelEntry>) =
        models.into_iter().partition(|m| m.out_of_scope.is_none());
    for m in &skipped {
        log(&format!(
            "out-of-scope  {}: {}",
            m.id,
            m.out_of_scope.as_deref().unwrap_or("")
        ));
    }
    if opts.workers.iter().any(|&w| w > 1)
        && opts.reason.as_deref().is_none_or(|r| r.trim().is_empty())
    {
        return Err(Error::Usage(
            "worker-specific goldens (--workers above 1) require --reason \"<why>\"".to_owned(),
        ));
    }
    let golden_path = |m: &ModelEntry, w: u32| {
        if w == 1 {
            model_golden_path(&root, &env.oracle.id, &corpus.id, &m.id)
        } else {
            worker_golden_path(&root, &env.oracle.id, &corpus.id, &m.id, w)
        }
    };
    let mut jobs: Vec<(&ModelEntry, u32)> = Vec::new();
    for &w in &opts.workers {
        for m in &runnable {
            if w > 1 && !m.worker_introspecting {
                log(&format!(
                    "skipped       {} at {w} workers: not worker-introspecting, so its single-worker golden applies at every worker count",
                    m.id
                ));
                continue;
            }
            jobs.push((m, w));
        }
    }
    check_path_collisions(
        jobs.iter()
            .map(|(m, w)| (m.id.as_str(), golden_path(m, *w))),
    )?;
    let total = jobs.len();
    log(&format!(
        "generating {total} TLC golden(s) with {} ({} job(s), -Xmx{})",
        env.oracle.id, opts.limits.jobs, opts.limits.xmx
    ));
    let done = AtomicUsize::new(0);
    parallel_map(&jobs, opts.limits.jobs, |_, &(m, workers)| {
        let timeout = opts.limits.timeout_for(m, None);
        let req = ModelRequest {
            corpus_root: &corpus.root,
            model: m,
            workers,
            timeout,
            xmx: &opts.limits.xmx,
        };
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        match run_oracle_model_traced(env, &req) {
            Err(e) => fail(format!("[{n}/{total}] ERROR {}: {e}", m.id)),
            Ok(OracleModelRun {
                result: ImplResult::Unsupported(why),
                ..
            }) => {
                fail(format!("[{n}/{total}] ERROR {}: {why}", m.id));
            }
            Ok(OracleModelRun {
                result:
                    ImplResult::Completed {
                        result,
                        command,
                        elapsed,
                    },
                trace_command,
            }) => {
                if result.verdict == Verdict::Timeout {
                    fail(format!(
                        "[{n}/{total}] TIMEOUT {} after {} s; no golden written",
                        m.id,
                        timeout.as_secs()
                    ));
                    return;
                }
                if result.verdict == Verdict::ResourceLimit {
                    fail(format!(
                        "[{n}/{total}] RESOURCE-LIMIT {}: {}; no golden written (not an oracle fact)",
                        m.id,
                        result.note.as_deref().unwrap_or("")
                    ));
                    return;
                }
                let verdict = result.verdict;
                let mut result = result;
                let mut prov = provenance(
                    env,
                    &corpus,
                    command,
                    timeout,
                    opts.reason.as_deref(),
                    elapsed,
                );
                prov.trace_command = trace_command;
                if !m.counts_comparable {
                    // Keep the payload deterministic (ADR 0003).
                    prov.uncompared_counts = Some(std::mem::take(&mut result.counts));
                }
                let golden = ModelGolden {
                    schema: GOLDEN_SCHEMA,
                    kind: "tlc-model".to_owned(),
                    oracle: env.oracle.id.clone(),
                    corpus: corpus.id.clone(),
                    model: GoldenModel::from(m),
                    workers,
                    result,
                    provenance: prov,
                };
                let path = golden_path(m, workers);
                match to_pretty(&golden)
                    .and_then(|t| write_golden(&path, &t, opts.reason.as_deref()))
                {
                    Ok(status) => log(&format!(
                        "[{n}/{total}] {:<26} {}{} ({:.1}s, {})",
                        verdict.as_str(),
                        m.id,
                        if workers == 1 {
                            String::new()
                        } else {
                            format!(" at {workers} workers")
                        },
                        elapsed.as_secs_f64(),
                        match status {
                            WriteStatus::Created => "created",
                            WriteStatus::Replaced => "REPLACED, payload changed",
                            WriteStatus::ReplacedSamePayload => "replaced, payload unchanged",
                        }
                    )),
                    Err(e) => fail(format!("[{n}/{total}] REFUSED {}: {e}", m.id)),
                }
            }
        }
    });
    if opts.sany {
        let (modules, excluded): (Vec<&ModuleEntry>, Vec<&ModuleEntry>) = opts
            .selection
            .modules(&corpus)?
            .into_iter()
            .partition(|m| m.out_of_scope.is_none());
        for m in &excluded {
            log(&format!(
                "out-of-scope  {}: {}",
                m.path,
                m.out_of_scope.as_deref().unwrap_or("")
            ));
        }
        let modules: Vec<String> = modules.into_iter().map(|m| m.path.clone()).collect();
        check_path_collisions(modules.iter().map(|m| {
            (
                m.as_str(),
                sany_golden_path(&root, &env.oracle.id, &corpus.id, m),
            )
        }))?;
        let total = modules.len();
        log(&format!(
            "generating {total} SANY golden(s) with {}",
            env.oracle.id
        ));
        let done = AtomicUsize::new(0);
        parallel_map(&modules, opts.limits.jobs, |_, module| {
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            let timeout = opts.limits.sany_timeout();
            match run_oracle_sany(env, &corpus.root, module, timeout) {
                Err(e) => fail(format!("[{n}/{total}] ERROR {module}: {e}")),
                Ok(ImplResult::Unsupported(why)) => {
                    fail(format!("[{n}/{total}] ERROR {module}: {why}"));
                }
                Ok(ImplResult::Completed {
                    result,
                    command,
                    elapsed,
                }) => {
                    if result.verdict == SanyVerdict::Timeout {
                        fail(format!(
                            "[{n}/{total}] TIMEOUT SANY {module}; no golden written"
                        ));
                        return;
                    }
                    let accepted = result.accepted;
                    let golden = SanyGolden {
                        schema: GOLDEN_SCHEMA,
                        kind: "sany-module".to_owned(),
                        oracle: env.oracle.id.clone(),
                        corpus: corpus.id.clone(),
                        module: module.clone(),
                        result,
                        provenance: provenance(
                            env,
                            &corpus,
                            command,
                            timeout,
                            opts.reason.as_deref(),
                            elapsed,
                        ),
                    };
                    let path = sany_golden_path(&root, &env.oracle.id, &corpus.id, module);
                    match to_pretty(&golden)
                        .and_then(|t| write_golden(&path, &t, opts.reason.as_deref()))
                    {
                        Ok(_) => log(&format!(
                            "[{n}/{total}] SANY {} {module}",
                            if accepted { "accept" } else { "REJECT" }
                        )),
                        Err(e) => fail(format!("[{n}/{total}] REFUSED {module}: {e}")),
                    }
                }
            }
        });
    }
    assert_pristine(&corpus)?;
    Ok(failures.into_inner())
}

/// Options of `run`.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Corpus ids, run in order into one report.
    pub corpora: Vec<String>,
    /// Which SANY verdict modules are compared on: full SANY, or the
    /// parse phase of the module's own file.
    pub sany_phase: SanyPhase,
    /// Oracle whose goldens are compared against.
    pub golden_oracle: String,
    /// Selection.
    pub selection: Selection,
    /// Worker counts.
    pub workers: Vec<u32>,
    /// Limits.
    pub limits: Limits,
    /// Compare goldens with the Examples manifests.
    pub crosscheck_manifest: bool,
    /// Compare model-checking goldens. Disable for module-only runs.
    pub models: bool,
    /// Also compare SANY goldens.
    pub sany: bool,
    /// Where to write the JSON report.
    pub report_dir: PathBuf,
    /// Where to write the Markdown summary, if anywhere.
    pub summary_path: Option<PathBuf>,
}

fn record_from(
    workers: u32,
    outcome: Outcome,
    result: Option<&crate::tlc::result::OracleResult>,
    diffs: Vec<crate::compare::FieldDiff>,
    elapsed: Duration,
    detail: Option<String>,
) -> RunRecord {
    RunRecord {
        workers,
        outcome,
        verdict: result.map(|r| r.verdict),
        verdict_code: result.and_then(|r| r.verdict_code),
        exit_code: result.and_then(|r| r.exit_code),
        distinct_states: result.and_then(|r| r.counts.distinct_states),
        states_generated: result.and_then(|r| r.counts.states_generated),
        depth: result.and_then(|r| r.counts.depth),
        trace_length: result.and_then(|r| r.trace.as_ref().map(|t| t.length)),
        diffs,
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        detail,
    }
}

/// The registries that can explain a difference, and the oracle under test.
struct Explainers<'a> {
    /// `conformance/expected-divergences.toml` (dive's `dive` profile only).
    divergences: &'a crate::divergence::Registry,
    /// `conformance/oracle-changes.toml` (TLC oracles only).
    changes: &'a crate::upstream::Registry,
    /// The oracle id when the implementation is a TLC oracle.
    implementation_oracle: Option<&'a str>,
}

impl Explainers<'_> {
    /// The registered upstream changes that explain `diffs`, joined for a
    /// report detail, or `None`.
    fn upstream(
        &self,
        subject: crate::upstream::Subject,
        path: &str,
        golden_oracle: &str,
        golden: &crate::upstream::ResultFacts,
        observed: &crate::upstream::ResultFacts,
        diffs: &[FieldDiff],
    ) -> Option<String> {
        let found = self.changes.explain(&crate::upstream::Comparison {
            subject,
            path,
            golden_oracle,
            implementation_oracle: self.implementation_oracle,
            golden,
            observed,
            diffs,
        });
        if found.is_empty() {
            return None;
        }
        Some(format!(
            "known upstream change: {}",
            found
                .iter()
                .map(|c| format!("`{}` ({})", c.id, c.upstream_commit))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Combines per-worker outcomes: the worst one wins, in the order
/// oracle-error, timeout, disagree, unsupported, agree.
fn combine(outcomes: impl Iterator<Item = Outcome>) -> Outcome {
    let rank = |o: Outcome| match o {
        Outcome::OracleError => 5,
        Outcome::Timeout => 4,
        Outcome::Disagree => 3,
        Outcome::Unsupported
        | Outcome::ExpectedDivergence
        | Outcome::KnownUpstreamChange
        | Outcome::AgreePendingTraceValidation => 2,
        Outcome::OutOfScope | Outcome::DivergenceMode => 1,
        Outcome::Agree => 0,
    };
    outcomes
        .max_by_key(|o| rank(*o))
        .unwrap_or(Outcome::OracleError)
}

#[allow(
    clippy::too_many_lines,
    reason = "the validation steps read best in order"
)]
fn run_model(
    imp: &dyn Implementation,
    corpus: &CorpusIndex,
    m: &ModelEntry,
    golden_root: &Path,
    golden_sha256: &str,
    explainers: &Explainers<'_>,
    opts: &RunOptions,
) -> ModelReport {
    let registry = explainers.divergences;
    let mut report = ModelReport {
        corpus: corpus.id.clone(),
        model: m.id.clone(),
        module: m.module.clone(),
        tier: m.tier,
        outcome: Outcome::OracleError,
        detail: None,
        golden_verdict: None,
        runs: Vec::new(),
        worker_invariance: None,
        manifest_drift: Vec::new(),
        worker_introspecting: m.worker_introspecting,
    };
    if let Some(reason) = &m.out_of_scope {
        report.outcome = Outcome::OutOfScope;
        report.detail = Some(reason.clone());
        return report;
    }
    let path = model_golden_path(golden_root, &opts.golden_oracle, &corpus.id, &m.id);
    if let Some(why) = imp.models_unsupported() {
        // The implementation checks no model at all; its refusal is the
        // answer, golden or not. A missing golden is still noted.
        report.outcome = Outcome::Unsupported;
        report.detail = Some(if path.exists() {
            why
        } else {
            format!("{why} (no golden from `{}` either)", opts.golden_oracle)
        });
        return report;
    }
    let golden = if path.exists() {
        match read_model_golden(&path) {
            Ok(g) => g,
            Err(e) => {
                report.detail = Some(e.to_string());
                return report;
            }
        }
    } else {
        report.detail = Some(format!(
            "no golden from `{}` for this model",
            opts.golden_oracle
        ));
        return report;
    };
    report.golden_verdict = Some(golden.result.verdict);
    if opts.crosscheck_manifest {
        report.manifest_drift =
            crosscheck_manifest(&m.expected, &golden.result, m.mode == ModelMode::Exhaustive);
    }
    if golden.provenance.corpus_commit != corpus.commit {
        report.detail = Some(format!(
            "golden was generated at corpus commit {}, but {} is pinned",
            golden.provenance.corpus_commit, corpus.commit
        ));
        return report;
    }
    if golden.provenance.oracle.sha256 != golden_sha256 {
        report.detail = Some(format!(
            "golden was generated by jar {}, but `{}` is pinned to {golden_sha256}",
            golden.provenance.oracle.sha256, opts.golden_oracle
        ));
        return report;
    }
    if golden.model != GoldenModel::from(m) {
        report.detail = Some(
            "golden describes a different model entry (manifest changed); regenerate".to_owned(),
        );
        return report;
    }
    if !golden.result.verdict.is_oracle_fact() {
        report.detail = Some(format!(
            "golden verdict is {}: {}",
            golden.result.verdict.as_str(),
            golden.result.note.clone().unwrap_or_default()
        ));
        return report;
    }
    let policy = ComparePolicy {
        oracle_leniency: imp.is_oracle(),
        may_omit_property: imp.is_oracle() && imp.omits_property_names(),
    };
    // Under `--compat=tlc`, `TLCGet("config").worker` is the physical
    // worker count, as in TLC (D-011), so a model that reads it may
    // legitimately behave as TLC does at that count: it is compared with
    // TLC's own run at the same count, on verdict, error class and exact
    // numeric exit (ADR 0004; ADR 0096 corrects the omitted numeric field).
    // The default profile reports the logical
    // count, 1, so the single-worker golden applies at every count there.
    let compat = !imp.is_oracle()
        && matches!(
            imp.profile(),
            Some(crate::divergence::Profile::Tlc | crate::divergence::Profile::TlcWithOverrides(_))
        );
    let mut points = Vec::new();
    for &workers in &opts.workers {
        let worker_golden = if compat && m.worker_introspecting && workers > 1 {
            let path =
                worker_golden_path(golden_root, &opts.golden_oracle, &corpus.id, &m.id, workers);
            path.exists().then(|| {
                usable_worker_golden(
                    &path,
                    corpus,
                    m,
                    &opts.golden_oracle,
                    golden_sha256,
                    workers,
                )
            })
        } else {
            None
        };
        if let Some(Err(why)) = &worker_golden {
            report.runs.push(record_from(
                workers,
                Outcome::OracleError,
                None,
                vec![],
                Duration::ZERO,
                Some(why.clone()),
            ));
            continue;
        }
        let worker_golden = worker_golden.and_then(std::result::Result::ok);
        let req = ModelRequest {
            corpus_root: &corpus.root,
            model: m,
            workers,
            timeout: opts
                .limits
                .timeout_for(m, Some(golden.provenance.elapsed_ms)),
            xmx: &opts.limits.xmx,
        };
        let rec = match imp.check_model(&req) {
            Err(e) => record_from(
                workers,
                Outcome::OracleError,
                None,
                vec![],
                Duration::ZERO,
                Some(e.to_string()),
            ),
            Ok(ImplResult::Unsupported(why)) => record_from(
                workers,
                Outcome::Unsupported,
                None,
                vec![],
                Duration::ZERO,
                Some(why),
            ),
            Ok(ImplResult::Completed {
                result, elapsed, ..
            }) => {
                // A run compared with TLC at its own worker count is not
                // part of the worker-invariance judgement: there the
                // variance is TLC's own.
                if worker_golden.is_none() {
                    points.push(WorkerPoint {
                        workers,
                        verdict: result.verdict,
                        distinct_states: result.counts.distinct_states,
                        states_generated: result.counts.states_generated,
                        depth: result.counts.depth,
                        violated_property: result.violated_property.clone(),
                    });
                }
                let ctx = CompareContext {
                    workers,
                    counts_comparable: m.counts_comparable,
                };
                let mut diffs = match &worker_golden {
                    Some(g) => diff_verdict_and_class(&g.result, &result),
                    None => diff_results_with(&golden.result, &result, ctx, policy),
                };
                let mut trace_note = match &worker_golden {
                    Some(_) => Some(format!(
                        "compared with TLC at {workers} workers on verdict, error class and numeric exit (the model reads TLCGet(\"config\"); ADR 0004)"
                    )),
                    None if compat && m.worker_introspecting && workers > 1 => Some(format!(
                        "no worker-specific golden at {workers} workers; compared with the single-worker golden"
                    )),
                    None => None,
                };
                let validation_golden = worker_golden.as_ref().unwrap_or(&golden);
                let requires_word = matches!(
                    result.verdict,
                    Verdict::SafetyViolation
                        | Verdict::ActionPropertyViolation
                        | Verdict::LivenessViolation
                        | Verdict::Deadlock
                ) || (result.verdict == validation_golden.result.verdict
                    && validation_golden.result.trace.is_some());
                if !imp.is_oracle() && (result.trace.is_some() || requires_word) {
                    // Charter rule 2: a counterexample must be independently
                    // replayed before it can count. The harness validates it
                    // itself (ADR 0016 §3).
                    // A validation claim cannot supply an absent word. Keep
                    // payload presence format-neutral: JSON and text steps
                    // both go through the independent validation hook.
                    let check = match &result.trace {
                        None => crate::trace_check::TraceCheck::no(
                            "the verdict requires a counterexample, but the result carries none",
                        ),
                        Some(t)
                            if t.json.as_ref().is_none_or(serde_json::Value::is_null)
                                && t.steps.as_ref().is_none_or(Vec::is_empty) =>
                        {
                            crate::trace_check::TraceCheck::no(
                                "the counterexample carries no word payload",
                            )
                        }
                        Some(_) => imp.trace_validated(&req, &result, &validation_golden.result),
                    };
                    if !check.validated {
                        // Unconfirmed (refused by name) can at best be
                        // pending; shown wrong is a disagreement.
                        diffs.push(FieldDiff {
                            field: if check.refuted {
                                "trace.invalid"
                            } else {
                                "trace.validated"
                            }
                            .to_owned(),
                            expected: true.into(),
                            actual: false.into(),
                        });
                        let note = format!("trace not validated: {}", check.detail);
                        trace_note = Some(match trace_note {
                            Some(n) => format!("{n}; {note}"),
                            None => note,
                        });
                    }
                }
                let outcome = match result.verdict {
                    Verdict::Timeout => Outcome::Timeout,
                    Verdict::OracleError | Verdict::Unclassified | Verdict::ResourceLimit
                        if imp.is_oracle() =>
                    {
                        Outcome::OracleError
                    }
                    _ if imp.options().iter().any(|o| o.result_changing) => Outcome::DivergenceMode,
                    _ if diffs.is_empty() => Outcome::Agree,
                    // Everything compared agrees; only the independent
                    // validation could not confirm the counterexample.
                    // Not an agreement until the validator vouches for it.
                    _ if diffs.iter().all(|d| d.field == "trace.validated") => {
                        Outcome::AgreePendingTraceValidation
                    }
                    _ => Outcome::Disagree,
                };
                let mut detail = match (result.note.clone(), trace_note) {
                    (Some(n), Some(t)) => Some(format!("{n}; {t}")),
                    (n, t) => n.or(t),
                };
                let excused = if outcome == Outcome::Disagree && !imp.is_oracle() {
                    let profile = imp.profile();
                    let active: Vec<String> = imp
                        .options()
                        .iter()
                        .map(|o| {
                            o.value
                                .as_ref()
                                .map_or_else(|| o.name.clone(), |v| format!("{}={v}", o.name))
                        })
                        .collect();
                    registry
                        .excuse(profile.as_ref(), &active, &m.id, result.verdict, &diffs)
                        .or_else(|| {
                            registry.excuse(
                                profile.as_ref(),
                                &active,
                                &m.module,
                                result.verdict,
                                &diffs,
                            )
                        })
                } else {
                    None
                };
                let upstream = if outcome == Outcome::Disagree && imp.is_oracle() {
                    explainers.upstream(
                        crate::upstream::Subject::Model,
                        &m.id,
                        &opts.golden_oracle,
                        &(&golden.result).into(),
                        &(&result).into(),
                        &diffs,
                    )
                } else {
                    None
                };
                let outcome = if let Some(d) = excused {
                    detail = Some(format!("expected divergence `{}` ({})", d.id, d.link));
                    Outcome::ExpectedDivergence
                } else if let Some(u) = upstream {
                    detail = Some(u);
                    Outcome::KnownUpstreamChange
                } else {
                    outcome
                };
                record_from(workers, outcome, Some(&result), diffs, elapsed, detail)
            }
        };
        report.runs.push(rec);
    }
    // For TLC itself the model's outcome is its single-worker comparison;
    // multi-worker oracle runs are TLC's own invariance and are reported in
    // `worker_invariance` (ADR 0004 amendment). Every other implementation
    // must agree at every worker count.
    let primary: Vec<&RunRecord> = if imp.is_oracle() && report.runs.iter().any(|r| r.workers == 1)
    {
        report.runs.iter().filter(|r| r.workers == 1).collect()
    } else {
        report.runs.iter().collect()
    };
    report.outcome = combine(primary.iter().map(|r| r.outcome));
    report.detail = primary
        .iter()
        .find(|r| r.outcome == report.outcome)
        .and_then(|r| r.detail.clone());
    if opts.workers.len() > 1 && points.len() > 1 {
        let inv = worker_invariance(points);
        let varies = !inv.verdict_invariant
            || inv.counts_invariant == Some(false)
            || inv.depth_varies
            || inv.property_varies;
        if !imp.is_oracle()
            && varies
            && matches!(
                report.outcome,
                Outcome::Agree | Outcome::AgreePendingTraceValidation
            )
        {
            report.outcome = Outcome::Disagree;
            report.detail = Some("result varies with the worker count".to_owned());
        }
        report.worker_invariance = Some(inv);
    }
    report
}

/// Reads and checks the worker-specific golden at `path`. `Err` carries the
/// detail of an `oracle-error` outcome.
fn usable_worker_golden(
    path: &Path,
    corpus: &CorpusIndex,
    m: &ModelEntry,
    oracle_id: &str,
    golden_sha256: &str,
    workers: u32,
) -> std::result::Result<ModelGolden, String> {
    let g = read_model_golden(path).map_err(|e| e.to_string())?;
    let shown = path.display();
    if g.oracle != oracle_id || g.corpus != corpus.id {
        return Err(format!("{shown} names a different oracle or corpus"));
    }
    if g.workers != workers {
        return Err(format!(
            "{shown} records -workers {}, not {workers}",
            g.workers
        ));
    }
    if g.provenance.corpus_commit != corpus.commit {
        return Err(format!(
            "{shown} was generated at corpus commit {}, but {} is pinned",
            g.provenance.corpus_commit, corpus.commit
        ));
    }
    if g.provenance.oracle.sha256 != golden_sha256 {
        return Err(format!(
            "{shown} was generated by jar {}, but {golden_sha256} is pinned",
            g.provenance.oracle.sha256
        ));
    }
    if g.provenance
        .reason
        .as_deref()
        .is_none_or(|r| r.trim().is_empty())
    {
        return Err(format!("{shown} records no --reason"));
    }
    if g.model != GoldenModel::from(m) {
        return Err(format!(
            "{shown} describes a different model entry (manifest changed); regenerate"
        ));
    }
    if !g.result.verdict.is_oracle_fact() {
        return Err(format!(
            "{shown}: golden verdict is {}",
            g.result.verdict.as_str()
        ));
    }
    Ok(g)
}

/// Reads and checks the SANY golden of `module`. `Err` carries the detail
/// of an `oracle-error` outcome.
fn usable_sany_golden(
    corpus: &CorpusIndex,
    module: &str,
    golden_root: &Path,
    golden_sha256: &str,
    opts: &RunOptions,
) -> std::result::Result<SanyGolden, String> {
    let path = sany_golden_path(golden_root, &opts.golden_oracle, &corpus.id, module);
    if !path.exists() {
        return Err(format!("no SANY golden from `{}`", opts.golden_oracle));
    }
    let golden = read_sany_golden(&path).map_err(|e| e.to_string())?;
    if golden.provenance.corpus_commit != corpus.commit {
        return Err("SANY golden was generated at another corpus commit".to_owned());
    }
    if golden.provenance.oracle.sha256 != golden_sha256 {
        return Err(format!(
            "SANY golden was generated by jar {}, but `{}` is pinned to {golden_sha256}",
            golden.provenance.oracle.sha256, opts.golden_oracle
        ));
    }
    if matches!(
        golden.result.verdict,
        SanyVerdict::Unclassified | SanyVerdict::Timeout
    ) {
        return Err(format!(
            "golden is not a usable oracle fact: {:?}",
            golden.result.note
        ));
    }
    Ok(golden)
}

/// `--sany-phase parse`: compares the implementation's parse-only verdict
/// with the parse-phase verdict derived from the full SANY golden.
fn run_parse_phase(
    imp: &dyn Implementation,
    corpus: &CorpusIndex,
    module: &str,
    golden: &SanyGolden,
    result_changing: bool,
    opts: &RunOptions,
    r: &mut ModuleReport,
) {
    let expected = match crate::sany::parse_phase(&golden.result, module) {
        Ok(v) => v,
        Err(why) => {
            r.detail = Some(format!("golden: {why}"));
            return;
        }
    };
    r.golden_parse_verdict = Some(expected);
    match imp.parse_only(&corpus.root, module, opts.limits.sany_timeout()) {
        Err(e) => r.detail = Some(e.to_string()),
        Ok(ImplResult::Unsupported(why)) => {
            r.outcome = Outcome::Unsupported;
            r.detail = Some(why);
        }
        Ok(ImplResult::Completed { result, .. }) => {
            r.parse_verdict = result.verdict;
            if result.verdict != Some(expected) {
                r.diffs.push(FieldDiff {
                    field: "parse_verdict".to_owned(),
                    expected: serde_json::to_value(expected).unwrap_or_default(),
                    actual: serde_json::to_value(result.verdict).unwrap_or_default(),
                });
            }
            r.outcome = if result.timed_out {
                Outcome::Timeout
            } else if result.verdict.is_none() && imp.is_oracle() {
                Outcome::OracleError
            } else if result_changing {
                Outcome::DivergenceMode
            } else if r.diffs.is_empty() {
                Outcome::Agree
            } else {
                Outcome::Disagree
            };
            r.detail = result.note;
        }
    }
}

fn run_module(
    imp: &dyn Implementation,
    corpus: &CorpusIndex,
    module: &ModuleEntry,
    golden_root: &Path,
    golden_sha256: &str,
    explainers: &Explainers<'_>,
    opts: &RunOptions,
) -> ModuleReport {
    let mut r = ModuleReport {
        corpus: corpus.id.clone(),
        module: module.path.clone(),
        outcome: Outcome::OracleError,
        golden_verdict: None,
        verdict: None,
        phase: opts.sany_phase,
        golden_parse_verdict: None,
        parse_verdict: None,
        diffs: Vec::new(),
        detail: None,
    };
    if let Some(reason) = &module.out_of_scope {
        r.outcome = Outcome::OutOfScope;
        r.detail = Some(reason.clone());
        return r;
    }
    let module = module.path.as_str();
    let golden = match usable_sany_golden(corpus, module, golden_root, golden_sha256, opts) {
        Ok(g) => g,
        Err(detail) => {
            r.golden_verdict = None;
            r.detail = Some(detail);
            return r;
        }
    };
    r.golden_verdict = Some(golden.result.verdict);
    let result_changing = !imp.is_oracle() && imp.options().iter().any(|o| o.result_changing);
    if opts.sany_phase == SanyPhase::Parse {
        run_parse_phase(imp, corpus, module, &golden, result_changing, opts, &mut r);
        return r;
    }
    match imp.parse_module(&corpus.root, module, opts.limits.sany_timeout()) {
        Err(e) => r.detail = Some(e.to_string()),
        Ok(ImplResult::Unsupported(why)) => {
            r.outcome = Outcome::Unsupported;
            r.detail = Some(why);
        }
        Ok(ImplResult::Completed { result, .. }) => {
            r.verdict = Some(result.verdict);
            r.diffs = diff_sany(&golden.result, &result);
            r.outcome = match result.verdict {
                SanyVerdict::Timeout => Outcome::Timeout,
                SanyVerdict::Unclassified if imp.is_oracle() => Outcome::OracleError,
                _ if result_changing => Outcome::DivergenceMode,
                _ if r.diffs.is_empty() => Outcome::Agree,
                _ => Outcome::Disagree,
            };
            r.detail.clone_from(&result.note);
            if r.outcome == Outcome::Disagree && r.detail.is_none() {
                r.detail.clone_from(&result.error_text);
            }
            if r.outcome == Outcome::Disagree
                && imp.is_oracle()
                && let Some(u) = explainers.upstream(
                    crate::upstream::Subject::Module,
                    module,
                    &opts.golden_oracle,
                    &(&golden.result).into(),
                    &(&result).into(),
                    &r.diffs,
                )
            {
                r.outcome = Outcome::KnownUpstreamChange;
                r.detail = Some(u);
            }
        }
    }
    r
}

/// `run`: runs `imp` against the goldens, writes the report and summary, and
/// returns the report.
///
/// # Errors
/// Fails if the corpus cannot be loaded or the report cannot be written.
#[allow(
    clippy::too_many_lines,
    reason = "one linear pass: models, modules, then the report"
)]
pub fn run(
    config: &Config,
    cache: &Cache,
    imp: &dyn Implementation,
    imp_oracle: Option<&OracleEnv>,
    opts: &RunOptions,
) -> Result<Report> {
    if opts.corpora.is_empty() {
        return Err(Error::Usage("run needs at least one corpus".to_owned()));
    }
    let corpora = opts
        .corpora
        .iter()
        .map(|id| load_corpus(config, cache, id))
        .collect::<Result<Vec<_>>>()?;
    let golden_oracle = config.oracles.oracle(&opts.golden_oracle)?;
    let registry = crate::divergence::Registry::load(&config.repo_root)?;
    let changes = crate::upstream::Registry::load(&config.repo_root, &config.oracles)?;
    let explainers = Explainers {
        divergences: &registry,
        changes: &changes,
        implementation_oracle: if imp.is_oracle() {
            imp_oracle.map(|e| e.oracle.id.as_str())
        } else {
            None
        },
    };
    let golden_role = golden_oracle.role;
    let implementation_role = imp_oracle.map(|e| e.oracle.role);
    let root = golden_root(&config.repo_root);
    let mut selected = Vec::new();
    for corpus in &corpora {
        let models = if opts.models {
            opts.selection.models(corpus)?
        } else {
            Vec::new()
        };
        let modules = if opts.sany {
            opts.selection.modules(corpus)?
        } else {
            Vec::new()
        };
        selected.push((corpus, models, modules));
    }
    let models_selected: usize = selected.iter().map(|s| s.1.len()).sum();
    let modules_selected: usize = selected.iter().map(|s| s.2.len()).sum();
    if models_selected == 0 && modules_selected == 0 {
        return Err(Error::Usage(
            "the selection is empty: no model or module matches the tier and filters".to_owned(),
        ));
    }
    let mut model_reports = Vec::new();
    let mut module_reports = Vec::new();
    for (corpus, models, modules) in &selected {
        let total = models.len();
        log(&format!(
            "running {} against {} golden(s) of {} on {total} model(s) and {} module(s), workers {:?}, SANY phase {:?}",
            imp.name(),
            corpus.id,
            opts.golden_oracle,
            modules.len(),
            opts.workers,
            opts.sany_phase
        ));
        let done = AtomicUsize::new(0);
        model_reports.extend(parallel_map(models, opts.limits.jobs, |_, m| {
            let r = run_model(
                imp,
                corpus,
                m,
                &root,
                &golden_oracle.sha256,
                &explainers,
                opts,
            );
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            log(&format!(
                "[{n}/{total}] {:<13} {}{}",
                r.outcome.as_str(),
                m.id,
                r.detail
                    .as_deref()
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default()
            ));
            r
        }));
        let total = modules.len();
        let done = AtomicUsize::new(0);
        module_reports.extend(parallel_map(modules, opts.limits.jobs, |_, module| {
            let r = run_module(
                imp,
                corpus,
                module,
                &root,
                &golden_oracle.sha256,
                &explainers,
                opts,
            );
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            log(&format!(
                "[{n}/{total}] SANY {:<13} {}",
                r.outcome.as_str(),
                module.path
            ));
            r
        }));
        // Every implementation, oracle or not, must leave the corpus as it
        // found it.
        assert_pristine(corpus)?;
    }
    model_reports.sort_by(|a, b| (&a.corpus, &a.model).cmp(&(&b.corpus, &b.model)));
    module_reports.sort_by(|a, b| (&a.corpus, &a.module).cmp(&(&b.corpus, &b.module)));
    let now = unix_now();
    let impl_slug: String = imp
        .name()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let report = Report {
        schema_version: crate::report::REPORT_SCHEMA,
        run_id: unique_run_id(
            &opts.report_dir,
            &match opts.sany_phase {
                SanyPhase::Full => impl_slug,
                SanyPhase::Parse => format!("{impl_slug}-parse"),
            },
            &opts.selection.tier,
        ),
        generated_at: rfc3339(now),
        generator: generator(),
        implementation: imp.name(),
        implementation_oracle: imp_oracle.map(OracleEnv::provenance),
        golden_oracle: golden_oracle.id.clone(),
        golden_oracle_sha256: golden_oracle.sha256.clone(),
        golden_oracle_role: Some(golden_role),
        implementation_oracle_role: implementation_role,
        reference: golden_role == crate::config::Role::Reference
            || implementation_role == Some(crate::config::Role::Reference),
        implementation_dive: imp.dive_provenance(),
        corpora: corpora
            .iter()
            .map(|corpus| CorpusInfo {
                id: corpus.id.clone(),
                commit: corpus.commit.clone(),
                unfetched_submodules: corpus.unfetched_submodules.clone(),
                uncovered: corpus.uncovered.clone(),
            })
            .collect(),
        host: Host::current(),
        settings: Settings {
            tier: opts.selection.tier.clone(),
            filters: opts.selection.filters.clone(),
            workers: opts.workers.clone(),
            jobs: opts.limits.jobs,
            xmx: opts.limits.xmx.clone(),
            timeout_policy: opts.limits.policy(),
            crosscheck_manifest: opts.crosscheck_manifest,
            sany_phase: opts.sany_phase,
            impl_options: imp.options().to_vec(),
            impl_profile: imp.profile(),
            strict_tier: imp
                .profile()
                .map(|p| p.is_strict() && !imp.options().iter().any(|o| o.result_changing)),
        },
        summary: summarise(
            &model_reports,
            &module_reports,
            models_selected,
            modules_selected,
        )?,
        models: model_reports,
        modules: module_reports,
    };
    let path = opts.report_dir.join(format!("{}.json", report.run_id));
    write_atomic(&path, to_pretty(&report)?.as_bytes())?;
    log(&format!("report: {}", path.display()));
    if let Some(md) = &opts.summary_path {
        let existing = std::fs::read_to_string(md).ok();
        let merged = crate::report::merge_summary(
            existing.as_deref(),
            &crate::report::summary_key(&report),
            &render_markdown(&report)?,
        );
        write_atomic(md, merged.as_bytes())?;
        log(&format!("summary: {}", md.display()));
    }
    Ok(report)
}

/// A run id that is unique in `report_dir`: UTC time to the millisecond,
/// the implementation and the tier, with a numeric suffix if a report of
/// that name already exists.
fn unique_run_id(report_dir: &Path, impl_slug: &str, tier: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let base = format!(
        "{}{:03}-{impl_slug}-{tier}",
        compact_stamp(now.as_secs()).trim_end_matches('Z'),
        now.subsec_millis()
    );
    let mut id = base.clone();
    let mut n = 1;
    while report_dir.join(format!("{id}.json")).exists() {
        n += 1;
        id = format!("{base}-{n}");
    }
    id
}

/// `corpus list`: prints the selected models and their classification.
///
/// # Errors
/// Fails if the corpus cannot be loaded.
pub fn corpus_list(
    config: &Config,
    cache: &Cache,
    id: &str,
    selection: &Selection,
) -> Result<String> {
    let c = config.corpora.corpus(id)?;
    if c.layout == Layout::Tbd {
        return Ok(format!(
            "corpus `{id}`: layout `tbd` is not yet supported\n"
        ));
    }
    let corpus = load_corpus(config, cache, id)?;
    let mut s = String::new();
    for m in selection.models(&corpus)? {
        let _ = writeln!(
            s,
            "{:<6} {:<12} {:<18} {}{}",
            format!("{:?}", m.tier).to_lowercase(),
            match m.mode {
                ModelMode::Exhaustive => "exhaustive".to_owned(),
                ModelMode::Simulate { traces } => format!("simulate:{traces}"),
                ModelMode::Generate => "generate".to_owned(),
                ModelMode::Symbolic => "symbolic".to_owned(),
            },
            m.expected.result,
            m.id,
            m.out_of_scope
                .as_deref()
                .map(|r| format!("  [out-of-scope: {r}]"))
                .unwrap_or_default()
        );
    }
    for m in corpus.modules.iter().filter(|m| m.out_of_scope.is_some()) {
        let _ = writeln!(
            s,
            "module {}  [out-of-scope: {}]",
            m.path,
            m.out_of_scope.as_deref().unwrap_or_default()
        );
    }
    let _ = writeln!(
        s,
        "{} module(s), {} model(s) in corpus `{id}` @ {}; unfetched submodules: {}",
        corpus.modules.len(),
        corpus.models.len(),
        corpus.commit,
        if corpus.unfetched_submodules.is_empty() {
            "none".to_owned()
        } else {
            corpus.unfetched_submodules.join(", ")
        }
    );
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_map_preserves_order() {
        let items: Vec<u32> = (0..50).collect();
        let out = parallel_map(&items, 4, |i, x| (i, x * 2));
        assert_eq!(out.len(), 50);
        assert!(
            out.iter()
                .enumerate()
                .all(|(i, (j, v))| i == *j && *v == 2 * u32::try_from(i).unwrap_or(0))
        );
    }

    #[test]
    fn combine_prefers_worst() {
        assert_eq!(
            combine([Outcome::Agree, Outcome::Disagree].into_iter()),
            Outcome::Disagree
        );
        assert_eq!(
            combine([Outcome::Timeout, Outcome::Disagree].into_iter()),
            Outcome::Timeout
        );
        assert_eq!(combine(std::iter::empty()), Outcome::OracleError);
    }
}
