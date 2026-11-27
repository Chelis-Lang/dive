//! Release-qualification timing of one native temporal job (dive
//! docs/developer/release-performance-plan.md, S1). Not part of Sail: the
//! dive runbook copies this file into `crates/sail-dive/examples/` on the
//! measurement host and builds it with `--locked`.
//!
//! Usage: temporal_bench <request.json> <worker> <worker-sha256> <deadline-ms>
//! Prints one JSON object with the phase timings of one repetition.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use sail_dive::{Operation, Request, client};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn materialize(request: &Request, dir: &Path) -> (PathBuf, PathBuf, Vec<PathBuf>) {
    let mut parents = std::collections::BTreeSet::new();
    for (path, source) in &request.model.files {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, source).unwrap();
        parents.insert(full.parent().unwrap().to_path_buf());
    }
    (
        dir.join(&request.model.root),
        dir.join(&request.model.config),
        parents.into_iter().collect(),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args[1] == "--itf" {
        // Convert a native dive trace document to Sail's ITF transport.
        let native: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
        let itf = sail_dive::native_to_itf(&native).unwrap();
        println!("{}", serde_json::to_string(&itf).unwrap());
        return;
    }
    let request: Request =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let worker = PathBuf::from(&args[2]);
    let sha = &args[3];
    let deadline = Duration::from_millis(args[4].parse().unwrap());
    let tmp = std::env::temp_dir().join(format!("sail-temporal-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // 1. Request encoding (the parent's serde_json::to_vec).
    let t = Instant::now();
    let bytes = serde_json::to_vec(&request).unwrap();
    let encode_ms = ms(t);

    // 2. Worker SHA-256 verification and private copy (as client::WorkerImage::freeze).
    let t = Instant::now();
    let mut image = Vec::new();
    std::fs::File::open(&worker).unwrap().read_to_end(&mut image).unwrap();
    let digest = Sha256::digest(&image);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(&hex, sha, "worker digest");
    let copy = tmp.join("sail-runtime-worker");
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o500).open(&copy).unwrap();
        f.write_all(&image).unwrap();
    }
    let verify_copy_ms = ms(t);

    // 3. Process spawn floor: the private copy, empty environment, closed stdin,
    //    until it exits (it refuses the empty request).
    let t = Instant::now();
    let mut child = Command::new(&copy)
        .env_clear()
        .current_dir(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let spawn_spawned_ms = ms(t);
    let _ = child.wait();
    let spawn_exit_ms = ms(t);

    // 4. The D.I.V.E. library call alone, in process, with the worker's settings.
    let (root, config, libraries) = materialize(&request, &tmp.join("model"));
    let t = Instant::now();
    let dive_call = match &request.operation {
        Operation::Check { max_states, max_depth } => {
            let r = dive_api::check_file(&dive_api::CheckRequest {
                file: root,
                config: Some(config),
                library_paths: libraries,
                store: dive_api::Store::Exact,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                temporal: dive_api::TemporalMode::Ordinary,
                time_limit: dive_api::BudgetSetting::Unlimited,
                max_memory: dive_api::BudgetSetting::Unlimited,
                max_states: Some(*max_states),
                max_depth: Some(*max_depth),
                ..Default::default()
            })
            .map(|r| r.verdict().to_owned());
            Some(format!("{r:?}"))
        }
        Operation::Simulate { traces, depth } => {
            let mut request = dive_api::SimulateRequest::default();
            request.file = root;
            request.config = Some(config);
            request.library_paths = libraries;
            request.seed = Some(0);
            request.workers = Some(1);
            request.traces = *traces;
            request.depth = Some(*depth);
            let r = dive_api::simulate_file(&request)
            .map(|r| r.to_json()["result"].clone());
            Some(format!("{r:?}"))
        }
        Operation::Validate { .. } => None,
    };
    let dive_call_ms = dive_call.as_ref().map(|_| ms(t));

    // 5. The worker body in process: materialization, admission, D.I.V.E.
    //    call and independent replay of returned traces.
    let t = Instant::now();
    let body = sail_dive::execute(&request);
    let worker_body_ms = ms(t);

    // 6. Response encoding (worker side) and parent decoding plus validation.
    let response_bytes = serde_json::to_vec(&body).unwrap();
    let t = Instant::now();
    let decoded = sail_dive::decode_response(&response_bytes).unwrap();
    sail_dive::validate_response(&request, &decoded).unwrap();
    let decode_validate_ms = ms(t);

    // 7. The complete embedded job through the isolated worker.
    let t = Instant::now();
    let result = client::run(&worker, sha, &request, deadline, &AtomicBool::new(false));
    let embedded_ms = ms(t);
    let (outcome, completion, distinct, depth) = match &result {
        client::RunResult::Completed(r) => (
            serde_json::to_value(r.outcome).unwrap(),
            serde_json::to_value(r.completion).unwrap(),
            r.native_report.pointer("/statistics/distinct").cloned()
                .unwrap_or(serde_json::Value::Null),
            r.native_report.pointer("/statistics/depth").cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
        other => (serde_json::Value::String(format!("{other:?}")), serde_json::Value::Null,
                  serde_json::Value::Null, serde_json::Value::Null),
    };
    let _ = std::fs::remove_dir_all(&tmp);
    println!(
        "{}",
        serde_json::json!({
            "request_bytes": bytes.len(),
            "response_bytes": response_bytes.len(),
            "encode_ms": encode_ms,
            "verify_copy_ms": verify_copy_ms,
            "spawn_ms": spawn_spawned_ms,
            "spawn_exit_ms": spawn_exit_ms,
            "dive_call_ms": dive_call_ms,
            "dive_call_result": dive_call,
            "worker_body_ms": worker_body_ms,
            "worker_body_outcome": serde_json::to_value(body.outcome).unwrap(),
            "decode_validate_ms": decode_validate_ms,
            "embedded_ms": embedded_ms,
            "outcome": outcome,
            "completion": completion,
            "distinct_states": distinct,
            "depth": depth,
            "backend_revision": match &result { client::RunResult::Completed(r) => serde_json::Value::String(r.backend_revision.clone()), _ => serde_json::Value::Null },
        })
    );
}
