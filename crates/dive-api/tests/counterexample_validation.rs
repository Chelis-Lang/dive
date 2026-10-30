//! Every counterexample `dive check` reports must validate independently
//! (ADR 0016, ADR 0086): the document in `counterexample.trace` is read back
//! by `dive-trace`, bound to the same specification and configuration, and
//! must be a behavior of the specification that ends as the report claims.
//!
//! The conformance harness does this for corpus models with TLC goldens.
//! This test does it for every fixture spec with a same-named configuration,
//! in both profiles, so counterexamples from fixtures without goldens (error
//! endings, liveness, start-up reads) are validated too.
//!
//! Outcomes per run:
//!
//! - *valid*: the validator accepts the trace;
//! - *refuted*: the validator rejects it, or its ending disagrees with the
//!   report's verdict. These fail the test unless [`known_refutation`]
//!   names an open issue;
//! - *unchecked*: the run has a counterexample but no trace document, the
//!   validator cannot bind the model, or it reports part of the claim
//!   unsupported (for example a deadlock, which it cannot confirm alone).
//!   These are counted, not failures: the validator never claims more than
//!   it checked.
//!
//! The fast test checks a fixed seed list. `DIVE_CX_VALIDATION=all` checks
//! every fixture, and `DIVE_CX_VALIDATION_REPORT=<path>` writes a JSON
//! summary of that sweep.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::trace::{Ending, Failure, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{BudgetSetting, CheckRequest, check_file};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Seeds for the fast test, valid in both profiles: one or more per ending
/// the validator reads.
const FAST_SEEDS: &[&str] = &[
    // deadlock
    "crates/dive-check/tests/fixtures/tlc-probes/specs/DeadlockMidLevel/DeadlockMidLevel.tla",
    "crates/dive-conformance/tests/fixtures/specs/Deadlock/Deadlock.tla",
    // temporal state error
    "conformance/corpus/liveness-atom-error-timing/specifications/DelayedSiblingStateError/DelayedSiblingStateError.tla",
    "crates/dive-api/tests/fixtures/native-sequence-operators/models/SequenceAppendTypeError/SequenceAppendTypeError.tla",
    // invariant
    "conformance/corpus/empty-subset-base-primary/specifications/DirectEmptyBase/DirectEmptyBase.tla",
    "conformance/corpus/compound-domain-streaming/specifications/WideProductInit/WideProductInit.tla",
    // lasso
    "conformance/corpus/fairness-replay/specifications/Fair/Fair.tla",
    "conformance/corpus/fairness-replay/specifications/FairStrong/FairStrong.tla",
    "conformance/corpus/configured-temporal-operators/specifications/ConfiguredReplyFails/ConfiguredReplyFails.tla",
    // action and initial properties
    "crates/dive-check/tests/fixtures/tlc-probes/specs/ImpliedActionSeen/ImpliedActionSeen.tla",
    "crates/dive-conformance/tests/fixtures/specs/ActionProperty/ActionProperty.tla",
    "crates/dive-conformance/tests/fixtures/specs/InitProperty/InitProperty.tla",
    // Terminal invariant error and action property outside constraints (#499).
    "crates/dive-check/tests/fixtures/errors/specs/ErInvConstrained/ErInvConstrained.tla",
    "crates/dive-check/tests/fixtures/dfid/DfidActionConstrained.tla",
    // Named invariant evaluation error (#490).
    "crates/dive-check/tests/fixtures/errors/specs/ErInvDivZero/ErInvDivZero.tla",
];

/// One seed per known refutation, with the refutation it shows.
const KNOWN_SEEDS: &[(&str, &str)] = &[
    (
        "crates/dive-check/tests/fixtures/errors/specs/ErConstrDivZero/ErConstrDivZero.tla",
        KNOWN_ERROR_RERAISED,
    ),
    (
        "crates/dive-check/tests/fixtures/errors/specs/ErActConstrDivZero/ErActConstrDivZero.tla",
        KNOWN_ERROR_RERAISED,
    ),
    (
        "crates/dive-check/tests/fixtures/errors/specs/ErPropDivZero/ErPropDivZero.tla",
        KNOWN_ERROR_RERAISED,
    ),
    (
        "conformance/corpus/compound-domain-emptiness/specifications/EmptyProductSkipsMixedInit/EmptyProductSkipsMixedInit.tla",
        KNOWN_SKIPPED_INIT_DISJUNCT,
    ),
];

/// States explored per run; a stopped run has no counterexample to check.
const MAX_STATES: u64 = 20_000;
/// Wall time per run.
const TIME_LIMIT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Profile {
    Dive,
    Tlc,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Self::Dive => "dive",
            Self::Tlc => "tlc",
        }
    }
}

fn run(spec: &Path, profile: Profile) -> Result<Value> {
    let report = check_file(&CheckRequest {
        file: spec.to_path_buf(),
        config: Some(spec.with_extension("cfg")),
        compat_tlc: profile == Profile::Tlc,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        max_states: Some(MAX_STATES),
        time_limit: BudgetSetting::Explicit(TIME_LIMIT),
        ..CheckRequest::default()
    })?;
    Ok(report.to_json())
}

/// Whether the trace's ending is the one the report's verdict and property
/// name.
fn ending_matches(verdict: &str, property: Option<&str>, ending: &Ending) -> bool {
    match (verdict, ending) {
        ("invariant-violation", Ending::Invariant(name))
        | ("action-property-violation" | "property-violation", Ending::Property(name)) => {
            property == Some(name.as_str())
        }
        ("deadlock", Ending::Deadlock)
        // A failed `Assert` while checking a property is an evaluation error
        // to TLC, but the trace names the assertion itself.
        | ("assertion-failure" | "evaluation-error", Ending::Assertion)
        | (
            "property-violation" | "liveness-violation",
            Ending::Lasso { .. } | Ending::Stuttering | Ending::TemporalStateError { .. },
        )
        | (
            "evaluation-error" | "overflow",
            Ending::Error { .. } | Ending::InvariantError { .. } | Ending::TemporalStateError { .. },
        ) => true,
        _ => false,
    }
}

/// The validator's refutations of correct counterexamples found by the
/// sweep. Each matches TLC's own counterexample for the same model.
const KNOWN_ERROR_RERAISED: &str =
    "#490: a claimed evaluation error is re-raised by a last-state check and refuted";
const KNOWN_CONSTRAINED_LAST_STATE: &str =
    "#499: anonymous error endings cannot yet prove terminal checks outside constraints";
const KNOWN_SKIPPED_INIT_DISJUNCT: &str = "#443: Init membership raises on a disjunct TLC's generator skips (empty product or record set)";

/// A refutation the sweep already found, so it does not fail the test.
/// [`known_refutations_still_reproduce`] fails once one is fixed.
fn known_refutation(ending: &Ending, failure: &Failure, last: usize) -> Option<&'static str> {
    let check = failure.check.name();
    let raises = matches!(ending, Ending::Error { .. } | Ending::Assertion);
    if raises
        && failure.state == last
        && [
            "invariant",
            "constraint",
            "action-constraint",
            "implied-action",
        ]
        .contains(&check)
        && !failure.detail.contains("may lie outside the model")
    {
        return Some(KNOWN_ERROR_RERAISED);
    }
    if raises
        && matches!(check, "constraint" | "action-constraint")
        && failure.state == last
        && failure
            .detail
            .contains("only a checked terminal invariant or property may lie outside the model")
    {
        return Some(KNOWN_CONSTRAINED_LAST_STATE);
    }
    (check == "init" && failure.detail.contains("Attempted to check if non-"))
        .then_some(KNOWN_SKIPPED_INIT_DISJUNCT)
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// No counterexample (or a stopped run).
    NoCounterexample,
    /// Valid, with the ending's kind.
    Valid(String),
    Unchecked(String),
    Known(&'static str),
    Refuted(String),
}

fn check_run(spec: &Path, profile: Profile) -> Result<Outcome> {
    let report = run(spec, profile)?;
    let cx = &report["counterexample"];
    if cx.is_null() {
        return Ok(Outcome::NoCounterexample);
    }
    let verdict = report["verdict"].as_str().unwrap_or_default();
    if cx["trace"].is_null() {
        return Ok(Outcome::Unchecked(format!("no trace document ({verdict})")));
    }
    let doc = match TraceDoc::from_json(&cx["trace"]) {
        Ok(doc) => doc,
        Err(e) => return Ok(Outcome::Refuted(format!("unreadable trace document: {e}"))),
    };
    let Some(ending) = doc.ending.clone() else {
        return Ok(Outcome::Refuted("the trace document has no ending".into()));
    };
    if !ending_matches(verdict, report["property"].as_str(), &ending) {
        return Ok(Outcome::Refuted(format!(
            "the trace ends {:?}, but the report says {verdict} {}",
            ending, report["property"]
        )));
    }
    if cx["length"].as_u64() != u64::try_from(doc.logical_length()).ok() {
        return Ok(Outcome::Refuted(format!(
            "the report counts {} states, the trace document {}",
            cx["length"],
            doc.logical_length()
        )));
    }
    let options = TraceOptions {
        compat_tlc: profile == Profile::Tlc,
        run: Some((0, 0)),
        ..TraceOptions::default()
    };
    let model = match SpecModel::load(spec, &spec.with_extension("cfg"), &options) {
        Ok(model) => model,
        Err(e) => {
            return Ok(Outcome::Unchecked(format!(
                "cannot bind: {}",
                first_line(&e.to_string())
            )));
        }
    };
    let checked = match model.try_read(&doc) {
        Ok((checked, _)) => checked,
        Err(e) => {
            return Ok(Outcome::Unchecked(format!(
                "validation did not complete: {e}"
            )));
        }
    };
    Ok(match checked.validation {
        Validation::Valid => Outcome::Valid(
            ending.to_json()["kind"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ),
        Validation::Unsupported(why) => {
            Outcome::Unchecked(format!("unsupported: {}", first_line(&why)))
        }
        Validation::Invalid(failure) => {
            let detail = format!(
                "state {}, check `{}`: {}",
                failure.state + 1,
                failure.check.name(),
                failure.detail
            );
            match known_refutation(&ending, &failure, doc.states.len().saturating_sub(1)) {
                Some(issue) => Outcome::Known(issue),
                None => Outcome::Refuted(detail),
            }
        }
    })
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    // Keep reasons groupable: drop source locations and quoted values.
    line.split(" of module ")
        .next()
        .unwrap_or(line)
        .chars()
        .take(120)
        .collect()
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
    runs: usize,
    no_counterexample: usize,
    valid: BTreeMap<String, Vec<String>>,
    unchecked: BTreeMap<String, Vec<String>>,
    known: BTreeMap<&'static str, Vec<String>>,
    refuted: Vec<Value>,
}

fn check_seeds(seeds: &[String]) -> Result<Tally> {
    let workspace = workspace_root();
    let mut tally = Tally::default();
    for seed in seeds {
        let spec = workspace.join(seed);
        for profile in [Profile::Dive, Profile::Tlc] {
            tally.runs += 1;
            let at = format!("{seed} [{}]", profile.name());
            match check_run(&spec, profile)? {
                Outcome::NoCounterexample => tally.no_counterexample += 1,
                Outcome::Valid(kind) => tally.valid.entry(kind).or_default().push(at),
                Outcome::Unchecked(why) => tally.unchecked.entry(why).or_default().push(at),
                Outcome::Known(issue) => tally.known.entry(issue).or_default().push(at),
                Outcome::Refuted(detail) => {
                    tally
                        .refuted
                        .push(json!({"seed": seed, "profile": profile.name(), "detail": detail}));
                }
            }
        }
    }
    Ok(tally)
}

fn summary(tally: &Tally) -> String {
    let unchecked: usize = tally.unchecked.values().map(Vec::len).sum();
    let known: usize = tally.known.values().map(Vec::len).sum();
    let valid: usize = tally.valid.values().map(Vec::len).sum();
    let by_ending: BTreeMap<&String, usize> = tally
        .valid
        .iter()
        .map(|(kind, at)| (kind, at.len()))
        .collect();
    let mut out = format!(
        "{} runs: {} without a counterexample, {valid} valid {by_ending:?}, {unchecked} unchecked, {known} known, {} refuted\n",
        tally.runs,
        tally.no_counterexample,
        tally.refuted.len()
    );
    for (why, at) in &tally.unchecked {
        let _ = writeln!(out, "  unchecked ({}): {why}", at.len());
    }
    for (issue, at) in &tally.known {
        let _ = writeln!(out, "  known ({}): {issue}", at.len());
    }
    for r in &tally.refuted {
        let _ = writeln!(
            out,
            "  REFUTED {} [{}]: {}",
            r["seed"], r["profile"], r["detail"]
        );
    }
    out
}

#[test]
fn fixed_seeds_validate() -> Result<()> {
    let seeds: Vec<String> = FAST_SEEDS.iter().map(|s| (*s).to_owned()).collect();
    let tally = check_seeds(&seeds)?;
    let valid: usize = tally.valid.values().map(Vec::len).sum();
    assert_eq!(valid, tally.runs, "{}", summary(&tally));
    Ok(())
}

/// Fails once the validator accepts a known refutation's counterexample:
/// then remove its entry from [`known_refutation`].
#[test]
fn known_refutations_still_reproduce() -> Result<()> {
    let workspace = workspace_root();
    for (seed, known) in KNOWN_SEEDS {
        for profile in [Profile::Dive, Profile::Tlc] {
            let outcome = check_run(&workspace.join(seed), profile)?;
            assert_eq!(
                outcome,
                Outcome::Known(known),
                "{seed} [{}]",
                profile.name()
            );
        }
    }
    Ok(())
}

#[test]
fn every_fixture_counterexample_validates_when_requested() -> Result<()> {
    if std::env::var("DIVE_CX_VALIDATION").as_deref() != Ok("all") {
        return Ok(());
    }
    let seeds = all_seeds(&workspace_root());
    let tally = check_seeds(&seeds)?;
    eprintln!("{}", summary(&tally));
    if let Ok(path) = std::env::var("DIVE_CX_VALIDATION_REPORT") {
        let report = json!({
            "seeds": seeds.len(),
            "runs": tally.runs,
            "no_counterexample": tally.no_counterexample,
            "valid": tally.valid,
            "unchecked": tally.unchecked,
            "known": tally.known,
            "refuted": tally.refuted,
        });
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    assert!(tally.refuted.is_empty(), "{}", summary(&tally));
    Ok(())
}
