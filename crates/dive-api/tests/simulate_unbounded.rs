//! Simulation phase S6 (ADR 0162 §9, D-040): sets Init or Next cannot
//! enumerate because they are infinite are sampled with a stated, seeded
//! distribution in dive's profile, and every drawn member is checked by the
//! independent validator. Cases G3 to G6 are the pinned TLC receipts in
//! `crates/dive-sim/tests/fixtures/temporal/oracle.json`, where TLC fails
//! with GENERAL 1000; that error under `--compat=tlc` is tested in
//! `simulate_workers.rs`.

use dive_api::{KEEP_ALL_TRACES, SampleUnbounded, SimulateRequest, simulate_file};
use serde_json::Value;
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn written(module: &str, text: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-s6-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join(format!("{module}.tla"));
    std::fs::write(&file, text)?;
    std::fs::write(file.with_extension("cfg"), cfg)?;
    Ok((Scratch(directory), file))
}

fn fixture(module: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/temporal")
        .join(format!("{module}.tla"));
    written(module, &std::fs::read_to_string(source)?, cfg)
}

fn spec(module: &str, body: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    written(
        module,
        &format!(
            "---- MODULE {module} ----\nEXTENDS Integers, Sequences\nVARIABLE x\n{body}\n====\n"
        ),
        cfg,
    )
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

fn traces(json: &Value) -> &[Value] {
    json["traces"].as_array().map_or(&[], Vec::as_slice)
}

fn every_trace_valid(json: &Value) {
    assert!(!traces(json).is_empty(), "{json}");
    for trace in traces(json) {
        assert_eq!(trace["validation"]["result"], "valid", "{trace}");
    }
}

/// The values of `x` in every kept state.
fn values(json: &Value) -> Vec<Value> {
    traces(json)
        .iter()
        .flat_map(|trace| {
            trace["document"]["states"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .map(|state| state["values"]["x"].clone())
        .collect()
}

/// G3 to G6 in dive's profile: Init over `Nat` and successors over `Nat`
/// are drawn, by enumeration and by the constructive generator, and every
/// kept trace validates; the report names the distribution, the seed and
/// each set with the action that drew from it.
#[test]
fn unbounded_init_and_successors_are_sampled_and_validated() -> Result {
    for (module, sets) in [
        ("InfInit", &["Init line 5", "Next line 6"][..]),
        ("InfNext", &["Next line 6"][..]),
    ] {
        let (_scratch, file) = fixture(module, "INIT Init\nNEXT Next\nINVARIANT Small\n")?;
        for (generation, constructive_init) in [(false, false), (true, false), (true, true)] {
            let json = simulate_file(&SimulateRequest {
                file: file.clone(),
                seed: Some(1),
                traces: 5,
                depth: Some(20),
                generation,
                constructive_init,
                ..Default::default()
            })?
            .to_json();
            let case = format!("{module} generate={generation} constructive={constructive_init}");
            assert_eq!(json["result"], "sampled", "{case}: {json}");
            assert_eq!(json["exit_code"], 0, "{case}");
            assert_eq!(json["options"]["sample_unbounded"], "geometric", "{case}");
            assert_eq!(json["options"]["sample_mean"], 8, "{case}");
            assert_eq!(json["statistics"]["traces_started"], 5, "{case}");
            assert_eq!(json["statistics"]["traces_stalled"], 0, "{case}");
            every_trace_valid(&json);
            let sampled: Vec<&str> = json["statistics"]["sampled_sets"]
                .as_array()
                .map(|sets| sets.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            assert_eq!(sampled.len(), sets.len(), "{case}: {sampled:?}");
            for (text, at) in sampled.iter().zip(sets) {
                assert!(text.starts_with("Nat in <"), "{case}: {text}");
                assert!(text.contains(at), "{case}: {text}");
            }
            let notes = json["notes"].to_string();
            assert!(notes.contains("geometric with mean 8"), "{case}: {notes}");
            assert!(notes.contains("seed 1"), "{case}: {notes}");
            assert!(values(&json).iter().all(|x| x.as_u64().is_some()), "{case}");
            if module == "InfNext" {
                // The members drawn for `\E n \in Nat` are the step's
                // witnesses: the increment of `x`.
                let trace = &traces(&json)[0]["document"]["states"];
                for pair in trace.as_array().map_or(&[][..], Vec::as_slice).windows(2) {
                    let step = pair[1]["values"]["x"].as_u64().unwrap_or(0)
                        - pair[0]["values"]["x"].as_u64().unwrap_or(0);
                    assert_eq!(pair[1]["witnesses"], serde_json::json!([step]), "{case}");
                }
            }
        }
    }
    Ok(())
}

/// The same seed draws the same members at any worker count, and another
/// seed draws others.
#[test]
fn draws_are_reproducible_at_one_and_four_workers() -> Result {
    for module in ["InfInit", "InfNext"] {
        let (_scratch, file) = fixture(module, "INIT Init\nNEXT Next\nINVARIANT Small\n")?;
        let request = SimulateRequest {
            file,
            seed: Some(11),
            traces: 40,
            depth: Some(15),
            keep_traces: Some(KEEP_ALL_TRACES),
            ..Default::default()
        };
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
        assert_eq!(comparable(&one), comparable(&four), "{module}");
        every_trace_valid(&one);
        let other = simulate_file(&SimulateRequest {
            seed: Some(12),
            ..request
        })?
        .to_json();
        assert_ne!(values(&one), values(&other), "{module}");
    }
    Ok(())
}

/// The stated distributions: `Nat` geometric with the requested mean, `Int`
/// with both signs, `Seq(S)` of geometric length over members of `S`, and
/// records, tuples and functions component by component. Every drawn value
/// is a member of its set, which the validator checks on every kept state.
#[test]
#[allow(
    clippy::cast_precision_loss,
    reason = "averages of small counts and draws"
)]
fn distributions_are_the_stated_ones() -> Result {
    let draws = |module: &str, body: &str, mean: u64| -> Result<Value> {
        let (_scratch, file) = spec(module, body, "INIT Init\nNEXT Next\n")?;
        let json = simulate_file(&SimulateRequest {
            file,
            seed: Some(5),
            traces: 400,
            depth: Some(5),
            keep_traces: Some(KEEP_ALL_TRACES),
            sample_unbounded: Some(SampleUnbounded::Geometric { mean }),
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["result"], "sampled", "{module}: {json}");
        every_trace_valid(&json);
        Ok(json)
    };
    let average = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    for mean in [2_u64, 8, 30] {
        let json = draws("Nats", "Init == x \\in Nat\nNext == x' \\in Nat", mean)?;
        let xs: Vec<f64> = values(&json).iter().filter_map(Value::as_f64).collect();
        assert_eq!(xs.len(), 400 * 6);
        let observed = average(&xs);
        let expected = mean as f64;
        assert!(
            (observed - expected).abs() < expected * 0.15 + 0.2,
            "mean {mean}: {observed}"
        );
        assert!(json["notes"].to_string().contains(&format!("mean {mean}")));
    }
    let json = draws("Ints", "Init == x \\in Int\nNext == x' \\in Int", 8)?;
    let xs: Vec<i64> = values(&json).iter().filter_map(Value::as_i64).collect();
    assert!(xs.iter().any(|&x| x < 0) && xs.iter().any(|&x| x > 0));
    let json = draws(
        "Seqs",
        "Init == x = <<>>\nNext == x' \\in Seq({\"a\", \"b\"})",
        4,
    )?;
    let lengths: Vec<f64> = values(&json)
        .iter()
        .filter_map(|x| x["seq"].as_array().map(|s| s.len() as f64))
        .collect();
    assert!(
        (average(&lengths) - 4.0).abs() < 1.0,
        "{}",
        average(&lengths)
    );
    for x in values(&json) {
        for e in x["seq"].as_array().map_or(&[][..], Vec::as_slice) {
            assert!(e == "a" || e == "b", "{e}");
        }
    }
    let json = draws(
        "Parts",
        "Init == x = [a |-> 0, b |-> 1, p |-> <<0, \"u\">>, f |-> <<0, 0>>]\n\
         Next == x = x /\\ \\E r \\in [a : Nat, b : {1, 2}], p \\in Nat \\X {\"u\"}, f \\in [1..2 -> Int] :\n\
         \x20 x' = [a |-> r.a, b |-> r.b, p |-> p, f |-> f]",
        8,
    )?;
    let sets = json["statistics"]["sampled_sets"].to_string();
    for set in [
        "[a : Nat, b : {1, 2}]",
        "Nat \\\\X {\\\"u\\\"}",
        "[{1, 2} -> Int]",
    ] {
        assert!(sets.contains(set), "{set} in {sets}");
    }
    Ok(())
}

/// A violation reached through drawn members is validated like any other.
#[test]
fn violations_through_draws_are_validated() -> Result {
    let (_scratch, file) = spec(
        "Grow",
        "Init == x \\in Nat\nNext == \\E n \\in Nat : x' = x + n\nInv == x < 40",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for workers in [1, 4] {
        let report = simulate_file(&SimulateRequest {
            file: file.clone(),
            seed: Some(3),
            traces: 20,
            depth: Some(10),
            workers: Some(workers),
            ..Default::default()
        })?;
        let json = report.to_json();
        assert_eq!(json["result"], "violation", "{json}");
        assert_eq!(json["tlc_code"], 2110);
        assert_eq!(json["violation_validated"], true);
        assert_eq!(report.exit_code(), 10);
    }
    Ok(())
}

/// A draw that leads to no admitted state shows nothing about the state's
/// successors: it is drawn again, then the trace ends early, counted in
/// `traces_stalled`, and no deadlock is reported. A state with no successor
/// whose step drew nothing is still a deadlock.
#[test]
fn draws_without_a_successor_are_never_a_deadlock() -> Result {
    let (_scratch, file) = spec(
        "Far",
        "Init == x = 0\nNext == \\E n \\in Nat : n > 1000 /\\ x' = n",
        "INIT Init\nNEXT Next\n",
    )?;
    let json = simulate_file(&SimulateRequest {
        file,
        seed: Some(1),
        traces: 6,
        depth: Some(10),
        ..Default::default()
    })?
    .to_json();
    assert_eq!(json["result"], "sampled", "{json}");
    assert_eq!(json["statistics"]["traces_stalled"], 6);
    assert!(json["notes"].to_string().contains("This is not a deadlock"));
    every_trace_valid(&json);
    let (_scratch, file) = spec(
        "Down",
        "Init == x \\in Nat\nNext == x > 5 /\\ x' = x - 1",
        "INIT Init\nNEXT Next\n",
    )?;
    let json = simulate_file(&SimulateRequest {
        file,
        seed: Some(1),
        traces: 6,
        depth: Some(40),
        ..Default::default()
    })?
    .to_json();
    assert_eq!(json["result"], "violation", "{json}");
    assert_eq!(json["tlc_code"], 2114);
    assert_eq!(json["violation_validated"], true);
    Ok(())
}

/// Sets without a stated distribution stay named refusals, and a boolean
/// quantifier over `Nat` (here under a negation) is never sampled: it keeps
/// TLC's error.
#[test]
fn unsupported_sets_and_boolean_quantifiers_are_not_sampled() -> Result {
    for (module, body, result) in [
        (
            "Subsets",
            "Init == x = {}\nNext == x' \\in SUBSET Nat",
            "unsupported",
        ),
        (
            "Unions",
            "Init == x = 0\nNext == x' \\in Nat \\cup {-1}",
            "unsupported",
        ),
        (
            "Negated",
            "Init == x = 0\nNext == ~(\\E n \\in Nat : n = x) /\\ x' = x + 1",
            "evaluation-error",
        ),
    ] {
        let (_scratch, file) = spec(module, body, "INIT Init\nNEXT Next\n")?;
        let json = simulate_file(&SimulateRequest {
            file,
            seed: Some(1),
            traces: 3,
            depth: Some(5),
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["result"], result, "{module}: {json}");
        if result == "unsupported" {
            assert!(
                json["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("no stated distribution")),
                "{module}: {json}"
            );
        }
        assert_eq!(json["statistics"]["sampled_sets"], serde_json::json!([]));
    }
    Ok(())
}
