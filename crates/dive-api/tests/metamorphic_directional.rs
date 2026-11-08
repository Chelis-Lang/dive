//! Directional metamorphic tests: changes to a model whose effect on the
//! result is known in direction, though not in value.
//!
//! - *depth-bound*: `CONSTRAINT TLCGet("level") < k` only removes states, so
//!   the distinct count never decreases as `k` grows and reaches the
//!   unbounded count; a violation, once present, keeps the unbounded depth.
//! - *depth-invariant*: on a model that passes, `INVARIANT TLCGet("level") <
//!   k` fails with a counterexample one state longer for each larger `k`,
//!   until `k` passes the model's depth and the model passes again.
//! - *drop-invariant*: replacing one configured invariant with `TRUE` keeps
//!   a passing model passing with the same counts; a violation of another
//!   invariant stays at its depth; dropping the violated invariant finds no
//!   violation shallower than before.
//!
//! Each change is made in a wrapper module that extends the seed's root
//! module, `TLC` and `Naturals`, with an edited copy of the configuration. A seed whose
//! unchanged wrapper does not reproduce the seed's own report is skipped, so
//! the wrapper itself never causes a finding. Runs use the default profile,
//! seed 0, fp 0 and one worker, and stop at 20,000 states or ten seconds.
//!
//! The fast test checks a fixed seed list. `DIVE_DIRECTIONAL=all` checks
//! every fixture, and `DIVE_DIRECTIONAL_REPORT=<path>` writes a JSON
//! summary of that sweep.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Seeds for the fast test: passing and violated models, and models with
/// several invariants.
const FAST_SEEDS: &[&str] = &[
    "crates/dive-conformance/tests/fixtures/specs/Success/Success.tla",
    "crates/dive-conformance/tests/fixtures/specs/InvariantViolation/InvariantViolation.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/DieHard/DieHard.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/transaction_commit/TCommit.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/CigaretteSmokers/CigaretteSmokers.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErInvReplayViolation/ErInvReplayViolation.tla",
];

/// States explored per run.
const MAX_STATES: u64 = 20_000;
/// Wall time per run.
const TIME_LIMIT: Duration = Duration::from_secs(10);
/// The most bounds tried per relation and seed.
const MAX_BOUNDS: u64 = 8;
/// The most invariants dropped per seed.
const MAX_DROPPED: usize = 4;

/// Seeds whose own run takes longer are skipped: each costs a dozen runs,
/// and the memory budget measures the whole test process.
const MAX_BASE_MS: u64 = 1_000;

/// The wrapper module's name.
const WRAPPER: &str = "ZqDirWrap";

// ---------------------------------------------------------------------------
// Scratch directories

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "dive-directional-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}

// ---------------------------------------------------------------------------
// Runs

fn run(file: &Path, config: &Path) -> Result<Value> {
    let report = dive_api::check_file_engine(
        &CheckRequest {
            file: file.to_path_buf(),
            config: Some(config.to_path_buf()),
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            max_states: Some(MAX_STATES),
            time_limit: BudgetSetting::Explicit(TIME_LIMIT),
            ..CheckRequest::default()
        },
        dive_api::Engine::Core,
    )?;
    Ok(report.to_json())
}

struct Seed {
    dir: PathBuf,
    root: String,
    cfg_text: String,
    cfg: dive_config::ModelConfig,
}

impl Seed {
    /// Checks the wrapper with `definitions` added and configuration `cfg`.
    fn wrapped(&self, definitions: &str, cfg: &str) -> Result<Value> {
        let into = scratch()?;
        for entry in std::fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.is_file()
                && let Some(name) = path.file_name()
            {
                std::fs::copy(&path, into.0.join(name))?;
            }
        }
        let module = format!(
            "---- MODULE {WRAPPER} ----\nEXTENDS {}, TLC, Naturals\n{definitions}\n====\n",
            self.root
        );
        let tla = into.0.join(format!("{WRAPPER}.tla"));
        let cfg_path = into.0.join(format!("{WRAPPER}.cfg"));
        std::fs::write(&tla, module)?;
        std::fs::write(&cfg_path, cfg)?;
        run(&tla, &cfg_path)
    }
}

fn verdict(doc: &Value) -> &str {
    doc["verdict"].as_str().unwrap_or_default()
}
fn stat(doc: &Value, name: &str) -> Option<u64> {
    doc["statistics"][name].as_u64()
}
fn stopped(doc: &Value) -> bool {
    verdict(doc) == "stopped" || verdict(doc) == "resource-limit"
}
fn length(doc: &Value) -> Option<u64> {
    doc["counterexample"]["length"].as_u64()
}

/// What the wrapper must reproduce for a seed to be used.
fn observed(doc: &Value) -> Value {
    json!({
        "verdict": doc["verdict"], "property": doc["property"],
        "statistics": doc["statistics"], "length": doc["counterexample"]["length"],
    })
}

/// One bound's result: `(k, verdict, distinct, counterexample length)`.
fn summary_row(k: u64, doc: &Value) -> String {
    format!(
        "k={k}: {} distinct={:?} length={:?}",
        verdict(doc),
        stat(doc, "distinct"),
        length(doc)
    )
}

// ---------------------------------------------------------------------------
// Relations

enum Outcome {
    Holds,
    Inconclusive(String),
    Fails(String),
}

/// Bounds `1..=min(depth + 2, MAX_BOUNDS)`, and `depth + 2` itself.
fn bounds(depth: u64) -> Vec<u64> {
    let mut ks: Vec<u64> = (1..=(depth + 2).min(MAX_BOUNDS)).collect();
    if !ks.contains(&(depth + 2)) {
        ks.push(depth + 2);
    }
    ks
}

fn depth_bound(seed: &Seed, base: &Value) -> Result<Outcome> {
    let Some(depth) = stat(base, "depth") else {
        return Ok(Outcome::Inconclusive("no depth".into()));
    };
    let mut rows = Vec::new();
    for k in bounds(depth) {
        let doc = seed.wrapped(
            &format!("ZqDirBound == TLCGet(\"level\") < {k}"),
            &format!("{}\nCONSTRAINT ZqDirBound\n", seed.cfg_text),
        )?;
        if stopped(&doc) {
            return Ok(Outcome::Inconclusive("a bounded run stopped".into()));
        }
        // dive may refuse a result it cannot reproduce exactly (D-002).
        if verdict(&doc) == "unsupported" {
            return Ok(Outcome::Inconclusive("a bounded run is unsupported".into()));
        }
        rows.push((k, doc));
    }
    let table = || {
        rows.iter()
            .map(|(k, d)| summary_row(*k, d))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let base_verdict = verdict(base);
    if let Some((k, doc)) = rows
        .iter()
        .find(|(_, d)| verdict(d) != "success" && verdict(d) != base_verdict)
    {
        return Ok(Outcome::Fails(format!(
            "k={k} reports {} for a {base_verdict} model: {}",
            verdict(doc),
            table()
        )));
    }
    if base_verdict == "success" {
        let distinct: Vec<Option<u64>> = rows.iter().map(|(_, d)| stat(d, "distinct")).collect();
        if distinct.windows(2).any(|w| w[0] > w[1]) {
            return Ok(Outcome::Fails(format!(
                "distinct decreases as k grows: {}",
                table()
            )));
        }
        if distinct.last().copied().flatten() != stat(base, "distinct") {
            return Ok(Outcome::Fails(format!(
                "the largest bound does not reach the unbounded {:?} distinct: {}",
                stat(base, "distinct"),
                table()
            )));
        }
        return Ok(Outcome::Holds);
    }
    // An invariant violation: once found, at the unbounded depth.
    let violated: Vec<bool> = rows
        .iter()
        .map(|(_, d)| verdict(d) == base_verdict)
        .collect();
    if violated.windows(2).any(|w| w[0] && !w[1]) || violated.last() != Some(&true) {
        return Ok(Outcome::Fails(format!(
            "the violation is not found for every bound past the first that finds it: {}",
            table()
        )));
    }
    if let Some((k, _)) = rows
        .iter()
        .find(|(_, d)| verdict(d) == base_verdict && length(d) != length(base))
    {
        return Ok(Outcome::Fails(format!(
            "k={k} finds the violation at another depth than the unbounded {:?}: {}",
            length(base),
            table()
        )));
    }
    Ok(Outcome::Holds)
}

fn depth_invariant(seed: &Seed, base: &Value) -> Result<Outcome> {
    let Some(depth) = stat(base, "depth") else {
        return Ok(Outcome::Inconclusive("no depth".into()));
    };
    let mut rows = Vec::new();
    for k in bounds(depth) {
        let doc = seed.wrapped(
            &format!("ZqDirLevel == TLCGet(\"level\") < {k}"),
            &format!("{}\nINVARIANT ZqDirLevel\n", seed.cfg_text),
        )?;
        if stopped(&doc) {
            return Ok(Outcome::Inconclusive("a bounded run stopped".into()));
        }
        // dive may refuse a result it cannot reproduce exactly (D-002).
        if verdict(&doc) == "unsupported" {
            return Ok(Outcome::Inconclusive("a bounded run is unsupported".into()));
        }
        rows.push((k, doc));
    }
    let table = || {
        rows.iter()
            .map(|(k, d)| summary_row(*k, d))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let fails = |d: &Value| verdict(d) == "invariant-violation" && d["property"] == "ZqDirLevel";
    if let Some((k, doc)) = rows
        .iter()
        .find(|(_, d)| !fails(d) && verdict(d) != "success")
    {
        return Ok(Outcome::Fails(format!(
            "k={k} reports {}: {}",
            verdict(doc),
            table()
        )));
    }
    let violated: Vec<bool> = rows.iter().map(|(_, d)| fails(d)).collect();
    if violated.windows(2).any(|w| !w[0] && w[1]) {
        return Ok(Outcome::Fails(format!(
            "the level invariant fails at a bound above one where it holds: {}",
            table()
        )));
    }
    for pair in rows.windows(2) {
        let ((k0, a), (k1, b)) = (&pair[0], &pair[1]);
        if fails(a) && fails(b) && k1 - k0 == 1 && length(b) != length(a).map(|l| l + 1) {
            return Ok(Outcome::Fails(format!(
                "the counterexample does not grow by one state from k={k0} to k={k1}: {}",
                table()
            )));
        }
    }
    if let Some((k, _)) = rows
        .iter()
        .find(|(_, d)| verdict(d) == "success" && stat(d, "distinct") != stat(base, "distinct"))
    {
        return Ok(Outcome::Fails(format!(
            "k={k} passes with other counts than the model: {}",
            table()
        )));
    }
    if rows.last().is_some_and(|(_, d)| fails(d)) {
        return Ok(Outcome::Fails(format!(
            "the level invariant still fails past the model's depth {depth}: {}",
            table()
        )));
    }
    Ok(Outcome::Holds)
}

fn drop_invariant(seed: &Seed, base: &Value) -> Result<Vec<(String, Outcome)>> {
    let mut names: Vec<String> = seed.cfg.invariants.iter().map(|n| n.text.clone()).collect();
    names.dedup();
    let mut outcomes = Vec::new();
    for dropped in names.iter().take(MAX_DROPPED) {
        let mut cfg = seed.cfg_text.clone();
        let mut spans: Vec<_> = seed
            .cfg
            .invariants
            .iter()
            .filter(|n| &n.text == dropped)
            .map(|n| (usize::from(n.span.start()), usize::from(n.span.end())))
            .collect();
        spans.sort_unstable();
        for (start, end) in spans.into_iter().rev() {
            cfg.replace_range(start..end, "ZqDirTrue");
        }
        let doc = seed.wrapped("ZqDirTrue == TRUE", &cfg)?;
        let shown = || {
            format!(
                "base {}, without {dropped}: {}",
                observed(base),
                observed(&doc)
            )
        };
        let outcome = if stopped(&doc) || verdict(&doc) == "unsupported" {
            Outcome::Inconclusive("the run without an invariant stopped or is unsupported".into())
        } else if verdict(base) == "success" {
            if observed(&doc) == observed(base) {
                Outcome::Holds
            } else {
                Outcome::Fails(shown())
            }
        } else if base["property"] != dropped.as_str() {
            let same = verdict(&doc) == verdict(base) && length(&doc) == length(base);
            if same && doc["property"] != dropped.as_str() {
                Outcome::Holds
            } else {
                Outcome::Fails(shown())
            }
        } else {
            // Another invariant, or an error, may now end the search, but
            // never at a smaller depth.
            let later = verdict(&doc) == "success"
                || (length(&doc).is_some() && length(&doc) >= length(base));
            if later && stat(&doc, "distinct") >= stat(base, "distinct") {
                Outcome::Holds
            } else {
                Outcome::Fails(shown())
            }
        };
        outcomes.push((dropped.clone(), outcome));
    }
    Ok(outcomes)
}

// ---------------------------------------------------------------------------
// Seeds and the sweep

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every fixture root module with a same-named configuration beside it.
fn all_seeds(workspace: &Path) -> Vec<String> {
    let mut seeds = Vec::new();
    let mut stack = vec![
        workspace.join("crates"),
        workspace.join("conformance/corpus"),
    ];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|n| n != "target" && n != "src")
                {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "tla")
                && path.with_extension("cfg").is_file()
                && let Ok(rel) = path.strip_prefix(workspace)
            {
                seeds.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    seeds.sort();
    seeds
}

#[derive(Default)]
struct Tally {
    seeds: usize,
    skipped: BTreeMap<String, usize>,
    holds: BTreeMap<&'static str, usize>,
    inconclusive: BTreeMap<String, usize>,
    wrapper_differs: Vec<Value>,
    failures: Vec<Value>,
}

impl Tally {
    fn record(&mut self, seed: &str, relation: &'static str, outcome: Outcome) {
        match outcome {
            Outcome::Holds => *self.holds.entry(relation).or_default() += 1,
            Outcome::Inconclusive(why) => {
                *self
                    .inconclusive
                    .entry(format!("{relation}: {why}"))
                    .or_default() += 1;
            }
            Outcome::Fails(detail) => self.failures.push(json!({
                "seed": seed, "relation": relation, "detail": detail,
            })),
        }
    }
}

fn check_seed(workspace: &Path, rel: &str, tally: &mut Tally) -> Result<()> {
    let tla = workspace.join(rel);
    let (Some(dir), Some(root)) = (tla.parent(), tla.file_stem().and_then(|s| s.to_str())) else {
        return Ok(());
    };
    let mut skip = |why: &str| *tally.skipped.entry(why.to_owned()).or_default() += 1;
    let cfg_text = std::fs::read_to_string(tla.with_extension("cfg"))?;
    let Some(cfg) = dive_config::parse(&cfg_text).into_config() else {
        skip("configuration does not parse");
        return Ok(());
    };
    if cfg.init.is_none() && cfg.specification.is_none() {
        skip("no behavior to check");
        return Ok(());
    }
    if !cfg.properties.is_empty() {
        skip("temporal properties");
        return Ok(());
    }
    let base = run(&tla, &tla.with_extension("cfg"))?;
    if stopped(&base) {
        skip("stopped");
        return Ok(());
    }
    if base["elapsed_ms"]
        .as_u64()
        .is_none_or(|ms| ms > MAX_BASE_MS)
    {
        skip("slower than a second");
        return Ok(());
    }
    if !["success", "invariant-violation"].contains(&verdict(&base)) {
        skip("neither success nor an invariant violation");
        return Ok(());
    }
    let seed = Seed {
        dir: dir.to_path_buf(),
        root: root.to_owned(),
        cfg_text,
        cfg,
    };
    let identity = seed.wrapped("", &seed.cfg_text)?;
    if observed(&identity) != observed(&base) {
        skip("the unchanged wrapper reports differently");
        tally.wrapper_differs.push(json!({
            "seed": rel, "base": observed(&base), "wrapper": observed(&identity),
        }));
        return Ok(());
    }
    tally.seeds += 1;
    tally.record(rel, "depth-bound", depth_bound(&seed, &base)?);
    if verdict(&base) == "success" {
        tally.record(rel, "depth-invariant", depth_invariant(&seed, &base)?);
    }
    if seed.cfg.invariants.len() >= 2 {
        for (_, outcome) in drop_invariant(&seed, &base)? {
            tally.record(rel, "drop-invariant", outcome);
        }
    }
    Ok(())
}

fn check_seeds(seeds: &[String]) -> Result<Tally> {
    let workspace = workspace_root();
    let mut tally = Tally::default();
    for seed in seeds {
        check_seed(&workspace, seed, &mut tally)?;
    }
    Ok(tally)
}

fn summary(tally: &Tally) -> String {
    let mut out = format!("directional: {} seeds checked\n", tally.seeds);
    for (why, n) in &tally.skipped {
        let _ = writeln!(out, "  skipped ({n}): {why}");
    }
    for (relation, n) in &tally.holds {
        let _ = writeln!(out, "  holds ({n}): {relation}");
    }
    for (why, n) in &tally.inconclusive {
        let _ = writeln!(out, "  inconclusive ({n}): {why}");
    }
    for f in &tally.failures {
        let _ = writeln!(
            out,
            "  FAILS {} {}: {}",
            f["seed"], f["relation"], f["detail"]
        );
    }
    out
}

#[test]
fn fixed_seeds_satisfy_directional_relations() -> Result<()> {
    let seeds: Vec<String> = FAST_SEEDS.iter().map(|s| (*s).to_owned()).collect();
    let tally = check_seeds(&seeds)?;
    assert!(tally.failures.is_empty(), "{}", summary(&tally));
    assert_eq!(tally.seeds, seeds.len(), "{}", summary(&tally));
    Ok(())
}

#[test]
fn every_fixture_satisfies_directional_relations_when_requested() -> Result<()> {
    if std::env::var("DIVE_DIRECTIONAL").as_deref() != Ok("all") {
        return Ok(());
    }
    let seeds = all_seeds(&workspace_root());
    let tally = check_seeds(&seeds)?;
    eprintln!("{}", summary(&tally));
    if let Ok(path) = std::env::var("DIVE_DIRECTIONAL_REPORT") {
        let report = json!({
            "seeds": seeds.len(),
            "checked": tally.seeds,
            "skipped": tally.skipped,
            "holds": tally.holds,
            "inconclusive": tally.inconclusive,
            "wrapper_differs": tally.wrapper_differs,
            "failures": tally.failures,
        });
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    assert!(tally.failures.is_empty(), "{}", summary(&tally));
    Ok(())
}
