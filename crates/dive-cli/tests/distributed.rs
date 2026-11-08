//! Separate native worker processes perform actual safety source evaluation.
#![allow(
    clippy::expect_used,
    reason = "bounded local process regression controls"
)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "dive-distributed-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir)?;
        std::fs::write(dir.join("key"), [43; 32])?;
        Ok(Self(dir))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Worker {
    child: Child,
    address: String,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn command(file: &Path, profile: &str, store: &str, operation: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
    command
        .arg(operation)
        .arg(file)
        .args(["--engine=core", "--seed=0", "--fp=0", "--output=json"])
        .arg(format!("--compat={profile}"))
        .arg(format!("--store={store}"))
        .env_remove("DIVE_ENGINE")
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CORE_OPT")
        .env_remove("DIVE_CHAOS");
    command
}
fn worker(
    file: &Path,
    profile: &str,
    store: &str,
    key: &Path,
) -> Result<Worker, Box<dyn std::error::Error>> {
    let mut child = command(file, profile, store, "worker")
        .arg("--worker-key")
        .arg(key)
        .arg("--listen=127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("worker stdout")?;
    let stderr = child.stderr.take().ok_or("worker stderr")?;
    let mut guard = Worker {
        child,
        address: String::new(),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let line = rx.recv_timeout(Duration::from_secs(15))??;
    if line.is_empty() {
        let mut error = String::new();
        std::io::Read::read_to_string(&mut BufReader::new(stderr), &mut error)?;
        return Err(format!("worker failed before readiness: {error}").into());
    }
    let ready: Value = serde_json::from_str(&line)?;
    assert_eq!(ready["status"], "ready", "{ready}");
    assert_eq!(ready["process_id"], guard.child.id());
    assert_eq!(ready["model_proved"], false);
    guard.address = ready["address"].as_str().ok_or("worker address")?.into();
    Ok(guard)
}
fn observable(report: &Value) -> Value {
    json!({"verdict":report["verdict"], "statistics":report["statistics"], "tlc":report["tlc"],
        "property":report["property"], "message":report["message"], "counterexample":report["counterexample"]})
}
fn validate(file: &Path, profile: &str, report: &Value, scratch: &Path) -> TestResult {
    let trace = &report["counterexample"]["trace"];
    if trace.is_null() {
        return Ok(());
    }
    let path = scratch.join("word.json");
    std::fs::write(&path, serde_json::to_vec(trace)?)?;
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_dive"))
            .arg("validate-trace")
            .arg(file)
            .arg(format!("--compat={profile}"))
            .args(["--output", "json", "--seed", "0", "--fp", "0"])
            .arg("--trace")
            .arg(&path)
            .env_remove("DIVE_COMPAT")
            .output()
    };
    let result = run()?;
    let doc: Value = serde_json::from_slice(&result.stdout).map_err(|e| {
        format!(
            "validation: {e}; {}",
            String::from_utf8_lossy(&result.stderr)
        )
    })?;
    assert!(
        result.status.success(),
        "{doc}, {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(doc["result"], "valid", "{doc}");
    let mut changed = trace.clone();
    // A false ending claim must not validate as the real selected word.
    if let Some(ending) = changed.get_mut("ending").and_then(Value::as_object_mut) {
        ending.insert("kind".into(), "invariant".into());
        ending.insert("name".into(), "absent-property".into());
        std::fs::write(&path, serde_json::to_vec(&changed)?)?;
        // Rejected either as an invalid word or, for error endings, as an
        // inadmissible claim; neither may succeed.
        let result = run()?;
        assert!(
            !result.status.success(),
            "mutated claim: {}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
    Ok(())
}

#[test]
fn new_combination_refusals_precede_cli_model_and_key_access() -> TestResult {
    let scratch = Scratch::new()?;
    let file = scratch.0.join("missing-model.tla");
    let key = scratch.0.join("missing-key");
    for profile in ["dive", "tlc"] {
        for (flag, feature) in [
            ("--store-compact-bytes=0", "Store reclamation"),
            ("--coverage=0", "TLC logical coverage"),
        ] {
            for operation in ["worker", "check"] {
                let mut run = command(&file, profile, "exact", operation);
                run.arg("--worker-key").arg(&key).arg(flag);
                if operation == "check" {
                    run.args([
                        "--distributed-worker=127.0.0.1:1",
                        "--distributed-worker=127.0.0.1:2",
                    ]);
                }
                let output = run.output()?;
                let message = String::from_utf8(output.stderr)?;
                assert_eq!(output.status.code(), Some(3), "{message}");
                let role = if operation == "worker" {
                    "worker preparation"
                } else {
                    "distributed safety"
                };
                assert!(
                    message.contains(&format!("{role} with {feature} is not implemented")),
                    "{message}"
                );
                assert_eq!(output.stdout, [] as [u8; 0]);
            }
        }
    }
    assert!(!file.exists());
    assert!(!key.exists());
    Ok(())
}

#[test]
fn two_process_workers_match_local_safety_in_both_profiles_and_stores() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    for relative in [
        "../../docs/user/examples/Counter.tla",
        "../dive-api/tests/fixtures/core-m0/CompoundTrace.tla",
        "../dive-api/tests/fixtures/core-m0/Dead.tla",
        "../dive-api/tests/fixtures/checker/errors/specs/ErInvReplayError/ErInvReplayError.tla",
    ] {
        let file = root.join(relative);
        for profile in ["dive", "tlc"] {
            for store in ["fp", "exact"] {
                let local = command(&file, profile, store, "check").output()?;
                let expected: Value = serde_json::from_slice(&local.stdout).map_err(|e| {
                    format!(
                        "local {relative}: {e}; {}",
                        String::from_utf8_lossy(&local.stderr)
                    )
                })?;
                assert!(
                    !matches!(
                        expected["verdict"].as_str(),
                        Some("unsupported" | "internal-error")
                    ),
                    "{expected}"
                );
                let a = worker(&file, profile, store, &key)?;
                let b = worker(&file, profile, store, &key)?;
                assert_ne!(a.child.id(), b.child.id());
                let actual = command(&file, profile, store, "check")
                    .arg("--worker-key")
                    .arg(&key)
                    .args([
                        "--distributed-worker",
                        &a.address,
                        "--distributed-worker",
                        &b.address,
                    ])
                    .output()?;
                let report: Value = serde_json::from_slice(&actual.stdout).map_err(|e| {
                    format!(
                        "distributed {relative}: {e}; {}",
                        String::from_utf8_lossy(&actual.stderr)
                    )
                })?;
                assert_eq!(
                    observable(&report),
                    observable(&expected),
                    "{relative}, {profile}, {store}; {}",
                    String::from_utf8_lossy(&actual.stderr)
                );
                assert_eq!(actual.status.code(), local.status.code());
                let facts = &report["core"]["metrics"]["distributed"];
                assert_eq!(facts["configured_workers"], 2, "{report}");
                assert_eq!(facts["workers"].as_array().ok_or("worker facts")?.len(), 2);
                for id in [a.child.id(), b.child.id()] {
                    assert!(
                        facts["workers"]
                            .as_array()
                            .ok_or("worker facts")?
                            .iter()
                            .any(|worker| worker["process_id"] == id)
                    );
                }
                assert!(
                    facts["completed_units"].as_u64().ok_or("completed work")? > 0,
                    "{report}"
                );
                assert!(facts["completed_units"].as_u64() <= facts["assigned_units"].as_u64());
                if report["verdict"] == "success" {
                    assert_eq!(facts["assigned_units"], facts["completed_units"]);
                }
                assert_eq!(facts["failures"], json!([]));
                if relative.ends_with("Counter.tla") {
                    assert!(
                        facts["workers"]
                            .as_array()
                            .ok_or("worker facts")?
                            .iter()
                            .all(|worker| worker["result_units"]
                                .as_u64()
                                .is_some_and(|units| units > 0)),
                        "both actual processes must return evaluated work: {facts}"
                    );
                }
                validate(&file, profile, &report, &scratch.0)?;
            }
        }
    }
    Ok(())
}

#[test]
fn worker_options_do_not_silently_change_a_local_request() -> TestResult {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    for flag in [
        "--worker-timeout-ms=10",
        "--distributed-retries=0",
        "--worker-key=absent",
    ] {
        let result = command(&file, "dive", "fp", "check").arg(flag).output()?;
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(result.stdout, [] as [u8; 0]);
        assert!(String::from_utf8_lossy(&result.stderr).contains("require worker mode"));
    }
    Ok(())
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
// Byte-only local test interposer. It has no key and cannot manufacture an
// authenticated result. The later worker really evaluates before truncation.
fn later_result_loss(
    listener: &TcpListener,
    target: SocketAddr,
    stop: &AtomicBool,
    fired: &AtomicBool,
) {
    listener.set_nonblocking(true).expect("proxy listener");
    while !stop.load(Ordering::Relaxed) {
        let (mut incoming, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(_) => return,
        };
        let mut outgoing = TcpStream::connect(target).expect("actual later worker");
        for stream in [&incoming, &outgoing] {
            stream.set_nodelay(true).expect("nodelay");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .expect("write timeout");
        }
        let mut hello = [0; 160];
        if incoming
            .read_exact(&mut hello)
            .and_then(|()| outgoing.write_all(&hello))
            .is_err()
        {
            continue;
        }
        let mut ack = [0; 101];
        if outgoing
            .read_exact(&mut ack)
            .and_then(|()| incoming.write_all(&ack))
            .is_err()
        {
            continue;
        }
        let mut reverse_in = outgoing.try_clone().expect("worker stream");
        let mut reverse_out = incoming.try_clone().expect("coordinator stream");
        std::thread::scope(|scope| {
            scope.spawn(move || {
                loop {
                    let mut header = [0; 17];
                    if reverse_in.read_exact(&mut header).is_err() {
                        break;
                    }
                    let len = usize::try_from(u64::from_le_bytes(
                        header[9..].try_into().expect("length"),
                    ))
                    .expect("native word");
                    if len > 64 << 20 {
                        break;
                    }
                    let mut frame = header.to_vec();
                    frame.resize(17 + len + 32, 0);
                    if reverse_in.read_exact(&mut frame[17..]).is_err() {
                        break;
                    }
                    if header[0] == 6 {
                        fired.store(true, Ordering::Relaxed);
                        let _ = reverse_out.write_all(&frame[..17 + len / 2]);
                        break;
                    }
                    if reverse_out.write_all(&frame).is_err() {
                        break;
                    }
                }
                let _ = reverse_in.shutdown(Shutdown::Both);
                let _ = reverse_out.shutdown(Shutdown::Both);
            });
            let _ = std::io::copy(&mut incoming, &mut outgoing);
            let _ = incoming.shutdown(Shutdown::Both);
            let _ = outgoing.shutdown(Shutdown::Both);
        });
    }
}

#[test]
fn a_later_lost_process_result_cannot_replace_an_earlier_valid_model_error() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = scratch.0.join("Twin.tla");
    std::fs::write(
        &file,
        "---- MODULE Twin ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..1\nNext == x'=x+2\nInv == x<2\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for profile in ["dive", "tlc"] {
        for store in ["fp", "exact"] {
            let local = command(&file, profile, store, "check").output()?;
            let expected: Value = serde_json::from_slice(&local.stdout)?;
            assert_eq!(expected["verdict"], "invariant-violation");
            let a = worker(&file, profile, store, &key)?;
            let b = worker(&file, profile, store, &key)?;
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?.to_string();
            let target = b.address.parse()?;
            let stop = Arc::new(AtomicBool::new(false));
            let fired = AtomicBool::new(false);
            std::thread::scope(|scope| -> TestResult {
                let _guard = Stop(Arc::clone(&stop));
                let stop = &stop;
                let fired = &fired;
                let listener = &listener;
                scope.spawn(move || later_result_loss(listener, target, stop, fired));
                let output = command(&file, profile, store, "check")
                    .arg("--worker-key")
                    .arg(&key)
                    .args([
                        "--distributed-worker",
                        &a.address,
                        "--distributed-worker",
                        &address,
                        "--distributed-retries=0",
                    ])
                    .output()?;
                let report: Value = serde_json::from_slice(&output.stdout)?;
                assert!(
                    fired.load(Ordering::Relaxed),
                    "later evaluated RESULT was actually truncated"
                );
                assert_eq!(observable(&report), observable(&expected));
                assert_eq!(output.status.code(), local.status.code());
                let facts = &report["core"]["metrics"]["distributed"];
                assert_eq!(facts["failed_units"], 1);
                assert_eq!(facts["completed_units"], 1);
                assert_eq!(facts["failures"], json!([]));
                validate(&file, profile, &report, &scratch.0)?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Robustness controls (#953). Every outcome is the local result or a named,
// non-success refusal without statistics or a counterexample. Synchronization
// is explicit: finite worker lifetimes, listeners that never accept, and
// relays that stop forwarding after a fixed number of sessions.

enum Started {
    Ready(Worker),
    Refused(Option<i32>, String),
}
fn spawn_worker(
    exe: &Path,
    file: &Path,
    profile: &str,
    key: &Path,
    extra: &[&str],
) -> Result<Started, Box<dyn std::error::Error>> {
    let mut child = Command::new(exe)
        .arg("worker")
        .arg(file)
        .args(["--seed=0", "--fp=0", "--output=json", "--store=fp"])
        .arg(format!("--compat={profile}"))
        .arg("--worker-key")
        .arg(key)
        .args(extra)
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CORE_OPT")
        .env_remove("DIVE_CHAOS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("worker stdout")?;
    let mut stderr = child.stderr.take().ok_or("worker stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let line = rx.recv_timeout(Duration::from_secs(30))??;
    if line.is_empty() {
        let mut message = String::new();
        stderr.read_to_string(&mut message)?;
        let status = child.wait()?;
        return Ok(Started::Refused(status.code(), message));
    }
    let ready: Value = serde_json::from_str(&line)?;
    assert_eq!(ready["status"], "ready", "{ready}");
    assert_eq!(ready["model_proved"], false);
    Ok(Started::Ready(Worker {
        child,
        address: ready["address"].as_str().ok_or("worker address")?.into(),
    }))
}
fn ready(
    file: &Path,
    profile: &str,
    key: &Path,
    extra: &[&str],
) -> Result<Worker, Box<dyn std::error::Error>> {
    match spawn_worker(
        Path::new(env!("CARGO_BIN_EXE_dive")),
        file,
        profile,
        key,
        extra,
    )? {
        Started::Ready(worker) => Ok(worker),
        Started::Refused(code, message) => {
            Err(format!("worker refused ({code:?}): {message}").into())
        }
    }
}
struct Run {
    code: Option<i32>,
    report: Value,
    stderr: String,
}
fn coordinate(
    file: &Path,
    profile: &str,
    key: &Path,
    endpoints: &[&str],
    extra: &[&str],
) -> Result<Run, Box<dyn std::error::Error>> {
    let mut run = command(file, profile, "fp", "check");
    run.arg("--worker-key").arg(key);
    for endpoint in endpoints {
        run.args(["--distributed-worker", endpoint]);
    }
    let output = run.args(extra).output()?;
    let report = if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout)?
    };
    Ok(Run {
        code: output.status.code(),
        report,
        stderr: String::from_utf8(output.stderr)?,
    })
}
// A failed distributed run never carries a verdict or a word. Statistics, when
// present, are the committed progress of a stopped run, as for a local budget.
fn refused(run: &Run, needle: &str) {
    assert_eq!(run.code, Some(3), "{} {}", run.report, run.stderr);
    assert_eq!(run.report["verdict"], "resource-limit", "{}", run.report);
    assert!(run.report["tlc"].is_null(), "{}", run.report);
    assert!(run.report["property"].is_null(), "{}", run.report);
    assert!(run.report["counterexample"].is_null(), "{}", run.report);
    let message = run.report["message"].as_str().unwrap_or_default();
    assert!(message.contains(needle), "expected `{needle}` in {message}");
}
fn usage(run: &Run, needle: &str) {
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.report.is_null());
    assert!(
        run.stderr.contains(needle),
        "expected `{needle}` in {}",
        run.stderr
    );
}
fn wide(scratch: &Path) -> std::io::Result<PathBuf> {
    let file = scratch.join("Wide.tla");
    std::fs::write(
        &file,
        "---- MODULE Wide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..31\nNext == x' = IF x < 32 THEN x + 32 ELSE x\nInv == x < 64\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    Ok(file)
}

// Relays the first `forward` sessions to `target`; later sessions are accepted
// and held without a byte, so the coordinator's deadline is the only way out.
fn relay(listener: &TcpListener, target: &str, forward: usize, stop: &AtomicBool) {
    listener.set_nonblocking(true).expect("relay listener");
    let mut held = Vec::new();
    let mut sessions = 0;
    std::thread::scope(|scope| {
        while !stop.load(Ordering::Relaxed) {
            let incoming = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(_) => return,
            };
            incoming
                .set_nonblocking(false)
                .expect("blocking relay stream");
            sessions += 1;
            if sessions > forward {
                held.push(incoming);
                continue;
            }
            let outgoing = TcpStream::connect(target).expect("relay target");
            let (mut a, mut b) = (
                incoming.try_clone().expect("clone"),
                outgoing.try_clone().expect("clone"),
            );
            scope.spawn(move || {
                let _ = std::io::copy(&mut a, &mut b);
                let _ = b.shutdown(Shutdown::Both);
            });
            let (mut a, mut b) = (outgoing, incoming);
            scope.spawn(move || {
                let _ = std::io::copy(&mut a, &mut b);
                let _ = b.shutdown(Shutdown::Both);
            });
        }
        for stream in &held {
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
}

#[test]
fn four_process_workers_match_local_results_deterministically_across_verdicts() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    for (relative, verdict) in [
        ("../../docs/user/examples/Counter.tla", "success"),
        (
            "../dive-api/tests/fixtures/core-m0/CompoundTrace.tla",
            "invariant-violation",
        ),
        ("../dive-api/tests/fixtures/core-m0/Dead.tla", "deadlock"),
        (
            "../dive-api/tests/fixtures/checker/dfid/DfidConstrained.tla",
            "success",
        ),
        (
            "../dive-api/tests/fixtures/checker/dfid/DfidActionConstrained.tla",
            "action-property-violation",
        ),
        (
            "../dive-api/tests/fixtures/checker/errors/specs/ErActConstrDivZero/ErActConstrDivZero.tla",
            "evaluation-error",
        ),
    ] {
        let file = root.join(relative);
        // The two-worker control above covers both stores; here both profiles
        // run for the violation and error models only, keeping the test fast.
        let profiles: &[&str] = if matches!(verdict, "invariant-violation" | "evaluation-error") {
            &["dive", "tlc"]
        } else {
            &["dive"]
        };
        for &profile in profiles {
            let local = command(&file, profile, "fp", "check").output()?;
            let expected: Value = serde_json::from_slice(&local.stdout)?;
            assert_eq!(expected["verdict"], verdict, "{relative}: {expected}");
            let workers = (0..4)
                .map(|_| ready(&file, profile, &key, &[]))
                .collect::<Result<Vec<_>, _>>()?;
            let endpoints: Vec<&str> = workers.iter().map(|w| w.address.as_str()).collect();
            let first = coordinate(&file, profile, &key, &endpoints, &[])?;
            let second = coordinate(&file, profile, &key, &endpoints, &[])?;
            for run in [&first, &second] {
                assert_eq!(
                    observable(&run.report),
                    observable(&expected),
                    "{relative}, {profile}: {}",
                    run.stderr
                );
                assert_eq!(run.code, local.status.code());
                let facts = &run.report["core"]["metrics"]["distributed"];
                assert_eq!(facts["configured_workers"], 4, "{facts}");
                assert_eq!(facts["failures"], json!([]));
                assert_eq!(facts["retries"], 0);
            }
            validate(&file, profile, &first.report, &scratch.0)?;
        }
    }
    Ok(())
}

#[test]
fn a_worker_that_exits_mid_run_is_retried_elsewhere_or_the_run_fails_by_name() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = wide(&scratch.0)?;
    let local = command(&file, "dive", "fp", "check").output()?;
    let expected: Value = serde_json::from_slice(&local.stdout)?;
    assert_eq!(expected["verdict"], "success");
    // The finite worker serves its startup ping and three units, then exits.
    let a = ready(&file, "dive", &key, &["--worker-max-units=3"])?;
    let b = ready(&file, "dive", &key, &[])?;
    let run = coordinate(&file, "dive", &key, &[&a.address, &b.address], &[])?;
    assert_eq!(
        observable(&run.report),
        observable(&expected),
        "{}",
        run.stderr
    );
    assert_eq!(run.code, Some(0));
    let facts = &run.report["core"]["metrics"]["distributed"];
    assert!(facts["retries"].as_u64().ok_or("retries")? > 0, "{facts}");
    assert_eq!(facts["failures"], json!([]));
    assert_eq!(facts["completed_units"], facts["assigned_units"]);

    let a = ready(&file, "dive", &key, &["--worker-max-units=3"])?;
    let b = ready(&file, "dive", &key, &[])?;
    let run = coordinate(
        &file,
        "dive",
        &key,
        &[&a.address, &b.address],
        &["--distributed-retries=0"],
    )?;
    refused(&run, "DIVE-S0502: distributed I/O");
    assert_eq!(run.report["core"]["metrics"]["distributed"]["retries"], 0);

    // Every endpoint gone: the bounded retries are exhausted.
    let a = ready(&file, "dive", &key, &["--worker-max-units=2"])?;
    let b = ready(&file, "dive", &key, &["--worker-max-units=2"])?;
    let run = coordinate(
        &file,
        "dive",
        &key,
        &[&a.address, &b.address],
        &["--distributed-retries=3"],
    )?;
    refused(&run, "DIVE-S0502: distributed I/O");
    assert!(
        run.report["core"]["metrics"]["distributed"]["retries"]
            .as_u64()
            .ok_or("retries")?
            >= 3
    );
    Ok(())
}

#[test]
fn endpoints_that_never_answer_or_stall_hit_the_deadline_by_name() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let local = command(&file, "dive", "fp", "check").output()?;
    let expected: Value = serde_json::from_slice(&local.stdout)?;
    let b = ready(&file, "dive", &key, &[])?;

    // No listener at all.
    let closed = TcpListener::bind("127.0.0.1:0")?.local_addr()?.to_string();
    let run = coordinate(&file, "dive", &key, &[&closed, &b.address], &[])?;
    refused(&run, "DIVE-S0502: distributed I/O");

    // A listener that never accepts: TCP connects, nothing ever answers.
    let silent = TcpListener::bind("127.0.0.1:0")?;
    let silent_address = silent.local_addr()?.to_string();
    let run = coordinate(
        &file,
        "dive",
        &key,
        &[&silent_address, &b.address],
        &["--worker-timeout-ms=2000"],
    )?;
    refused(&run, "did not answer the authenticated handshake");
    refused(&run, "no reply within the --worker-timeout-ms deadline");
    drop(silent);

    // A worker reachable only through a relay that forwards the startup
    // session and then stalls every unit session.
    let a = ready(&file, "dive", &key, &[])?;
    // The relay's held sessions only end at the coordinator's deadline. It is
    // generous so that the healthy worker b, bound by the same deadline, is
    // never mistaken for a stalled one on a loaded host.
    for (retries, recovered) in [
        ("--distributed-retries=2", true),
        ("--distributed-retries=0", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let relay_address = listener.local_addr()?.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        std::thread::scope(|scope| -> TestResult {
            let _guard = Stop(Arc::clone(&stop));
            let (listener, target, stop) = (&listener, a.address.as_str(), &stop);
            scope.spawn(move || relay(listener, target, 1, stop));
            let run = coordinate(
                &file,
                "dive",
                &key,
                &[&relay_address, &b.address],
                &["--worker-timeout-ms=5000", retries],
            )?;
            if recovered {
                assert_eq!(
                    observable(&run.report),
                    observable(&expected),
                    "{}",
                    run.stderr
                );
                assert_eq!(run.code, Some(0));
                let facts = &run.report["core"]["metrics"]["distributed"];
                assert!(facts["retries"].as_u64().ok_or("retries")? > 0, "{facts}");
            } else {
                refused(&run, "no reply within the --worker-timeout-ms deadline");
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn a_unit_longer_than_the_worker_deadline_is_reported_by_the_worker() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = scratch.0.join("SlowUnit.tla");
    std::fs::write(
        &file,
        "---- MODULE SlowUnit ----\nEXTENDS Naturals, FiniteSets\nVARIABLE x\nInit == x = 0\nNext == x < 2 /\\ x' = x + 1 + 0 * Cardinality({s \\in SUBSET (1..24) : Cardinality(s) = 12})\nInv == x < 9\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    // The unit filters 2^24 subsets, far beyond any deadline here (the
    // evaluator polls the deadline while enumerating);
    // the worker's 3 s deadline leaves its handshake ample time on a loaded
    // host, and the coordinator's own deadline is far longer.
    let a = ready(&file, "dive", &key, &["--worker-timeout-ms=3000"])?;
    let b = ready(&file, "dive", &key, &["--worker-timeout-ms=3000"])?;
    let run = coordinate(
        &file,
        "dive",
        &key,
        &[&a.address, &b.address],
        &["--worker-timeout-ms=60000"],
    )?;
    refused(
        &run,
        "worker refused incomplete unit: the unit did not finish within the worker's --worker-timeout-ms of 3000 ms",
    );
    Ok(())
}

#[test]
fn keys_must_be_present_exact_and_shared() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let other = scratch.0.join("other-key");
    std::fs::write(&other, [7; 32])?;
    let short = scratch.0.join("short-key");
    std::fs::write(&short, [7; 31])?;
    let absent = scratch.0.join("absent-key");
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let a = ready(&file, "dive", &key, &[])?;
    let b = ready(&file, "dive", &key, &[])?;
    let endpoints = [a.address.as_str(), b.address.as_str()];
    let run = coordinate(
        &file,
        "dive",
        &other,
        &endpoints,
        &["--worker-timeout-ms=2000"],
    )?;
    refused(
        &run,
        "check that every role reads the same --worker-key file",
    );
    let run = coordinate(&file, "dive", &short, &endpoints, &[])?;
    refused(&run, "must contain exactly 32 bytes");
    let run = coordinate(&file, "dive", &absent, &endpoints, &[])?;
    refused(&run, "absent-key");
    let output = command(&file, "dive", "fp", "check")
        .args([
            "--distributed-worker",
            endpoints[0],
            "--distributed-worker",
            endpoints[1],
        ])
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --worker-key FILE"));

    let c = ready(&file, "dive", &other, &[])?;
    let run = coordinate(&file, "dive", &key, &[&a.address, &c.address], &[])?;
    refused(
        &run,
        "check that every role reads the same --worker-key file",
    );
    let exe = Path::new(env!("CARGO_BIN_EXE_dive"));
    for (bad, needle) in [
        (&short, "must contain exactly 32 bytes"),
        (&absent, "absent-key"),
    ] {
        match spawn_worker(exe, &file, "dive", bad, &[])? {
            Started::Ready(_) => return Err("worker started without a valid key".into()),
            Started::Refused(code, message) => {
                assert_eq!(code, Some(3), "{message}");
                assert!(message.contains(needle), "{message}");
            }
        }
    }
    Ok(())
}

#[test]
fn a_worker_from_another_build_refuses_by_identity() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let other = scratch.0.join("dive-other-build");
    std::fs::copy(env!("CARGO_BIN_EXE_dive"), &other)?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&other)?
        .write_all(b"different build")?;
    let a = match spawn_worker(&other, &file, "dive", &key, &[])? {
        Started::Ready(worker) => worker,
        Started::Refused(code, message) => {
            return Err(format!("other build refused ({code:?}): {message}").into());
        }
    };
    let b = ready(&file, "dive", &key, &[])?;
    let run = coordinate(&file, "dive", &key, &[&a.address, &b.address], &[])?;
    refused(&run, "identity mismatch");
    Ok(())
}

#[test]
fn endpoint_and_listener_policy_refuse_before_loading_the_model() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let missing = scratch.0.join("Missing.tla");
    let run = coordinate(&missing, "dive", &key, &["127.0.0.1:1"], &[])?;
    usage(&run, "requires 2..64 --distributed-worker endpoints");
    let run = coordinate(&missing, "dive", &key, &["127.0.0.1:1", "127.0.0.1:1"], &[])?;
    usage(&run, "endpoints must be distinct");
    let many: Vec<String> = (1..=65).map(|port| format!("127.0.0.1:{port}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let run = coordinate(&missing, "dive", &key, &many, &[])?;
    usage(&run, "requires 2..64 --distributed-worker endpoints");
    for (flag, needle) in [
        ("--worker-timeout-ms=0", "1..60000"),
        ("--worker-timeout-ms=60001", "1..60000"),
        ("--distributed-retries=9", "0..8"),
    ] {
        let run = coordinate(
            &missing,
            "dive",
            &key,
            &["127.0.0.1:1", "127.0.0.1:2"],
            &[flag],
        )?;
        usage(&run, needle);
    }
    assert!(!missing.exists());

    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    let busy = TcpListener::bind("127.0.0.1:0")?;
    let busy_address = busy.local_addr()?.to_string();
    let exe = Path::new(env!("CARGO_BIN_EXE_dive"));
    match spawn_worker(
        exe,
        &file,
        "dive",
        &key,
        &[&format!("--listen={busy_address}")],
    )? {
        Started::Ready(_) => return Err("worker listened on a port in use".into()),
        Started::Refused(code, message) => {
            assert_eq!(code, Some(2), "{message}");
            assert!(message.contains("worker listener"), "{message}");
        }
    }

    // Two endpoints that reach one worker process are one worker.
    let a = ready(&file, "dive", &key, &[])?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let alias = listener.local_addr()?.to_string();
    let stop = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| -> TestResult {
        let _guard = Stop(Arc::clone(&stop));
        let (listener, target, stop) = (&listener, a.address.as_str(), &stop);
        scope.spawn(move || relay(listener, target, usize::MAX, stop));
        let run = coordinate(&file, "dive", &key, &[&a.address, &alias], &[])?;
        refused(&run, "two independently initialized workers required");
        Ok(())
    })?;
    Ok(())
}

#[test]
fn unsupported_models_and_options_refuse_by_name_on_both_roles() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let model = |name: &str, extra: &str, cfg: &str| -> std::io::Result<PathBuf> {
        let file = scratch.0.join(format!("{name}.tla"));
        std::fs::write(
            &file,
            format!(
                "---- MODULE {name} ----\nEXTENDS Naturals, TLC\nCONSTANT P\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 3\n{extra}\n====\n"
            ),
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            format!("CONSTANT P = {{p1, p2}}\nINIT Init\nNEXT Next\n{cfg}\n"),
        )?;
        Ok(file)
    };
    let cases = [
        (
            model("Sym", "Perms == Permutations(P)", "SYMMETRY Perms")?,
            "with SYMMETRY is not implemented",
        ),
        (
            model(
                "Live",
                "Spec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == <>(x = 2)",
                "PROPERTY Live",
            )?,
            "temporal properties",
        ),
        (
            model("Eff", "Inv == PrintT(x)", "INVARIANT Inv")?,
            "with runtime effects",
        ),
        (
            model("Vw", "V == x % 2", "VIEW V")?,
            "with VIEW is not implemented",
        ),
    ];
    let exe = Path::new(env!("CARGO_BIN_EXE_dive"));
    for (file, needle) in &cases {
        // Unreachable endpoints prove the refusal precedes any connection.
        let run = coordinate(file, "dive", &key, &["127.0.0.1:1", "127.0.0.1:2"], &[])?;
        assert_eq!(run.code, Some(3), "{} {}", run.report, run.stderr);
        assert_eq!(run.report["verdict"], "unsupported", "{}", run.report);
        let message = run.report["message"].as_str().unwrap_or_default();
        assert!(message.contains(needle), "{message}");
        assert!(run.report["statistics"].is_null());
        match spawn_worker(exe, file, "dive", &key, &[])? {
            Started::Ready(_) => return Err(format!("worker admitted {}", file.display()).into()),
            Started::Refused(code, message) => {
                assert_eq!(code, Some(3), "{message}");
                assert!(message.contains("worker preparation"), "{message}");
                assert!(message.contains(needle), "{message}");
                assert!(!message.contains("Unsupported {"), "{message}");
            }
        }
    }
    // A worker whose own start-up ends in a model verdict names the verdict
    // and its message, not a Debug dump labelled unsupported.
    for (file, kind, needle) in [
        (
            model("AssFalse", "ASSUME FALSE", "")?,
            "assumption-failure",
            "Assumption",
        ),
        (
            model("AssDiv", "ASSUME 1 \\div 0 = 0", "")?,
            "evaluation-error",
            "The second argument of \\div is 0",
        ),
    ] {
        match spawn_worker(exe, &file, "dive", &key, &[])? {
            Started::Ready(_) => return Err(format!("worker admitted {}", file.display()).into()),
            Started::Refused(code, message) => {
                assert_eq!(code, Some(3), "{message}");
                assert!(
                    message.contains(&format!("worker preparation stopped with {kind}")),
                    "{message}"
                );
                assert!(message.contains(needle), "{message}");
                assert!(!message.contains("unsupported"), "{message}");
                assert!(!message.contains(" {"), "{message}");
            }
        }
    }
    Ok(())
}

#[test]
fn unsupported_coordinator_options_refuse_by_name() -> TestResult {
    let scratch = Scratch::new()?;
    let key = scratch.0.join("key");
    let counter =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/user/examples/Counter.tla");
    for (flags, needle) in [
        (
            &["--sequential"][..],
            "with --sequential is not implemented",
        ),
        (
            &[
                "--workers=1",
                "--checkpoint",
                "x.ckpt",
                "--checkpoint-after-levels=1",
            ][..],
            "checkpoint",
        ),
    ] {
        let run = coordinate(
            &counter,
            "dive",
            &key,
            &["127.0.0.1:1", "127.0.0.1:2"],
            flags,
        )?;
        assert_ne!(run.code, Some(0), "{}", run.stderr);
        assert!(
            run.stderr.contains(needle) || run.report.to_string().contains(needle),
            "{needle}: {} {}",
            run.report,
            run.stderr
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg("check")
        .arg(&counter)
        .args(["--output=json", "--worker-key"])
        .arg(&key)
        .args([
            "--distributed-worker=127.0.0.1:1",
            "--distributed-worker=127.0.0.1:2",
        ])
        .env_remove("DIVE_COMPAT")
        .output()?;
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("without an explicit --seed"));
    Ok(())
}
