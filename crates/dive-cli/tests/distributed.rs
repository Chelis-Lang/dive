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
        let result = run()?;
        let doc: Value = serde_json::from_slice(&result.stdout)?;
        assert!(!result.status.success(), "mutated claim: {doc}");
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
