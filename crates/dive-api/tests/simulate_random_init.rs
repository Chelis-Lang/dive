//! `RandomElement` in Init samples per trace (ADR 0162 §8, D-044) and keeps
//! TLC's verdicts, error classes and printed texts, with every sampled trace
//! independently validated.
#![allow(clippy::expect_used, reason = "bounded fixture setup and assertions")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_api::SimulateRequest;
use dive_api::trace::{Check, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/random-init")
}
fn request(row: &Value, compat_tlc: bool) -> SimulateRequest {
    {
        let mut built = SimulateRequest::default();
        built.file = fixtures().join(format!("{}.tla", row["model"].as_str().expect("model")));
        built.seed = row["seed"].as_i64();
        built.aril = row["aril"].as_u64().expect("aril");
        built.depth =
            Some(usize::try_from(row["depth"].as_u64().expect("depth")).expect("depth bound"));
        built.traces = row["requested_traces"].as_u64().expect("traces");
        built.compat_tlc = compat_tlc;
        built
    }
}
/// Runs `request` and checks that every trace it reports validates.
fn sampled(request: &SimulateRequest) -> (Value, u8) {
    let report = dive_api::simulate_file(request).expect("report");
    let json = report.to_json();
    for trace in json["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{trace}");
    }
    (json, report.exit_code())
}

/// The texts `report` printed, in order; with `distinct`, each once.
fn printed(report: &Value, distinct: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for event in ["startup_output", "runtime_output"]
        .iter()
        .flat_map(|field| report[*field].as_array().into_iter().flatten())
    {
        if event["kind"] == "print" && !(distinct && out.contains(&event["text"])) {
            out.push(event["text"].clone());
        }
    }
    out
}

/// TLC's verdict, error class, violated property and printed texts for
/// `row`. Values differ: Init draws per trace (D-044), so a print in Init
/// is printed once per trace, and for that model only the distinct texts
/// are compared.
fn assert_agrees(row: &Value, request: &SimulateRequest) {
    let (report, exit) = sampled(request);
    let label = &row["label"];
    let expected = match row["physical_exit"].as_u64() {
        Some(0) => "sampled",
        Some(12) => "violation",
        _ => "evaluation-error",
    };
    assert_eq!(report["result"], expected, "{label}: {report}");
    assert_eq!(
        report["tlc_code"],
        row["codes"].get(0).cloned().unwrap_or(Value::Null),
        "{label}"
    );
    if request.compat_tlc {
        assert_eq!(
            Some(u64::from(exit)),
            row["physical_exit"].as_u64(),
            "{label}"
        );
    }
    if expected == "violation" {
        assert_eq!(
            report["observed_violation"]["name"],
            row["violated_property"]
        );
        assert_eq!(report["violation_validated"], true, "{label}");
    }
    let init_prints = row["model"] == "InitRandomStoredPrint";
    assert_eq!(
        json!(printed(&report, init_prints)),
        row["printed"],
        "{label}"
    );
}

#[test]
fn actual_primary_init_words_errors_and_output_keep_their_boundaries() {
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("oracle.json")).expect("generated primary data"),
    )
    .expect("primary JSON");
    assert_eq!(oracle["oracle"], "tlc-2026.10.06.014338");
    let mut runs = 0;
    for row in oracle["primary_probes"].as_array().expect("primary cases") {
        for compat_tlc in [true, false] {
            if !compat_tlc && row["aril"] != 0 {
                continue;
            }
            assert_agrees(row, &request(row, compat_tlc));
            runs += 1;
        }
    }
    assert_eq!(runs, 20);
}

#[test]
fn seed_prefix_statistics_and_true_word_mutations_remain_independent() {
    let file = fixtures().join("InitRandomInterval.tla");
    for compat_tlc in [false, true] {
        let mut request = SimulateRequest::default();
        file.clone_into(&mut request.file);
        request.seed = Some(0);
        request.depth = Some(3);
        request.traces = 2;
        request.compat_tlc = compat_tlc;
        let (first, _) = sampled(&request);
        let (again, _) = sampled(&request);
        assert_eq!(first["result"], "sampled");
        assert_eq!(
            first["traces"], again["traces"],
            "a seed reproduces its samples"
        );
        let starts: Vec<_> = first["traces"]
            .as_array()
            .expect("traces")
            .iter()
            .map(|trace| trace["document"]["states"][0]["values"]["x"].clone())
            .collect();
        assert_ne!(
            starts[0], starts[1],
            "each trace draws its own initial state"
        );
        // The independent validator still admits any drawn initial value and
        // rejects forged Init, Next and ending claims.
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                ..Default::default()
            },
        )
        .expect("fresh model");
        let doc = TraceDoc::behavior(vec![json!({"x":17}), json!({"x":18})]).expect("word");
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
        for (state, value, check) in [(0, -1, Check::Init), (1, -999, Check::Next)] {
            let mut forged = doc.clone();
            forged.states[state]["x"] = json!(value);
            assert!(
                matches!(model.validate(&forged).validation,Validation::Invalid(f) if f.check==check)
            );
        }
        let false_ending = doc.clone().with_ending(Ending::Invariant("Safe".into()));
        assert!(
            matches!(model.validate(&false_ending).validation,Validation::Invalid(f) if f.check==Check::Ending)
        );
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Files(PathBuf);
impl Files {
    fn new(init: &str, next: &str, safe: &str, extra: &str, config_extra: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dive-sim-init-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).expect("unique test directory");
        std::fs::write(dir.join("Boundary.tla"),format!("---- MODULE Boundary ----\nEXTENDS Integers,TLC\n{extra}\nVARIABLE x\nInit=={init}\nNext=={next}\nSafe=={safe}\n====\n")).expect("source");
        std::fs::write(
            dir.join("Boundary.cfg"),
            format!("INIT Init\nNEXT Next\nINVARIANT Safe\n{config_extra}"),
        )
        .expect("config");
        Self(dir)
    }
    fn request(&self) -> SimulateRequest {
        {
            let mut built = SimulateRequest::default();
            built.file = self.0.join("Boundary.tla");
            built.seed = Some(0);
            built.depth = Some(1);
            built.traces = 1;
            built.compat_tlc = true;
            built
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn nested_correlated_and_captured_randomness_has_independent_sample_proof() {
    for (init, next, safe, extra, config) in [
        (
            "x=RandomElement({RandomElement({7})})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        (
            "x=RandomElement({7}) /\\ x=RandomElement({7})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        ("x=RandomElement({7})+0", "UNCHANGED x", "TRUE", "", ""),
        (
            "x=Apply(RandomElement,{7})",
            "UNCHANGED x",
            "TRUE",
            "Apply(Draw(_),S)==Draw(S)",
            "",
        ),
        (
            "x=Draw({7})",
            "UNCHANGED x",
            "TRUE",
            "CONSTANT Draw(_)",
            "CONSTANT Draw <- RandomElement\n",
        ),
        (
            "x=RandomElement({7})",
            "UNCHANGED x",
            "RandomElement({TRUE})",
            "",
            "",
        ),
        (
            "x=RandomElement(IF FALSE THEN RandomElement({{7}}) ELSE {7})",
            "IF FALSE THEN x'=RandomElement({9}) ELSE UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        (
            "x=RandomElement(IF PrintT(\"before-inner\") THEN {RandomElement({7})} ELSE {})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
    ] {
        let (report, _) = sampled(&Files::new(init, next, safe, extra, config).request());
        assert_eq!(report["result"], "sampled", "{init}: {report}");
        let document = &report["traces"][0]["document"];
        assert_eq!(document["states"][0]["values"]["x"], 7);
        // A constant-level invariant is evaluated once at start-up, as TLC
        // does (its warning 2149), so its draw is a start-up draw.
        let startup = if safe == "TRUE" {
            Value::Null
        } else {
            json!([true])
        };
        assert_eq!(document["startup_draws"], startup, "{init}");
    }
}

#[test]
fn nested_domain_errors_precede_the_unreached_outer_native_invocation() {
    let files = Files::new(
        "x=RandomElement(IF PrintT(\"before-inner-error\") THEN {RandomElement(42)} ELSE {})",
        "UNCHANGED x",
        "TRUE",
        "",
        "",
    );
    let (report, _) = sampled(&files.request());
    assert_eq!(report["result"], "evaluation-error", "{report}");
    assert_eq!(report["tlc_code"], 1000);
    assert_eq!(
        json!(printed(&report, false)),
        json!(["\"before-inner-error\""])
    );
}
