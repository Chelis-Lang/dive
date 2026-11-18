//! Public Rust callers independently initialize and run two native processes.
#![allow(
    clippy::expect_used,
    reason = "bounded authenticated process regression controls"
)]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, DistributedOptions, Store, check_file, prepare_core_worker};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn request(file: &Path, tlc: bool, exact: bool) -> CheckRequest {
    CheckRequest {
        file: file.into(),
        config: Some(file.with_extension("cfg")),
        compat_tlc: tlc,
        store: if exact {
            Store::Exact
        } else {
            Store::Fingerprint
        },
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-distributed-api-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("key"), [89; 32])?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Worker {
    child: Child,
    address: SocketAddr,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn worker(req: &CheckRequest, key: &Path) -> Result<Worker, Box<dyn std::error::Error>> {
    worker_at(
        req,
        key,
        "127.0.0.1:0".parse()?,
        None,
        0,
        Duration::from_secs(5),
    )
}
fn worker_at(
    req: &CheckRequest,
    key: &Path,
    address: SocketAddr,
    max_units: Option<u64>,
    delay_ms: u64,
    timeout: Duration,
) -> Result<Worker, Box<dyn std::error::Error>> {
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", "worker_process_entry", "--nocapture"])
        .env("DIVE_TEST_WORKER_SOURCE", &req.file)
        .env("DIVE_TEST_WORKER_KEY", key)
        .env("DIVE_TEST_WORKER_ADDRESS", address.to_string())
        .env("DIVE_TEST_WORKER_MAX", max_units.unwrap_or(0).to_string())
        .env("DIVE_TEST_WORKER_DELAY", delay_ms.to_string())
        .env("DIVE_TEST_WORKER_TIMEOUT", timeout.as_millis().to_string())
        .env(
            "DIVE_TEST_WORKER_TLC",
            if req.compat_tlc { "1" } else { "0" },
        )
        .env(
            "DIVE_TEST_WORKER_EXACT",
            if req.store == Store::Exact { "1" } else { "0" },
        )
        .env_remove("DIVE_CORE_OPT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("worker stdout")?;
    let mut guard = Worker {
        child,
        address: "127.0.0.1:0".parse()?,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) if line.starts_with("{\"address\"") => {
                    let _ = tx.send(line);
                    return;
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    });
    let ready: Value = serde_json::from_str(&rx.recv_timeout(Duration::from_secs(15))?)?;
    assert_eq!(ready["process_id"], guard.child.id());
    guard.address = ready["address"].as_str().ok_or("address")?.parse()?;
    Ok(guard)
}
fn observed(doc: &Value) -> Value {
    json!({"verdict":doc["verdict"],"statistics":doc["statistics"],"tlc":doc["tlc"],"property":doc["property"],"message":doc["message"],"counterexample":doc["counterexample"]})
}

// The integration-test executable is deliberately both caller and worker so
// executable identity is exact, with no test-only identity override.
#[test]
fn worker_process_entry() -> TestResult {
    let Some(file) = std::env::var_os("DIVE_TEST_WORKER_SOURCE") else {
        return Ok(());
    };
    let key = std::env::var_os("DIVE_TEST_WORKER_KEY").ok_or("key path")?;
    let req = request(
        Path::new(&file),
        std::env::var("DIVE_TEST_WORKER_TLC")? == "1",
        std::env::var("DIVE_TEST_WORKER_EXACT")? == "1",
    );
    let worker = prepare_core_worker(
        &req,
        Path::new(&key),
        Duration::from_millis(std::env::var("DIVE_TEST_WORKER_TIMEOUT")?.parse()?),
    )?;
    let listener = TcpListener::bind(std::env::var("DIVE_TEST_WORKER_ADDRESS")?)?;
    println!(
        "{}",
        json!({"address":listener.local_addr()?.to_string(),"process_id":std::process::id()})
    );
    std::io::stdout().flush()?;
    let max = std::env::var("DIVE_TEST_WORKER_MAX")?.parse::<u64>()?;
    let delay = std::env::var("DIVE_TEST_WORKER_DELAY")?.parse::<u64>()?;
    if max > 0 {
        worker.serve(&listener, Some(max), &|| false)?;
    } else if delay > 0 {
        loop {
            worker.serve(&listener, Some(1), &|| false)?;
            std::thread::sleep(Duration::from_millis(delay));
        }
    } else {
        worker.serve(&listener, None, &|| false)?;
    }
    Ok(())
}

#[test]
fn rust_consumers_use_two_processes_with_exact_words_and_owned_construction_history() -> TestResult
{
    check_cases(&[
        "tests/fixtures/core-m0/Bag.tla",
        "tests/fixtures/core-m0/CompoundTrace.tla",
        "tests/fixtures/checker/errors/specs/ErNextAfterDomain/ErNextAfterDomain.tla",
    ])
}
#[test]
fn owned_alias_receiver_and_normalization_demand_cross_actual_worker_processes() -> TestResult {
    check_cases(&[
        "../../conformance/corpus/construction-set-history/specifications/ExceptSetHandoff/ExceptSetHandoff.tla",
        "../../conformance/corpus/construction-set-history/specifications/MapAliasDemand/MapAliasDemand.tla",
        "../../conformance/corpus/construction-set-history/specifications/MapUnionViolation/MapUnionViolation.tla",
    ])
}
#[allow(
    clippy::too_many_lines,
    reason = "paired profile/store process checks retain independent word and mutation assertions together"
)]
fn check_cases(cases: &[&str]) -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    for relative in cases {
        for tlc in [false, true] {
            for exact in [false, true] {
                let mut req = request(&root.join(relative), tlc, exact);
                let baseline = check_file(&req)?;
                let expected = baseline.to_json();
                assert!(
                    !matches!(
                        expected["verdict"].as_str(),
                        Some("unsupported" | "internal-error")
                    ),
                    "{relative}: {expected}"
                );
                let a = worker(&req, &key)?;
                let b = worker(&req, &key)?;
                req.distributed = Some(DistributedOptions::new(
                    vec![a.address, b.address],
                    key.clone(),
                ));
                // Existing fallible frontier/predecessor routes remain real and
                // coordinator-owned; the worker never receives a file path.
                req.frontier_memory_bytes = Some(0);
                req.predecessor_memory_bytes = Some(0);
                let actual = check_file(&req)?;
                let doc = actual.to_json();
                assert_eq!(
                    observed(&doc),
                    observed(&expected),
                    "{relative}, {tlc}, {exact}"
                );
                assert_eq!(actual.exit_code(), baseline.exit_code());
                let facts = &doc["core"]["metrics"]["distributed"];
                assert_eq!(facts["workers"].as_array().ok_or("workers")?.len(), 2);
                assert!(facts["completed_units"].as_u64().ok_or("completed")? > 0);
                assert_eq!(facts["failures"], json!([]));
                assert!(
                    facts["workers"]
                        .as_array()
                        .ok_or("workers")?
                        .iter()
                        .all(|worker| worker["result_units"]
                            .as_u64()
                            .is_some_and(|units| units > 0)),
                    "both actual processes must return evaluated work: {facts}"
                );
                for id in [a.child.id(), b.child.id()] {
                    assert!(
                        facts["workers"]
                            .as_array()
                            .ok_or("workers")?
                            .iter()
                            .any(|worker| worker["process_id"] == id)
                    );
                }
                if !doc["counterexample"].is_null() {
                    let model = SpecModel::load(
                        &req.file,
                        req.config.as_ref().ok_or("config")?,
                        &TraceOptions {
                            compat_tlc: tlc,
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        },
                    )?;
                    let trace = TraceDoc::from_json(&doc)?;
                    let positive = model.try_validate(&trace)?;
                    assert_eq!(
                        positive.validation,
                        Validation::Valid,
                        "{relative}: {positive:?}"
                    );
                    let mut forged = doc["counterexample"]["trace"].clone();
                    if forged["ending"]["ec"].is_string() {
                        forged["ending"]["ec"] = "TLC_VALUE_ASSERT_FAILED".into();
                    } else {
                        forged["ending"]["kind"] = "invariant".into();
                        forged["ending"]["name"] = "absent-property".into();
                    }
                    let mutant = TraceDoc::from_json(&forged)?;
                    assert_ne!(
                        model.try_validate(&mutant)?.validation,
                        Validation::Valid,
                        "forged ending must fail: {relative}"
                    );
                    if relative.ends_with("CompoundTrace.tla")
                        || relative.ends_with("MapUnionViolation.tla")
                    {
                        let mut changed = trace.clone();
                        if relative.ends_with("CompoundTrace.tla") {
                            changed.states[1].insert("x".into(), json!({"seq":[99]}));
                        } else {
                            changed.states[1].insert("step".into(), json!(99));
                        }
                        assert_ne!(
                            model.try_validate(&changed)?.validation,
                            Validation::Valid,
                            "forged concrete step must fail: {relative}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn a_worker_process_restarts_and_uncommitted_units_continue_at_the_same_epoch() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = scratch.0.join("Wide.tla");
    std::fs::write(
        &file,
        "---- MODULE Wide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..127\nStep == x'=IF x<128 THEN x+128 ELSE x\nNext == Step \\/ Step\nInv == x<256\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for tlc in [false, true] {
        for exact in [false, true] {
            let mut req = request(&file, tlc, exact);
            let local = check_file(&req)?.to_json();
            let mut a = worker_at(
                &req,
                &key,
                "127.0.0.1:0".parse()?,
                Some(1),
                0,
                Duration::from_secs(5),
            )?;
            let b = worker_at(
                &req,
                &key,
                "127.0.0.1:0".parse()?,
                None,
                8,
                Duration::from_secs(5),
            )?;
            req.distributed = Some(DistributedOptions::new(
                vec![a.address, b.address],
                key.clone(),
            ));
            std::thread::scope(|scope| -> TestResult {
                let checking = scope.spawn(|| check_file(&req));
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while a.child.try_wait()?.is_none() {
                    if std::time::Instant::now() >= deadline {
                        return Err("first worker did not finish its bounded unit".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                let replacement =
                    worker_at(&req, &key, a.address, None, 0, Duration::from_secs(5))?;
                assert_ne!(replacement.child.id(), a.child.id());
                let actual = checking
                    .join()
                    .map_err(|_| "coordinator thread")??
                    .to_json();
                assert_eq!(observed(&actual), observed(&local), "{tlc}, {exact}");
                let facts = &actual["core"]["metrics"]["distributed"];
                assert!(facts["retries"].as_u64().ok_or("retries")? > 0, "{facts}");
                assert!(facts["epoch"].as_u64().ok_or("epoch")? >= 2, "{facts}");
                assert!(
                    facts["workers"].as_array().ok_or("workers")?.iter().any(
                        |worker| worker["process_id"] == replacement.child.id()
                            && worker["result_units"]
                                .as_u64()
                                .is_some_and(|units| units > 0)
                    ),
                    "replacement must evaluate work: {facts}"
                );
                assert_eq!(facts["completed_units"], 256);
                assert_eq!(facts["assigned_units"], 256);
                assert_eq!(facts["failures"], json!([]));
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn identity_key_and_unsupported_combinations_refuse_before_search_and_caps_never_hold() -> TestResult
{
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let base = request(&file, false, false);
    let a = worker(&base, &key)?;
    let b = worker(&base, &key)?;
    let policy = DistributedOptions::new(vec![a.address, b.address], key.clone());
    for mutation in 0..6 {
        let mut req = base.clone();
        req.distributed = Some(policy.clone());
        match mutation {
            0 => req.seed = Some(1),
            1 => req.fp = Some(1),
            2 => req.compat_tlc = true,
            3 => req.action_composition = Some(dive_eval::ActionComposition::Off),
            4 => {
                let cfg = scratch.0.join("different.cfg");
                std::fs::write(
                    &cfg,
                    "INIT Init\nNEXT Next\nINVARIANT Bound\n\\* changed bytes\n",
                )?;
                req.config = Some(cfg);
            }
            _ => {
                let source = scratch.0.join("Counter.tla");
                std::fs::write(
                    &source,
                    format!("{}\n\\* changed bytes\n", std::fs::read_to_string(&file)?),
                )?;
                req.file = source;
            }
        }
        let doc = check_file(&req)?.to_json();
        assert_eq!(
            doc["verdict"], "resource-limit",
            "identity mutation {mutation}: {doc}"
        );
        assert!(
            doc["message"]
                .as_str()
                .ok_or("message")?
                .contains("identity mismatch")
        );
        assert!(doc["statistics"].is_null());
        assert!(doc["counterexample"].is_null());
    }
    let wrong = scratch.0.join("wrong-key");
    std::fs::write(&wrong, [13; 32])?;
    let mut req = base.clone();
    let mut bad = policy.clone();
    bad.key_file = wrong;
    bad.timeout = Duration::from_millis(500);
    req.distributed = Some(bad);
    let doc = check_file(&req)?.to_json();
    assert_eq!(doc["verdict"], "resource-limit");
    assert!(doc["statistics"].is_null());
    assert!(doc["counterexample"].is_null());
    for mutation in 0..7 {
        let mut req = base.clone();
        req.distributed = Some(policy.clone());
        match mutation {
            0 => req.seed = None,
            1 => req.fp = None,
            2 => req.temporal = dive_api::TemporalMode::FinalShort,
            3 => req.sequential = true,
            4 => req.candidate_memory_bytes = Some(0),
            5 => req.seen_disk = Some(dive_api::SeenOptions::default()),
            _ => req.source_coverage = true,
        }
        assert!(
            check_file(&req).is_err(),
            "unsupported combination {mutation}"
        );
    }
    for depth in [false, true] {
        let mut req = base.clone();
        if depth {
            req.max_depth = Some(1);
        } else {
            req.max_states = Some(2);
        }
        let local = check_file(&req)?.to_json();
        req.distributed = Some(policy.clone());
        let doc = check_file(&req)?.to_json();
        assert_eq!(observed(&doc), observed(&local));
        assert_eq!(doc["verdict"], "stopped");
        assert!(doc["counterexample"].is_null());
    }
    Ok(())
}

#[test]
fn new_combination_refusals_precede_model_and_key_access() -> TestResult {
    let scratch = Scratch::new()?;
    let file = scratch.0.join("missing-model.tla");
    let key = scratch.0.join("missing-key");
    let policy = DistributedOptions::new(
        vec!["127.0.0.1:1".parse()?, "127.0.0.1:2".parse()?],
        key.clone(),
    );
    for compat in [false, true] {
        for compact in [false, true] {
            let mut req = request(&file, compat, true);
            let feature = if compact {
                req.store_compact_bytes = Some(0);
                "Store reclamation"
            } else {
                req.tlc_coverage = true;
                "TLC logical coverage"
            };
            let worker = prepare_core_worker(&req, &key, Duration::from_secs(1));
            assert!(
                matches!(worker, Err(dive_api::ApiError::Unsupported { feature: message })
                    if message == format!("worker preparation with {feature} is not implemented")),
                "worker must reject {feature} before loading the missing model/key"
            );
            req.distributed = Some(policy.clone());
            let coordinator = check_file(&req);
            assert!(
                matches!(coordinator, Err(dive_api::ApiError::Unsupported { feature: message })
                    if message == format!("distributed safety with {feature} is not implemented")),
                "coordinator must reject {feature} before loading the missing model/key"
            );
        }
    }
    assert!(!file.exists());
    assert!(!key.exists());
    Ok(())
}

#[test]
fn per_unit_successor_admission_does_not_cap_ordinary_local_checking() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = scratch.0.join("Fanout.tla");
    std::fs::write(
        &file,
        "---- MODULE Fanout ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == IF x=0 THEN x' \\in 0..16384 ELSE x'=x\n====\n",
    )?;
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    let mut req = request(&file, false, true);
    req.workers = Some(1);
    let local = check_file(&req)?.to_json();
    assert_eq!(local["verdict"], "success");
    assert_eq!(local["statistics"]["distinct"], 16385);
    // Reach the deterministic emission limit even on a slow CI host; this
    // control distinguishes protocol admission from an incidental time stop.
    let a = worker_at(
        &req,
        &key,
        "127.0.0.1:0".parse()?,
        None,
        0,
        Duration::from_secs(20),
    )?;
    let b = worker_at(
        &req,
        &key,
        "127.0.0.1:0".parse()?,
        None,
        0,
        Duration::from_secs(20),
    )?;
    let mut policy = DistributedOptions::new(vec![a.address, b.address], key);
    policy.timeout = Duration::from_secs(20);
    req.distributed = Some(policy);
    let actual = check_file(&req)?.to_json();
    assert_eq!(actual["verdict"], "resource-limit", "{actual}");
    assert!(actual["counterexample"].is_null());
    assert!(
        actual["message"]
            .as_str()
            .ok_or("message")?
            .contains("successor admission")
    );
    Ok(())
}

/// A refusal of the distributed worker identity holds at any budget, so it
/// precedes a time budget spent by start-up work (#973). Source admission
/// refuses every user definition with a runtime effect, even one Core never
/// evaluates, so the run is refused, not stopped, in both profiles.
#[test]
fn identity_refusal_precedes_a_startup_budget_stop() -> TestResult {
    use dive_api::BudgetSetting;
    let scratch = Scratch::new()?;
    let file = scratch.0.join("Effectful.tla");
    // `Big` is start-up work far longer than the 10 ms limit; `Log` is never
    // used, so only the identity's source admission sees its effect.
    std::fs::write(
        &file,
        "---- MODULE Effectful ----\nEXTENDS Naturals,FiniteSets,TLC\nVARIABLE x\nBig == Cardinality({<<i,j>>:i \\in 1..500,j \\in 1..500}) > 0\nLog(n) == PrintT(n)\nInit == x = 0\nNext == Big /\\ x' = (x + 1) % 3\n====\n",
    )?;
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    // No worker is reached: the refusal comes before any connection.
    let policy = DistributedOptions::new(
        vec!["127.0.0.1:1".parse()?, "127.0.0.1:2".parse()?],
        scratch.0.join("key"),
    );
    for tlc in [false, true] {
        let mut req = request(&file, tlc, false);
        req.time_limit = BudgetSetting::Explicit(Duration::from_millis(10));
        req.distributed = Some(policy.clone());
        let report = check_file(&req)?;
        let doc = report.to_json();
        assert_eq!(doc["verdict"], "unsupported", "tlc={tlc}: {doc}");
        assert!(
            doc["message"]
                .as_str()
                .ok_or("message")?
                .contains("runtime effects or temporal effects in Log"),
            "tlc={tlc}: {doc}"
        );
        assert_eq!(report.exit_code(), 3, "tlc={tlc}");
        assert!(doc["stopped"].is_null(), "tlc={tlc}: {doc}");
        // Without distribution the same budget stops at start-up.
        req.distributed = None;
        let local = check_file(&req)?.to_json();
        assert_eq!(local["verdict"], "stopped", "tlc={tlc}: {local}");
        assert_eq!(
            local["stopped"]["phase"], "check-startup",
            "tlc={tlc}: {local}"
        );
    }
    Ok(())
}
