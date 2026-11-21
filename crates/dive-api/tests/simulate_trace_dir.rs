//! Simulation phase S5 (ADR 0162, D-042): `--trace-dir` writes every
//! sampled trace, independently validated, to its own file as it commits;
//! `--trace-format json|tla` writes TLC's `-dumpTrace` layouts, checked
//! against the pinned oracle's receipts in
//! `crates/dive-sim/tests/fixtures/trace-formats`; `--trace-ids A..B`
//! regenerates selected traces of a run.

use dive_api::{SimulateRequest, TraceFormat, simulate_file, trace_file_name};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Result<Scratch> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-s5-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    Ok(Scratch(directory))
}

fn receipts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/trace-formats")
}

fn spec(scratch: &Scratch, module: &str, body: &str, cfg: &str) -> Result<PathBuf> {
    let file = scratch.0.join(format!("{module}.tla"));
    std::fs::write(
        &file,
        format!("---- MODULE {module} ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n"),
    )?;
    std::fs::write(file.with_extension("cfg"), cfg)?;
    Ok(file)
}

/// Every file in `dir`, by name.
fn files(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        out.insert(
            entry.file_name().to_string_lossy().into_owned(),
            std::fs::read(entry.path())?,
        );
    }
    Ok(out)
}

fn dump(module: &str, format: TraceFormat, compat_tlc: bool) -> Result<(Scratch, Vec<u8>)> {
    let out = scratch()?;
    let dir = out.0.join("traces");
    let json = simulate_file(&SimulateRequest {
        file: receipts().join(format!("{module}.tla")),
        compat_tlc,
        seed: Some(1),
        traces: 1,
        depth: Some(10),
        trace_dir: Some(dir.clone()),
        trace_format: format,
        ..Default::default()
    })?
    .to_json();
    assert_eq!(json["result"], "violation", "{module}: {json}");
    assert_eq!(json["statistics"]["traces_written"], 1, "{module}");
    let text = std::fs::read(dir.join(trace_file_name(0, format)))?;
    Ok((out, text))
}

/// TLC's layouts, byte for byte, where the trace and its records do not
/// depend on TLC's random choices or on the field order of records.
#[test]
fn tlc_layouts_are_the_oracle_receipts() -> Result {
    for (module, compat) in [
        ("Ord", &[false, true][..]),
        ("One", &[false, true][..]),
        ("Fmt", &[true][..]),
    ] {
        for (format, extension) in [(TraceFormat::TlcJson, "json"), (TraceFormat::TlcTla, "tla")] {
            let receipt = std::fs::read(receipts().join(format!("{module}.tlc-dump.{extension}")))?;
            for &compat_tlc in compat {
                let (_scratch, text) = dump(module, format, compat_tlc)?;
                assert_eq!(
                    String::from_utf8_lossy(&text),
                    String::from_utf8_lossy(&receipt),
                    "{module} {extension} tlc={compat_tlc}"
                );
            }
        }
    }
    // In dive's profile a record lists its fields in canonical order; the
    // values are TLC's.
    let (_scratch, text) = dump("Fmt", TraceFormat::TlcJson, false)?;
    let mut ours: Value = serde_json::from_slice(&text)?;
    let mut theirs: Value =
        serde_json::from_slice(&std::fs::read(receipts().join("Fmt.tlc-dump.json"))?)?;
    for document in [&mut ours, &mut theirs] {
        if let Some(vars) = document["vars"].as_array_mut() {
            vars.sort_by_key(ToString::to_string);
        }
    }
    assert_eq!(ours, theirs);
    let (_scratch, text) = dump("Fmt", TraceFormat::TlcTla, false)?;
    assert!(String::from_utf8_lossy(&text).contains("<<1, [doubled |-> 0, x |-> 0]>>"));
    // An ALIAS that is not a record is not written, as in TLC, and unnamed
    // disjuncts take the enclosing definition's name.
    let (_scratch, text) = dump("Un", TraceFormat::TlcJson, false)?;
    let document: Value = serde_json::from_slice(&text)?;
    assert_eq!(document["vars"], serde_json::json!(["x"]));
    assert_eq!(
        document["counterexample"]["state"][0],
        serde_json::json!([1, {"x": 0}])
    );
    assert_eq!(document["counterexample"]["action"][0][1]["name"], "Next");
    Ok(())
}

/// Every trace is written once it commits, validated, and the directory is
/// the same at any worker count. A file in dive's format is the trace's
/// report entry, which `TraceDoc` reads.
#[test]
fn every_trace_is_written_in_id_order_at_any_worker_count() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Walk",
        "Init == x \\in 0..3\nNext == \\E d \\in {1, 2} : x' = (x + d) % 7",
        "INIT Init\nNEXT Next\n",
    )?;
    let mut directories = Vec::new();
    for workers in [1, 4] {
        let dir = out.0.join(format!("w{workers}"));
        let json = simulate_file(&SimulateRequest {
            file: file.clone(),
            seed: Some(9),
            traces: 40,
            depth: Some(12),
            workers: Some(workers),
            trace_dir: Some(dir.clone()),
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["result"], "sampled", "{json}");
        assert_eq!(json["statistics"]["traces_written"], 40);
        assert_eq!(json["options"]["trace_format"], "dive");
        // The report still keeps its own ten traces.
        assert_eq!(json["traces"].as_array().map(Vec::len), Some(10));
        let written = files(&dir)?;
        assert_eq!(written.len(), 40);
        for id in 0..40 {
            let text = &written[&trace_file_name(id, TraceFormat::Dive)];
            let entry: Value = serde_json::from_slice(text)?;
            assert_eq!(entry["id"], id);
            assert_eq!(entry["validation"]["result"], "valid", "{entry}");
            let document = dive_api::trace::TraceDoc::parse(std::str::from_utf8(text)?)?;
            assert_eq!(document.states.len(), 13);
            // A kept trace's file is its report entry.
            if let Some(kept) = json["traces"]
                .as_array()
                .and_then(|traces| traces.get(usize::try_from(id).ok()?))
            {
                assert_eq!(&entry, kept);
            }
        }
        directories.push(written);
    }
    assert_eq!(directories[0], directories[1]);
    Ok(())
}

/// A trace is fixed by the seed and its id: `--trace-ids A..B` writes the
/// same files as the whole run, and its report keeps those traces.
#[test]
fn trace_ids_regenerate_the_same_traces() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Ids",
        "Init == x \\in 0..3\nNext == \\E d \\in {1, 2, 3} : x' = (x + d) % 11",
        "INIT Init\nNEXT Next\n",
    )?;
    for format in [TraceFormat::Dive, TraceFormat::TlcJson, TraceFormat::TlcTla] {
        let whole = out.0.join(format!("whole-{}", format.name()));
        let part = out.0.join(format!("part-{}", format.name()));
        simulate_file(&SimulateRequest {
            file: file.clone(),
            seed: Some(4),
            traces: 30,
            depth: Some(8),
            trace_dir: Some(whole.clone()),
            trace_format: format,
            ..Default::default()
        })?;
        let json = simulate_file(&SimulateRequest {
            file: file.clone(),
            seed: Some(4),
            trace_ids: Some((12, 17)),
            depth: Some(8),
            workers: Some(3),
            trace_dir: Some(part.clone()),
            trace_format: format,
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["options"]["trace_ids"], serde_json::json!([12, 17]));
        assert_eq!(json["statistics"]["traces_started"], 6);
        let ids: Vec<u64> = json["traces"]
            .as_array()
            .map(|traces| traces.iter().filter_map(|t| t["id"].as_u64()).collect())
            .unwrap_or_default();
        assert_eq!(ids, (12..=17).collect::<Vec<_>>());
        let whole = files(&whole)?;
        let part = files(&part)?;
        assert_eq!(part.len(), 6);
        for (name, text) in &part {
            assert_eq!(Some(text), whole.get(name), "{name}");
        }
    }
    Ok(())
}

/// The run stops at the deciding trace: the directory holds the traces up
/// to it and no partial files, and the deciding trace ends as the report's.
#[test]
fn the_directory_ends_at_the_deciding_trace() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Stop",
        "Init == x = 0\nNext == \\E d \\in 0..40 : x' = x + d\nInv == x < 150",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let dir = out.0.join("traces");
    let json = simulate_file(&SimulateRequest {
        file,
        seed: Some(2),
        traces: 1000,
        depth: Some(6),
        workers: Some(4),
        trace_dir: Some(dir.clone()),
        ..Default::default()
    })?
    .to_json();
    assert_eq!(json["result"], "violation", "{json}");
    let deciding = json["traces"]
        .as_array()
        .and_then(|traces| traces.last())
        .and_then(|trace| trace["id"].as_u64())
        .ok_or("deciding trace")?;
    assert!(deciding > 0, "a later trace decides");
    let written = files(&dir)?;
    assert_eq!(written.len() as u64, deciding + 1);
    assert_eq!(json["statistics"]["traces_written"], deciding + 1);
    assert!(written.keys().all(|name| !name.starts_with('.')));
    let entry: Value =
        serde_json::from_slice(&written[&trace_file_name(deciding, TraceFormat::Dive)])?;
    assert_eq!(
        entry["document"]["ending"],
        serde_json::json!({"kind": "invariant", "name": "Inv"})
    );
    assert_eq!(entry["validation"]["result"], "valid");
    Ok(())
}

/// Draws from unbounded sets are recorded in the files as in the report.
#[test]
fn drawn_witnesses_reach_the_files() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Draw",
        "Init == x = 0\nNext == \\E n \\in Nat : x' = x + n",
        "INIT Init\nNEXT Next\n",
    )?;
    let dir = out.0.join("traces");
    let json = simulate_file(&SimulateRequest {
        file,
        seed: Some(5),
        traces: 15,
        depth: Some(5),
        trace_dir: Some(dir.clone()),
        ..Default::default()
    })?
    .to_json();
    assert_eq!(json["result"], "sampled", "{json}");
    let written = files(&dir)?;
    assert_eq!(written.len(), 15);
    for text in written.values() {
        let entry: Value = serde_json::from_slice(text)?;
        assert_eq!(entry["validation"]["result"], "valid");
        assert!(
            entry["document"]["states"][1]["witnesses"].is_array(),
            "{entry}"
        );
    }
    Ok(())
}

/// The directory must be absent or empty, so every file in it is the run's.
#[test]
fn a_directory_with_files_is_refused() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Full",
        "Init == x = 0\nNext == x' = x",
        "INIT Init\nNEXT Next\n",
    )?;
    let dir = out.0.join("traces");
    std::fs::create_dir(&dir)?;
    std::fs::write(dir.join("old.json"), "{}")?;
    let refused = simulate_file(&SimulateRequest {
        file,
        seed: Some(1),
        traces: 2,
        trace_dir: Some(dir),
        ..Default::default()
    });
    assert!(
        matches!(&refused, Err(dive_api::ApiError::Rejected { message }) if message.contains("not empty")),
        "{refused:?}"
    );
    Ok(())
}

/// An initial state that violates an invariant is found before any trace
/// runs; it is written as the run's first trace.
#[test]
fn an_initial_violation_is_the_first_file() -> Result {
    let out = scratch()?;
    let file = spec(
        &out,
        "Iv",
        "Init == x \\in 0..5\nNext == x' = x\nInv == x < 3",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for (ids, first) in [(None, 0), (Some((4, 9)), 4)] {
        let dir = out.0.join(format!("from-{first}"));
        let json = simulate_file(&SimulateRequest {
            file: file.clone(),
            seed: Some(1),
            traces: if ids.is_some() { 0 } else { 5 },
            trace_ids: ids,
            trace_dir: Some(dir.clone()),
            trace_format: TraceFormat::TlcTla,
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["result"], "violation", "{json}");
        assert_eq!(json["traces"][0]["id"], first);
        let written = files(&dir)?;
        assert_eq!(written.len(), 1);
        let text = String::from_utf8_lossy(&written[&trace_file_name(first, TraceFormat::TlcTla)])
            .into_owned();
        assert!(
            text.contains("[action |-> {}, state |-> {<<1, [x |-> "),
            "{text}"
        );
    }
    Ok(())
}

/// A temporal counterexample is written as the report gives it: a lasso
/// with its false properties, and a bad prefix, each validated.
#[test]
fn temporal_counterexamples_reach_the_files() -> Result {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/temporal");
    let cycle = std::fs::read_to_string(fixtures.join("Cycle.tla"))?;
    let unbounded = std::fs::read_to_string(fixtures.join("Unbounded.tla"))?
        .replace("====", "StartsBounded == (x = 0) => [](x < 4)\n====");
    for (module, source, cfg, kind) in [
        (
            "Cycle",
            cycle,
            "SPECIFICATION FairSpec\nPROPERTY InfOftenZero EventuallyFive InfOftenFive\n",
            "lasso",
        ),
        (
            "Unbounded",
            unbounded,
            "SPECIFICATION FairSpec\nPROPERTY StartsBounded\n",
            "bad-prefix",
        ),
    ] {
        let out = scratch()?;
        let file = out.0.join(format!("{module}.tla"));
        std::fs::write(&file, source)?;
        std::fs::write(file.with_extension("cfg"), cfg)?;
        let dir = out.0.join("traces");
        let json = simulate_file(&SimulateRequest {
            file,
            seed: Some(1),
            traces: 5,
            depth: Some(10),
            trace_dir: Some(dir.clone()),
            ..Default::default()
        })?
        .to_json();
        assert_eq!(json["result"], "violation", "{json}");
        assert_eq!(json["violation_validated"], true);
        let kept = json["traces"]
            .as_array()
            .and_then(|traces| traces.last())
            .ok_or("deciding trace")?;
        let id = kept["id"].as_u64().ok_or("id")?;
        let entry: Value = serde_json::from_slice(&std::fs::read(
            dir.join(trace_file_name(id, TraceFormat::Dive)),
        )?)?;
        assert_eq!(&entry, kept, "{module}");
        assert_eq!(entry["document"]["ending"]["kind"], kind);
        assert_eq!(entry["validation"]["result"], "valid");
        if kind == "lasso" {
            assert_eq!(
                entry["document"]["properties"],
                serde_json::json!(["EventuallyFive", "InfOftenFive"])
            );
        }
    }
    Ok(())
}
