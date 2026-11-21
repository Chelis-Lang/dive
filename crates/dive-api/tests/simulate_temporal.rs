//! Simulation phase S3 (ADR 0162 §4): temporal `PROPERTY` on sampled
//! traces, against the pinned TLC receipts in
//! `crates/dive-sim/tests/fixtures/temporal/oracle.json`. Every reported
//! counterexample is independently validated, the report is the same at 1
//! and 4 workers, and simulation never reports that a property holds.

use dive_api::{SimulateRequest, simulate_file};
use serde_json::{Value, json};
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Removes a per-test scratch directory.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn oracle() -> Result<Value> {
    Ok(serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/temporal/oracle.json"
    ))?)
}

fn case(oracle: &Value, id: &str) -> Result<Value> {
    oracle["cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"] == id))
        .cloned()
        .ok_or_else(|| format!("no oracle case {id}").into())
}

/// Writes `module` from the temporal fixtures with configuration `cfg`.
fn fixture(module: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/temporal")
        .join(format!("{module}.tla"));
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-s3-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join(format!("{module}.tla"));
    std::fs::write(&file, std::fs::read_to_string(source)?)?;
    std::fs::write(file.with_extension("cfg"), cfg)?;
    Ok((Scratch(directory), file))
}

/// The request a receipt's flags describe; `num` is the total trace count.
fn request(case: &Value, file: PathBuf, compat_tlc: bool) -> Result<SimulateRequest> {
    let flags: Vec<&str> = case["flags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let after = |flag: &str| {
        flags
            .iter()
            .position(|f| *f == flag)
            .and_then(|i| flags.get(i + 1))
            .copied()
    };
    Ok(SimulateRequest {
        file,
        compat_tlc,
        seed: Some(1),
        traces: after("-simulate")
            .and_then(|num| num.strip_prefix("num="))
            .ok_or("num")?
            .parse()?,
        depth: Some(after("-depth").ok_or("depth")?.parse()?),
        no_deadlock: flags.contains(&"-deadlock"),
        ..Default::default()
    })
}

fn comparable(report: &Value) -> Value {
    let mut report = report.clone();
    if let Some(options) = report["options"].as_object_mut() {
        options.remove("workers_requested");
        options.remove("workers_used");
        options.remove("budget");
    }
    if let Some(map) = report.as_object_mut() {
        map.remove("budget");
        map.remove("resources");
    }
    report
}

/// Runs at 1 and 4 workers, asserts the reports agree, and checks that
/// every kept trace validated and nothing claims a proof.
fn run(request: &SimulateRequest) -> Result<Value> {
    let one = simulate_file(&SimulateRequest {
        workers: Some(1),
        ..request.clone()
    })?
    .to_json();
    let four = simulate_file(&SimulateRequest {
        workers: Some(4),
        ..request.clone()
    })?
    .to_json();
    assert_eq!(comparable(&four), comparable(&one), "4 workers");
    assert_eq!(one["model_proved"], false, "{one}");
    for trace in one["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{one}");
    }
    Ok(one)
}

/// What dive reports for a receipt: `None` for a sample without a
/// violation, else the deciding code and, for a temporal violation, the
/// counterexample's ending kind.
#[derive(Debug, PartialEq)]
enum Expect {
    Sampled,
    Temporal(&'static str),
    Safety(u64),
}

fn deciding(report: &Value) -> Expect {
    if report["result"] == "sampled" {
        return Expect::Sampled;
    }
    let code = report["tlc_code"].as_u64().unwrap_or(0);
    if code != 2116 {
        return Expect::Safety(code);
    }
    let index = report["observed_violation"]["trace"].as_u64().unwrap_or(0);
    let ending = &report["traces"][usize::try_from(index).unwrap_or(0)]["document"]["ending"];
    match ending["kind"].as_str() {
        Some("lasso") => Expect::Temporal("lasso"),
        Some("stuttering") => Expect::Temporal("stuttering"),
        Some("bad-prefix") => Expect::Temporal("bad-prefix"),
        _ => Expect::Temporal("other"),
    }
}

/// Under `--compat=tlc` dive answers each receipt's truth where TLC is
/// sound, and matches its exit status, deciding code, ending kind and, for
/// a deterministic specification, generated count. V1 and Y2 are D-037: dive answers the truth where
/// TLC's view or symmetric loop closure does not.
#[test]
fn receipts_match_tlc_where_tlc_is_sound() -> Result {
    use Expect::{Safety, Sampled, Temporal};
    let oracle = oracle()?;
    let cases = [
        ("C1", Temporal("lasso")),
        ("C2", Temporal("lasso")),
        ("C3", Temporal("lasso")),
        ("C4", Sampled),
        ("C5", Sampled),
        ("C6", Safety(2110)),
        ("C7", Sampled),
        ("C8", Sampled),
        ("C9", Temporal("stuttering")),
        ("T1", Sampled),
        ("T2", Temporal("stuttering")),
        ("T3", Temporal("stuttering")),
        ("T4", Safety(2114)),
        ("U1", Sampled),
        ("U2", Sampled),
        ("U3", Sampled),
        ("U5", Sampled),
        ("U8", Temporal("stuttering")),
        ("U9", Sampled),
        ("U10", Safety(2114)),
        ("U10d", Sampled),
        ("F1", Temporal("stuttering")),
        ("F2", Sampled),
        ("F3", Sampled),
        ("S1", Temporal("lasso")),
        ("S2", Sampled),
        ("V1", Sampled),
        ("V2", Sampled),
        ("Y1", Sampled),
        ("Y2", Temporal("lasso")),
        ("Y3", Temporal("lasso")),
    ];
    for (id, expected) in cases {
        let receipt = case(&oracle, id)?;
        let (_scratch, file) = fixture(
            receipt["module"].as_str().ok_or("module")?,
            receipt["cfg"].as_str().ok_or("cfg")?,
        )?;
        let report = run(&request(&receipt, file, true)?)?;
        assert_eq!(deciding(&report), expected, "{id}: {report}");
        if id == "V1" || id == "Y2" {
            continue;
        }
        assert_eq!(report["exit_code"], receipt["exit"], "{id}: {report}");
        // Fair and Strong choose between actions at random, so only their
        // verdicts are TLC's; the other modules are deterministic.
        let random = ["Fair", "Strong"].contains(&receipt["module"].as_str().unwrap_or_default());
        if matches!(expected, Safety(_)) || random {
            continue;
        }
        assert_eq!(
            report["generated"]
                .as_u64()
                .or(report["statistics"]["generated"].as_u64()),
            receipt["generated"].as_u64(),
            "{id}: {report}"
        );
        if let Temporal(_) = expected {
            assert_eq!(report["violation_validated"], true, "{id}: {report}");
            assert_eq!(
                receipt["stuttering"] == true,
                expected == Temporal("stuttering"),
                "{id}"
            );
        }
    }
    Ok(())
}

/// The false-property set of the reported word is complete, in
/// configuration order (ADR 0126), and the trace classes and per-property
/// counts say what the sample covered.
#[test]
fn a_lasso_names_every_property_false_on_it() -> Result {
    let (_scratch, file) = fixture(
        "Cycle",
        "SPECIFICATION FairSpec\nPROPERTY InfOftenZero EventuallyFive InfOftenFive\n",
    )?;
    let report = run(&SimulateRequest {
        file,
        seed: Some(1),
        traces: 5,
        depth: Some(10),
        ..Default::default()
    })?;
    assert_eq!(report["result"], "violation", "{report}");
    assert_eq!(report["observed_violation"]["name"], "EventuallyFive");
    let trace = &report["traces"][0]["document"];
    assert_eq!(
        trace["properties"],
        json!(["EventuallyFive", "InfOftenFive"])
    );
    assert_eq!(trace["closure"], "concrete");
    let properties = &report["statistics"]["properties"];
    assert_eq!(properties[0]["refuted_by_trace"], Value::Null);
    assert_eq!(properties[1]["refuted_by_trace"], 0);
    assert_eq!(properties[2]["refuted_by_trace"], 0);
    assert_eq!(report["statistics"]["trace_classes"]["violated"], 1);
    Ok(())
}

/// C7 and U1 (D-038): a trace cut by the depth under weak fairness has no
/// fair behavior, and the report says so rather than staying silent.
#[test]
fn truncated_traces_are_counted_unchecked() -> Result {
    let oracle = oracle()?;
    for id in ["C7", "U1"] {
        let receipt = case(&oracle, id)?;
        let (_scratch, file) = fixture(
            receipt["module"].as_str().ok_or("module")?,
            receipt["cfg"].as_str().ok_or("cfg")?,
        )?;
        let req = request(&receipt, file, true)?;
        let report = run(&req)?;
        assert_eq!(report["result"], "sampled", "{id}: {report}");
        assert_eq!(report["options"]["liveness_extend"], 0);
        let traces = report["statistics"]["traces_started"].clone();
        let property = &report["statistics"]["properties"][0];
        assert_eq!(property["checked_traces"], 0, "{id}: {report}");
        assert_eq!(property["unchecked_traces"], traces, "{id}: {report}");
        assert_eq!(property["unchecked_reasons"]["truncated"], traces);
        assert_eq!(report["statistics"]["trace_classes"]["truncated"], traces);
        let human = simulate_file(&req)?.render_human();
        assert!(
            human.contains("could not decide it") && human.contains("(not a proof)"),
            "{human}"
        );
    }
    Ok(())
}

/// D-039: in the default profile a trace cut by the depth continues until
/// it closes or terminates. C7's cycle closes after one more step and its
/// violation is found; U1 never closes and stays unchecked. An explicit
/// zero restores TLC's behavior.
#[test]
fn extension_decides_what_the_depth_cut() -> Result {
    let oracle = oracle()?;
    let receipt = case(&oracle, "C7")?;
    let (_scratch, file) = fixture("Cycle", receipt["cfg"].as_str().ok_or("cfg")?)?;
    let report = run(&request(&receipt, file.clone(), false)?)?;
    assert_eq!(report["options"]["liveness_extend"], 2);
    assert_eq!(deciding(&report), Expect::Temporal("lasso"), "{report}");
    assert_eq!(report["violation_validated"], true);
    let off = run(&SimulateRequest {
        liveness_extend: Some(0),
        ..request(&receipt, file, false)?
    })?;
    assert_eq!(off["result"], "sampled", "{off}");

    let receipt = case(&oracle, "C8")?;
    let (_scratch, file) = fixture("Cycle", receipt["cfg"].as_str().ok_or("cfg")?)?;
    let report = run(&request(&receipt, file, false)?)?;
    assert_eq!(report["result"], "sampled", "{report}");
    let classes = &report["statistics"]["trace_classes"];
    assert_eq!(classes["extended_closed"], 20, "{report}");
    assert_eq!(report["statistics"]["properties"][0]["checked_traces"], 20);

    let receipt = case(&oracle, "U1")?;
    let (_scratch, file) = fixture("Unbounded", receipt["cfg"].as_str().ok_or("cfg")?)?;
    let report = run(&request(&receipt, file, false)?)?;
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(
        report["statistics"]["properties"][0]["unchecked_reasons"]["truncated"],
        20
    );
    Ok(())
}

/// The default profile's answer for every receipt is its truth where a
/// trace can decide it; truncated traces leave the rest unchecked.
#[test]
fn the_default_profile_answers_the_truth() -> Result {
    let oracle = oracle()?;
    for receipt in oracle["cases"].as_array().into_iter().flatten() {
        let id = receipt["id"].as_str().unwrap_or_default();
        let module = receipt["module"].as_str().unwrap_or_default();
        let cfg = receipt["cfg"].as_str().unwrap_or_default();
        // A deadlock decides before any property (T4, U10).
        if !cfg.contains("PROPERTY")
            || receipt["exit"] == 11
            || ["A", "G"].iter().any(|p| id.starts_with(p))
            || ["U11", "U12", "U13", "U14"].contains(&id)
        {
            continue;
        }
        let (_scratch, file) = fixture(module, cfg)?;
        let report = run(&request(receipt, file, false)?)?;
        let truth = receipt["truth"].as_str().unwrap_or_default();
        match truth {
            "holds" => assert_eq!(report["result"], "sampled", "{id}: {report}"),
            "violated" => {
                // U-cases with `<>` can only be cut by the depth.
                let undecidable = report["result"] == "sampled"
                    && report["statistics"]["properties"]
                        .as_array()
                        .is_none_or(|p| p.iter().all(|p| p["checked_traces"] == 0));
                assert!(
                    report["result"] == "violation" || undecidable,
                    "{id}: {report}"
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// A finite refutation: under weak fairness no trace of `Unbounded` has a
/// fair behavior, but `(x = 0) => [](x < 4)` is false on every extension
/// of `x = 0 .. 4`. The default profile reports that shortest prefix;
/// `--compat=tlc` does not look for it.
#[test]
fn bad_prefixes_refute_on_truncated_traces() -> Result {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-sim/tests/fixtures/temporal/Unbounded.tla"),
    )?;
    let source = source.replace("====", "StartsBounded == (x = 0) => [](x < 4)\n====");
    let (scratch, file) = fixture(
        "Unbounded",
        "SPECIFICATION FairSpec\nPROPERTY StartsBounded\n",
    )?;
    std::fs::write(&file, source)?;
    let base = SimulateRequest {
        file,
        seed: Some(1),
        traces: 5,
        depth: Some(10),
        ..Default::default()
    };
    let report = run(&base)?;
    assert_eq!(
        deciding(&report),
        Expect::Temporal("bad-prefix"),
        "{report}"
    );
    assert_eq!(report["violation_validated"], true);
    let document = &report["traces"][0]["document"];
    assert_eq!(document["states"].as_array().map(Vec::len), Some(5));
    assert_eq!(document["version"], 7);
    let tlc = run(&SimulateRequest {
        compat_tlc: true,
        ..base
    })?;
    assert_eq!(tlc["result"], "sampled", "{tlc}");
    drop(scratch);
    Ok(())
}

/// A budget stop leaves the property unchecked; it is never a verdict.
#[test]
fn a_budget_stop_is_unchecked() -> Result {
    let (_scratch, file) = fixture("Cycle", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive\n")?;
    let report = run(&SimulateRequest {
        file,
        seed: Some(1),
        traces: 5,
        depth: Some(10),
        liveness_budget: Some(1),
        ..Default::default()
    })?;
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(report["options"]["liveness_budget"], 1);
    let property = &report["statistics"]["properties"][0];
    assert_eq!(property["unchecked_reasons"]["budget"], 5, "{report}");
    assert_eq!(property["checked_traces"], 0);
    Ok(())
}
