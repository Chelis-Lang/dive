//! Fault injection (charter §Engineering practice): a fault must end in the
//! correct answer or an explicit safe stop, never a different verdict or
//! counts presented as complete.
//!
//! Each seed is checked once unfaulted (the reference), then under faults:
//!
//! - *transparent* faults must reproduce the reference's observables
//!   (verdict, property, TLC code and exit status, statistics, counterexample
//!   and output): chaos scheduling at 1, 4 and 8 workers, spilling the
//!   frontier, predecessor log and seen set to disk with zero memory budgets,
//!   and pausing at a completed level then recovering from the checkpoint;
//! - *disruptive* faults may also stop safely, as `stopped` or
//!   `resource-limit` without a counterexample and with statistics no larger
//!   than the reference's, or refuse with an error: state, depth and time
//!   budgets at several points, spill and checkpoint directories that do
//!   not exist or are read-only, and checkpoint archives that are
//!   truncated, emptied, extended or have one byte flipped before recovery.
//!
//! An `internal-error` verdict always fails. A run dive refuses as
//! `unsupported` (a mode it does not combine) is inconclusive.
//!
//! Runs use the default profile, seed 0, fp 0 and the exact store; seeds
//! whose reference takes over a second are skipped, since the memory budget
//! measures the whole test process. The fast test checks a fixed seed list.
//! `DIVE_FAULTS=all` checks every fixture, and `DIVE_FAULTS_REPORT=<path>`
//! writes a JSON summary of that sweep.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest, SeenOptions, Store, checkpoint};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Seeds for the fast test: safety and liveness, passing and violated.
const FAST_SEEDS: &[&str] =
    &["crates/dive-conformance/tests/fixtures/specs/InvariantViolation/InvariantViolation.tla"];

/// States per run; larger references are skipped.
const MAX_STATES: u64 = 20_000;
/// Wall time per run.
const TIME_LIMIT: Duration = Duration::from_secs(10);
/// References slower than this are skipped.
const MAX_BASE_MS: u64 = 1_000;

// ---------------------------------------------------------------------------
// Scratch directories and a deterministic generator

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = make_writable(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "dive-faults-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}

fn make_writable(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "restoring a test directory"
    )]
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions)
}

fn make_read_only(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions)
}

struct Rng(u64);
impl Rng {
    fn from(text: &str) -> Self {
        Self(
            text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            }) | 1,
        )
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

// ---------------------------------------------------------------------------
// Runs and observables

fn request(spec: &Path) -> CheckRequest {
    CheckRequest {
        file: spec.to_path_buf(),
        config: Some(spec.with_extension("cfg")),
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        store: Store::Exact,
        max_states: Some(MAX_STATES),
        time_limit: BudgetSetting::Explicit(TIME_LIMIT),
        ..CheckRequest::default()
    }
}

/// What a faulted run must reproduce.
fn observed(doc: &Value) -> Value {
    let cx = &doc["counterexample"];
    json!({
        "verdict": doc["verdict"],
        "property": doc["property"],
        "tlc": [doc["tlc"]["code"], doc["tlc"]["exit_status"]],
        "statistics": [
            doc["statistics"]["generated"], doc["statistics"]["distinct"],
            doc["statistics"]["depth"], doc["statistics"]["left_on_queue"],
        ],
        "counterexample": if cx.is_null() { Value::Null } else { json!({
            "length": cx["length"], "back_to": cx["back_to"],
            "states": cx["states"].as_array().map(|s| s.iter().map(|s| s["values"].clone()).collect::<Vec<_>>()),
        })},
        "output": doc["output"],
    })
}

fn verdict(doc: &Value) -> &str {
    doc["verdict"].as_str().unwrap_or_default()
}
fn stat(doc: &Value, name: &str) -> Option<u64> {
    doc["statistics"][name].as_u64()
}
fn safe_stop(doc: &Value) -> bool {
    verdict(doc) == "stopped" || verdict(doc) == "resource-limit"
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Transparent,
    Disruptive,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Same,
    /// The same deciding result, with statistics bounded by the budget: a
    /// discovery limit also bounds diagnostic error replay (docs/user/checking.md).
    SameResultBoundedCounts,
    Stopped,
    Refused,
    Inconclusive(String),
    Fails(String),
}

/// Judges one faulted run against the reference.
fn judge(kind: Kind, reference: &Value, faulted: &std::result::Result<Value, String>) -> Outcome {
    let doc = match faulted {
        Ok(doc) => doc,
        Err(e) if kind == Kind::Disruptive => {
            return if e.is_empty() {
                Outcome::Fails("an empty error".into())
            } else {
                Outcome::Refused
            };
        }
        Err(e) => return Outcome::Fails(format!("refused a transparent fault: {e}")),
    };
    if verdict(doc) == "internal-error" {
        return Outcome::Fails(format!("internal error: {}", doc["message"]));
    }
    if verdict(doc) == "unsupported" && verdict(reference) != "unsupported" {
        return Outcome::Inconclusive(format!(
            "unsupported: {}",
            reason(doc["message"].as_str().unwrap_or_default())
        ));
    }
    if observed(doc) == observed(reference) {
        return Outcome::Same;
    }
    let (a, b) = (observed(doc), observed(reference));
    let same_result = ["verdict", "property", "tlc", "counterexample", "output"]
        .iter()
        .all(|k| a[k] == b[k]);
    let no_larger = ["distinct", "generated", "depth"].iter().all(|n| {
        stat(doc, n)
            .zip(stat(reference, n))
            .is_none_or(|(x, y)| x <= y)
    });
    if kind == Kind::Disruptive && same_result && no_larger && verdict(doc) != "success" {
        return Outcome::SameResultBoundedCounts;
    }
    if kind == Kind::Disruptive && safe_stop(doc) && !safe_stop(reference) {
        let larger = ["distinct", "generated"].iter().any(|n| {
            stat(doc, n)
                .zip(stat(reference, n))
                .is_some_and(|(a, b)| a > b)
        });
        return if !doc["counterexample"].is_null() {
            Outcome::Fails("a stopped run carries a counterexample".into())
        } else if larger {
            Outcome::Fails(format!(
                "a stopped run reports more than the complete run: {} vs {}",
                observed(doc)["statistics"],
                observed(reference)["statistics"]
            ))
        } else {
            Outcome::Stopped
        };
    }
    Outcome::Fails(format!(
        "reference {}, faulted {}",
        observed(reference),
        observed(doc)
    ))
}

fn checked(req: &CheckRequest) -> std::result::Result<Value, String> {
    dive_api::check_file_engine(req, dive_api::Engine::Core)
        .map(|r| r.to_json())
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Faults

struct Fault {
    name: String,
    kind: Kind,
    result: std::result::Result<Value, String>,
}

fn budget_faults(spec: &Path, reference: &Value, rng: &mut Rng) -> Vec<Fault> {
    let distinct = stat(reference, "distinct").unwrap_or(1);
    let depth = stat(reference, "depth").unwrap_or(1);
    let mut faults = Vec::new();
    let mut states: Vec<u64> = vec![
        1,
        distinct / 2,
        distinct.saturating_sub(1),
        distinct,
        distinct + 1,
    ];
    states.push(1 + rng.below(distinct.max(1)));
    states.sort_unstable();
    states.dedup();
    for n in states.into_iter().filter(|n| *n > 0) {
        let mut req = request(spec);
        req.max_states = Some(n);
        faults.push(Fault {
            name: format!("max-states {n}"),
            kind: Kind::Disruptive,
            result: checked(&req),
        });
    }
    let mut depths: Vec<u64> = vec![1, depth / 2, depth.saturating_sub(1), depth];
    depths.sort_unstable();
    depths.dedup();
    for d in depths.into_iter().filter(|d| *d > 0) {
        let mut req = request(spec);
        req.max_depth = Some(d);
        faults.push(Fault {
            name: format!("max-depth {d}"),
            kind: Kind::Disruptive,
            result: checked(&req),
        });
    }
    let mut req = request(spec);
    req.time_limit = BudgetSetting::Explicit(Duration::from_millis(1));
    faults.push(Fault {
        name: "time-limit 1ms".into(),
        kind: Kind::Disruptive,
        result: checked(&req),
    });
    faults
}

fn scheduling_faults(spec: &Path, rng: &mut Rng) -> Vec<Fault> {
    [1, 4, 8]
        .into_iter()
        .map(|workers| {
            let chaos = rng.next();
            let mut req = request(spec);
            req.workers = Some(workers);
            req.chaos = Some(chaos);
            Fault {
                name: format!("chaos {chaos} at {workers} workers"),
                kind: Kind::Transparent,
                result: checked(&req),
            }
        })
        .collect()
}

fn spill_faults(spec: &Path) -> Result<Vec<Fault>> {
    let mut faults = Vec::new();
    // Zero memory budgets: everything spills to private directories.
    for role in ["frontier", "predecessor", "seen", "all"] {
        let dir = scratch()?;
        let mut req = request(spec);
        spill(&mut req, role, &dir.0);
        faults.push(Fault {
            name: format!("spill {role}"),
            kind: Kind::Transparent,
            result: checked(&req),
        });
    }
    // Directories that cannot hold the spill.
    for role in ["frontier", "predecessor", "seen"] {
        let dir = scratch()?;
        let mut req = request(spec);
        spill(&mut req, role, &dir.0.join("missing"));
        faults.push(Fault {
            name: format!("spill {role} to a missing directory"),
            kind: Kind::Disruptive,
            result: checked(&req),
        });
        let mut req = request(spec);
        spill(&mut req, role, &dir.0);
        make_read_only(&dir.0)?;
        faults.push(Fault {
            name: format!("spill {role} to a read-only directory"),
            kind: Kind::Disruptive,
            result: checked(&req),
        });
        make_writable(&dir.0)?;
    }
    Ok(faults)
}

fn spill(req: &mut CheckRequest, role: &str, dir: &Path) {
    if role == "frontier" || role == "all" {
        req.frontier_memory_bytes = Some(0);
        req.frontier_directory = Some(dir.to_path_buf());
    }
    if role == "predecessor" || role == "all" {
        req.predecessor_memory_bytes = Some(0);
        req.predecessor_directory = Some(dir.to_path_buf());
    }
    if role == "seen" || role == "all" {
        req.seen_disk = Some(SeenOptions {
            directory: Some(dir.to_path_buf()),
            ..SeenOptions::default()
        });
    }
}

fn pause(req: &CheckRequest, path: &Path, levels: u64) -> std::result::Result<Value, String> {
    dive_api::check_file_checkpoint_engine(
        req,
        &checkpoint::Request {
            path: path.to_path_buf(),
            action: checkpoint::Action::PauseAfterLevels(levels),
            counters: checkpoint::Counters::Retain,
            config_source_sha256: None,
        },
        dive_api::Engine::Core,
    )
    .map(|r| r.to_json())
    .map_err(|e| e.to_string())
}

fn recover(req: &CheckRequest, path: &Path) -> std::result::Result<Value, String> {
    dive_api::check_file_checkpoint_engine(
        req,
        &checkpoint::Request {
            path: path.to_path_buf(),
            action: checkpoint::Action::Recover,
            counters: checkpoint::Counters::Retain,
            config_source_sha256: None,
        },
        dive_api::Engine::Core,
    )
    .map(|r| r.to_json())
    .map_err(|e| e.to_string())
}

/// Ways to damage an archive: (name, damage).
type Damage = (String, Box<dyn Fn(&mut Vec<u8>)>);

fn damages(len: usize, rng: &mut Rng) -> Vec<Damage> {
    let mut out: Vec<Damage> = vec![
        ("emptied".into(), Box::new(Vec::clear)),
        (
            "truncated to half".into(),
            Box::new(|b: &mut Vec<u8>| b.truncate(b.len() / 2)),
        ),
        (
            "last byte removed".into(),
            Box::new(|b: &mut Vec<u8>| {
                b.pop();
            }),
        ),
        (
            "extended".into(),
            Box::new(|b: &mut Vec<u8>| b.extend_from_slice(b"garbage")),
        ),
    ];
    let len = u64::try_from(len).unwrap_or(0);
    let mut offsets = vec![0, 9, 47.min(len.saturating_sub(1)), len.saturating_sub(1)];
    offsets.extend((0..4).map(|_| rng.below(len)));
    offsets.sort_unstable();
    offsets.dedup();
    for offset in offsets.into_iter().filter(|o| *o < len) {
        let bit = 1u8 << rng.below(8);
        out.push((
            format!("bit {bit:#04x} flipped at byte {offset}"),
            Box::new(move |b: &mut Vec<u8>| {
                if let Some(byte) = usize::try_from(offset).ok().and_then(|o| b.get_mut(o)) {
                    *byte ^= bit;
                }
            }),
        ));
    }
    out
}

fn checkpoint_faults(spec: &Path, reference: &Value, rng: &mut Rng) -> Result<Vec<Fault>> {
    let mut faults = Vec::new();
    let depth = stat(reference, "depth").unwrap_or(1);
    let mut levels = vec![0, 1, depth / 2];
    levels.sort_unstable();
    levels.dedup();
    for level in levels {
        let dir = scratch()?;
        let archive = dir.0.join("run.cp");
        let req = request(spec);
        let paused = match pause(&req, &archive, level) {
            Ok(doc) if doc["checkpoint"]["status"] == "paused-incomplete" => doc,
            // The model was decided, or refused checkpointing, before the
            // pause: the recovery fault does not apply.
            Ok(doc) => {
                faults.push(Fault {
                    name: format!("pause after {level} levels"),
                    kind: Kind::Disruptive,
                    result: Ok(doc),
                });
                continue;
            }
            Err(e) => {
                faults.push(Fault {
                    name: format!("pause after {level} levels"),
                    kind: Kind::Disruptive,
                    result: Err(e),
                });
                continue;
            }
        };
        faults.push(Fault {
            name: format!("pause after {level} levels"),
            kind: Kind::Disruptive,
            result: Ok(paused),
        });
        let bytes = std::fs::read(&archive)?;
        faults.push(Fault {
            name: format!("recover after {level} levels"),
            kind: Kind::Transparent,
            result: recover(&req, &archive),
        });
        for (name, damage) in damages(bytes.len(), rng) {
            let damaged = dir.0.join(format!("damaged-{}.cp", faults.len()));
            let mut copy = bytes.clone();
            damage(&mut copy);
            std::fs::write(&damaged, &copy)?;
            faults.push(Fault {
                name: format!("recover after {level} levels, archive {name}"),
                kind: Kind::Disruptive,
                result: recover(&req, &damaged),
            });
        }
        let missing = dir.0.join("missing").join("run.cp");
        faults.push(Fault {
            name: format!("pause after {level} levels into a missing directory"),
            kind: Kind::Disruptive,
            result: pause(&req, &missing, level),
        });
    }
    Ok(faults)
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
    outcomes: BTreeMap<String, usize>,
    inconclusive: BTreeMap<String, usize>,
    failures: Vec<Value>,
}

/// The fault family, for tallies: its name with numbers left out.
fn family(name: &str) -> String {
    name.split_whitespace()
        .map(|w| {
            if w.chars().any(|c| c.is_ascii_digit()) {
                if w.ends_with(',') { "N," } else { "N" }
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A refusal's reason without its source locations, property names or
/// generated numbers.
fn reason(message: &str) -> String {
    let text = message
        .find("does not yet support")
        .map_or(message, |at| &message[at..]);
    family(text).chars().take(100).collect()
}

fn check_seed(workspace: &Path, rel: &str, tally: &mut Tally) -> Result<()> {
    let spec = workspace.join(rel);
    let Ok(reference) = checked(&request(&spec)) else {
        *tally
            .skipped
            .entry("the reference is refused".into())
            .or_default() += 1;
        return Ok(());
    };
    let skip = if safe_stop(&reference) {
        Some("the reference stops")
    } else if reference["elapsed_ms"]
        .as_u64()
        .is_none_or(|ms| ms > MAX_BASE_MS)
    {
        Some("the reference is slower than a second")
    } else if ["unsupported", "rejected", "internal-error"].contains(&verdict(&reference)) {
        Some("the reference has no checking result")
    } else {
        None
    };
    if let Some(why) = skip {
        *tally.skipped.entry(why.into()).or_default() += 1;
        return Ok(());
    }
    tally.seeds += 1;
    let mut rng = Rng::from(rel);
    let mut faults = budget_faults(&spec, &reference, &mut rng);
    faults.extend(scheduling_faults(&spec, &mut rng));
    faults.extend(spill_faults(&spec)?);
    faults.extend(checkpoint_faults(&spec, &reference, &mut rng)?);
    for fault in faults {
        let outcome = judge(fault.kind, &reference, &fault.result);
        let key = |what: &str| format!("{}: {what}", family(&fault.name));
        match outcome {
            Outcome::Same => *tally.outcomes.entry(key("same")).or_default() += 1,
            Outcome::SameResultBoundedCounts => {
                *tally
                    .outcomes
                    .entry(key("same result, bounded counts"))
                    .or_default() += 1;
            }
            Outcome::Stopped => *tally.outcomes.entry(key("stopped")).or_default() += 1,
            Outcome::Refused => *tally.outcomes.entry(key("refused")).or_default() += 1,
            Outcome::Inconclusive(why) => *tally.inconclusive.entry(key(&why)).or_default() += 1,
            Outcome::Fails(detail) => tally
                .failures
                .push(json!({"seed": rel, "fault": fault.name, "detail": detail})),
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
    let mut out = format!("faults: {} seeds checked\n", tally.seeds);
    for (why, n) in &tally.skipped {
        let _ = writeln!(out, "  skipped ({n}): {why}");
    }
    for (what, n) in &tally.outcomes {
        let _ = writeln!(out, "  {what} ({n})");
    }
    for (why, n) in &tally.inconclusive {
        let _ = writeln!(out, "  inconclusive ({n}): {why}");
    }
    for f in &tally.failures {
        let _ = writeln!(
            out,
            "  FAILS {} [{}]: {}",
            f["seed"], f["fault"], f["detail"]
        );
    }
    out
}

#[test]
fn fixed_seeds_survive_faults() -> Result<()> {
    let seeds: Vec<String> = FAST_SEEDS.iter().map(|s| (*s).to_owned()).collect();
    let tally = check_seeds(&seeds)?;
    assert!(tally.failures.is_empty(), "{}", summary(&tally));
    assert_eq!(tally.seeds, seeds.len(), "{}", summary(&tally));
    Ok(())
}

#[test]
fn incomplete_diagnostic_replay_refuses_an_uncertified_error_class() -> Result<()> {
    use dive_api::ExitConvention;
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};

    let spec = workspace_root().join(
        "crates/dive-api/tests/fixtures/checker/errors/specs/ErInvReplayError/ErInvReplayError.tla",
    );
    for compat_tlc in [false, true] {
        let mut req = request(&spec);
        req.compat_tlc = compat_tlc;
        let reference_report = dive_api::check_file_engine(&req, dive_api::Engine::Core)?;
        let reference = reference_report.to_json();
        assert_eq!(reference_report.exit_code_for(ExitConvention::Native), 13);
        assert_eq!(reference_report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(
            observed(&reference),
            json!({
                "verdict": "evaluation-error",
                "property": null,
                "tlc": [2103, 75],
                "statistics": [4, 5, 3, 1],
                "counterexample": {
                    "length": 3, "back_to": null,
                    "states": [{"x":0,"y":0}, {"x":1,"y":0}, {"x":2,"y":0}],
                },
                "output": [],
            })
        );
        assert_eq!(reference["tlc"]["reported"], 2111);
        assert_eq!(reference["tlc"]["verified"], true);
        let wire = &reference["counterexample"]["trace"];
        assert_eq!(wire["version"], 2);
        assert_eq!(
            wire["ending"],
            json!({
                "kind": "invariant-error", "name": "InvA", "ec": "TLC_MODULE_DIVISION_BY_ZERO",
            })
        );
        let model = SpecModel::load(
            &spec,
            &spec.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?;
        assert_eq!(
            model.try_validate(&TraceDoc::from_json(wire)?)?.validation,
            Validation::Valid
        );

        // The retired reference engine refused this capped run: it could not
        // certify its diagnostic replay under the cap (#791). Core finds the
        // error within the cap and reports exactly the uncapped result, with
        // a word that validates independently.
        req.max_states = Some(4);
        let faulted_report = dive_api::check_file_engine(&req, dive_api::Engine::Core)?;
        let faulted = faulted_report.to_json();
        assert_eq!(faulted_report.exit_code_for(ExitConvention::Native), 13);
        assert_eq!(faulted_report.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(observed(&faulted), observed(&reference));
        assert_eq!(faulted["tlc"], reference["tlc"]);
        assert_eq!(
            model
                .try_validate(&TraceDoc::from_json(&faulted["counterexample"]["trace"])?)?
                .validation,
            Validation::Valid
        );
        assert_eq!(
            judge(Kind::Disruptive, &reference, &Ok(faulted)),
            Outcome::Same
        );
    }
    Ok(())
}

#[test]
fn every_fixture_survives_faults_when_requested() -> Result<()> {
    if std::env::var("DIVE_FAULTS").as_deref() != Ok("all") {
        return Ok(());
    }
    let seeds = all_seeds(&workspace_root());
    let tally = check_seeds(&seeds)?;
    eprintln!("{}", summary(&tally));
    if let Ok(path) = std::env::var("DIVE_FAULTS_REPORT") {
        let report = json!({
            "seeds": seeds.len(),
            "checked": tally.seeds,
            "skipped": tally.skipped,
            "outcomes": tally.outcomes,
            "inconclusive": tally.inconclusive,
            "failures": tally.failures,
        });
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    assert!(tally.failures.is_empty(), "{}", summary(&tally));
    Ok(())
}
