//! The human report and the JSON report of one `dive check` run must say the
//! same thing: the run's options, the verdict and violated properties, the
//! statistics, the counterexample, the printed output and the warnings.
//!
//! Each fixture spec with a same-named configuration is checked once per
//! profile; both renderings come from that one report, so any difference is
//! a rendering defect, not a difference between runs.
//!
//! The fast test checks a fixed seed list. `DIVE_OUTPUT_AGREEMENT=all`
//! checks every fixture, and `DIVE_OUTPUT_AGREEMENT_REPORT=<path>` writes a
//! JSON summary of that sweep.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest, check_file};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Seeds for the fast test: one or more per verdict family, plus warnings
/// and output that span several lines.
const FAST_SEEDS: &[&str] = &[
    "conformance/corpus/selectseq-concat-type-capability/specifications/ConcatFirstTypeError/ConcatFirstTypeError.tla",
    "conformance/corpus/selectseq-concat-type-capability/specifications/ConcatSecondTypeError/ConcatSecondTypeError.tla",
    "conformance/corpus/selectseq-concat-type-capability/specifications/RightChildErrorBeforeKernel/RightChildErrorBeforeKernel.tla",
    "conformance/corpus/liveness-atom-error-timing/specifications/DelayedSiblingStateError/DelayedSiblingStateError.tla",
    "conformance/corpus/temporal-demand-primary-probes/specifications/CurrentStateBinPriority/CurrentStateBinPriority.tla",
    "conformance/corpus/temporal-demand-primary-probes/specifications/UnusedDemandTautology/UnusedDemandTautology.tla",
    "conformance/corpus/liveness-atom-error-timing/specifications/TableauInitAtomError/TableauInitAtomError.tla",
    "crates/dive-conformance/tests/fixtures/specs/ActionProperty/ActionProperty.tla",
    "crates/dive-conformance/tests/fixtures/specs/AssertFailure/AssertFailure.tla",
    "crates/dive-conformance/tests/fixtures/specs/Assumption/Assumption.tla",
    "crates/dive-conformance/tests/fixtures/specs/ConfigError/ConfigError.tla",
    "crates/dive-conformance/tests/fixtures/specs/Deadlock/Deadlock.tla",
    "crates/dive-conformance/tests/fixtures/specs/EvalError/EvalError.tla",
    "crates/dive-conformance/tests/fixtures/specs/InitInvariant/InitInvariant.tla",
    "crates/dive-conformance/tests/fixtures/specs/InitProperty/InitProperty.tla",
    "crates/dive-conformance/tests/fixtures/specs/InvariantViolation/InvariantViolation.tla",
    "crates/dive-conformance/tests/fixtures/specs/Liveness/Liveness.tla",
    "crates/dive-conformance/tests/fixtures/specs/LivenessStutter/LivenessStutter.tla",
    "crates/dive-conformance/tests/fixtures/specs/Overflow/Overflow.tla",
    "crates/dive-conformance/tests/fixtures/specs/ParseError/ParseError.tla",
    "crates/dive-conformance/tests/fixtures/specs/PrintOutput/PrintOutput.tla",
    "crates/dive-conformance/tests/fixtures/specs/SanyWarning/SanyWarning.tla",
    "crates/dive-conformance/tests/fixtures/specs/Success/Success.tla",
    "crates/dive-model/tests/fixtures/specs/SuSubscript/SuSubscript.tla",
    "crates/dive-eval/tests/fixtures/next/specs/NxUnchangedChanged/NxUnchangedChanged.tla",
];

/// States explored per run.
const MAX_STATES: u64 = 20_000;
/// Wall time per run.
const TIME_LIMIT: Duration = Duration::from_secs(10);

fn run(spec: &Path, compat_tlc: bool) -> Result<(Value, String)> {
    let report = check_file(&CheckRequest {
        file: spec.to_path_buf(),
        config: Some(spec.with_extension("cfg")),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        max_states: Some(MAX_STATES),
        time_limit: BudgetSetting::Explicit(TIME_LIMIT),
        ..CheckRequest::default()
    })?;
    Ok((report.to_json(), report.render_human()))
}

/// Differences between the two renderings, each `field: detail`.
#[derive(Default)]
struct Differences(Vec<String>);

impl Differences {
    fn expect(&mut self, field: &str, holds: bool, detail: impl FnOnce() -> String) {
        if !holds {
            self.0.push(format!("{field}: {}", detail()));
        }
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The header line: `dive check F: profile P, store S[ (fp N)], seed N, R
/// worker(s) requested, U used`.
fn compare_header(doc: &Value, lines: &[&str], diff: &mut Differences) {
    let header = lines.first().copied().unwrap_or_default();
    let options = &doc["options"];
    let profile = format!("profile {},", text(&options["profile"]));
    diff.expect("header.profile", header.contains(&profile), || {
        format!("{header:?} lacks {profile:?}")
    });
    let store = format!("store {}", text(&options["store"]));
    diff.expect("header.store", header.contains(&store), || {
        format!("{header:?} lacks {store:?}")
    });
    let seed = format!("seed {},", text(&options["seed"]));
    diff.expect("header.seed", header.contains(&seed), || {
        format!("{header:?} lacks {seed:?}")
    });
    let workers = &options["workers"];
    let workers = format!(
        "{} worker(s) requested, {} used",
        text(&workers["requested"]),
        text(&workers["used"])
    );
    diff.expect("header.workers", header.contains(&workers), || {
        format!("{header:?} lacks {workers:?}")
    });
}

/// The line naming the result, for the verdicts that have one.
fn expected_headline(doc: &Value) -> Option<String> {
    let property = text(&doc["property"]);
    let initial = doc["counterexample"]["length"].as_u64() == Some(1);
    Some(match doc["verdict"].as_str()? {
        "success" if doc["check_kind"] == "temporal" => {
            "Complete final temporal analysis: no error has been found.".to_owned()
        }
        "success" => "Model checking completed. No error has been found.".to_owned(),
        "invariant-violation" if initial => {
            format!("Error: Invariant {property} is violated by the initial state:")
        }
        "invariant-violation" => format!("Error: Invariant {property} is violated."),
        "action-property-violation" => format!("Error: Action property {property} is violated."),
        "property-violation" if initial => {
            format!("Error: Property {property} is violated by the initial state:")
        }
        "deadlock" => "Error: Deadlock reached.".to_owned(),
        _ => return None,
    })
}

fn compare_verdict(doc: &Value, lines: &[&str], diff: &mut Differences) {
    if let Some(headline) = expected_headline(doc) {
        diff.expect("verdict", lines.contains(&headline.as_str()), || {
            format!("{} without the line {headline:?}", text(&doc["verdict"]))
        });
    }
    if doc["verdict"] == "liveness-violation" {
        let listed: Vec<String> = doc["properties"]
            .as_array()
            .map(|a| a.iter().map(text).collect())
            .unwrap_or_default();
        let names = match listed.split_last() {
            None => String::new(),
            Some((last, [])) => last.clone(),
            Some((last, [first])) => format!("{first} and {last}"),
            Some((last, rest)) => format!("{}, and {last}", rest.join(", ")),
        };
        let headline = if listed.len() == 1 {
            format!("Temporal property {names} was violated.")
        } else {
            format!("Temporal properties {names} were violated.")
        };
        diff.expect(
            "properties",
            !listed.is_empty() && lines.contains(&headline.as_str()),
            || format!("JSON `properties` {listed:?} require human line {headline:?}"),
        );
        let property = text(&doc["property"]);
        diff.expect("property", property == names, || {
            format!("JSON `property` {property:?} differs from its complete list {names:?}")
        });
    }
}

fn compare_statistics(doc: &Value, lines: &[&str], diff: &mut Differences) {
    let s = &doc["statistics"];
    if s.is_null() {
        return;
    }
    if lines.iter().any(|line| line.starts_with("Capture ")) {
        let completion = match doc["execution"]["capture_completed"].as_bool() {
            Some(true) => "completed",
            Some(false) => "incomplete",
            None => "completion unknown",
        };
        let expected = format!(
            "Capture {completion}: {} generated, {} distinct, depth {}; no temporal verdict.",
            s["generated"], s["distinct"], s["depth"]
        );
        diff.expect(
            "statistics.capture",
            lines.contains(&expected.as_str()),
            || format!("human capture statistics lack {expected:?}"),
        );
    }
    let safety = format!(
        "{} states generated, {} distinct states found, {} states left on queue.",
        s["generated"], s["distinct"], s["left_on_queue"]
    );
    let temporal = format!(
        "{} states generated, {} distinct states, depth {}.",
        s["generated"], s["distinct"], s["depth"]
    );
    let depth = format!(
        "The depth of the complete state graph search is {}.",
        s["depth"]
    );
    let printed: Vec<&&str> = lines
        .iter()
        .filter(|l| l.contains(" states generated, "))
        .collect();
    if printed.is_empty() {
        // TLC prints no statistics for some results; neither does dive.
        return;
    }
    let agrees = lines.contains(&temporal.as_str())
        || (lines.contains(&safety.as_str()) && lines.contains(&depth.as_str()));
    diff.expect("statistics", agrees, || {
        format!("human {printed:?}, JSON {s}")
    });
}

fn compare_counterexample(doc: &Value, human: &str, lines: &[&str], diff: &mut Differences) {
    let cx = &doc["counterexample"];
    let numbered = lines.iter().filter(|l| l.starts_with("State ")).count();
    if cx.is_null() {
        diff.expect("counterexample", numbered == 0, || {
            format!("no JSON counterexample, but the human report numbers {numbered} states")
        });
        return;
    }
    let states = cx["states"].as_array().map_or(&[][..], Vec::as_slice);
    let initial_only =
        states.len() == 1 && expected_headline(doc).is_some_and(|h| h.ends_with("initial state:"));
    if !initial_only {
        diff.expect("counterexample.length", numbered == states.len(), || {
            format!(
                "JSON has {} states, the human report numbers {numbered}",
                states.len()
            )
        });
    }
    for (i, state) in states.iter().enumerate() {
        if let Some(text) = state["text"].as_str() {
            diff.expect(
                "counterexample.state",
                human.contains(text.trim_end()),
                || format!("state {} text {text:?} is not in the human report", i + 1),
            );
        } else if let Some(values) = cx["trace"]["states"].get(i).map(|s| s["values"].clone()) {
            let line = format!("State {}: {values}", i + 1);
            diff.expect(
                "counterexample.state",
                lines.contains(&line.as_str()),
                || format!("{line:?} is not in the human report"),
            );
        }
    }
    if let Some(back_to) = cx["back_to"].as_u64() {
        let line = format!("Back to state {back_to}.");
        diff.expect(
            "counterexample.back_to",
            lines.contains(&line.as_str()),
            || format!("JSON back_to {back_to}, human lacks {line:?}"),
        );
    }
}

/// Printed output and warnings, which may span several lines.
fn compare_output(doc: &Value, human: &str, diff: &mut Differences) {
    let mut at = 0;
    for entry in doc["output"].as_array().into_iter().flatten().map(text) {
        match human[at..].find(&entry) {
            Some(found) => at += found + entry.len(),
            None => diff
                .0
                .push(format!("output: {entry:?} is missing or out of order")),
        }
    }
    for warning in doc["warnings"].as_array().into_iter().flatten() {
        let entry = format!("Warning: {}", text(&warning["message"]));
        diff.expect("warnings", human.contains(&entry), || {
            format!("{entry:?} is not in the human report")
        });
    }
}

fn compare(doc: &Value, human: &str) -> Vec<String> {
    let lines: Vec<&str> = human.lines().collect();
    let mut diff = Differences::default();
    compare_header(doc, &lines, &mut diff);
    compare_verdict(doc, &lines, &mut diff);
    compare_statistics(doc, &lines, &mut diff);
    compare_counterexample(doc, human, &lines, &mut diff);
    compare_output(doc, human, &mut diff);
    diff.0
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
    agree: BTreeMap<String, usize>,
    differences: Vec<Value>,
}

fn check_seeds(seeds: &[String]) -> Result<Tally> {
    let workspace = workspace_root();
    let mut tally = Tally::default();
    for seed in seeds {
        for (profile, compat_tlc) in [("dive", false), ("tlc", true)] {
            tally.runs += 1;
            let (doc, human) = run(&workspace.join(seed), compat_tlc)?;
            let found = compare(&doc, &human);
            if found.is_empty() {
                *tally.agree.entry(text(&doc["verdict"])).or_default() += 1;
            } else {
                tally.differences.push(json!({
                    "seed": seed, "profile": profile, "verdict": doc["verdict"],
                    "differences": found,
                }));
            }
        }
    }
    Ok(tally)
}

fn summary(tally: &Tally) -> String {
    let agree: usize = tally.agree.values().sum();
    let mut out = format!(
        "{} runs: {agree} agree {:?}, {} differ\n",
        tally.runs,
        tally.agree,
        tally.differences.len()
    );
    for d in &tally.differences {
        let _ = writeln!(
            out,
            "  {} [{}] {}: {}",
            d["seed"], d["profile"], d["verdict"], d["differences"]
        );
    }
    out
}

#[test]
fn fixed_seeds_agree() -> Result<()> {
    let seeds: Vec<String> = FAST_SEEDS.iter().map(|s| (*s).to_owned()).collect();
    let tally = check_seeds(&seeds)?;
    assert!(tally.differences.is_empty(), "{}", summary(&tally));
    Ok(())
}

#[test]
fn every_fixture_renders_agreeing_reports_when_requested() -> Result<()> {
    if std::env::var("DIVE_OUTPUT_AGREEMENT").as_deref() != Ok("all") {
        return Ok(());
    }
    let seeds = all_seeds(&workspace_root());
    let tally = check_seeds(&seeds)?;
    eprintln!("{}", summary(&tally));
    if let Ok(path) = std::env::var("DIVE_OUTPUT_AGREEMENT_REPORT") {
        let report = json!({
            "seeds": seeds.len(),
            "runs": tally.runs,
            "agree": tally.agree,
            "differences": tally.differences,
        });
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    assert!(tally.differences.is_empty(), "{}", summary(&tally));
    Ok(())
}
