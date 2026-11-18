//! Independent public consumer: native worker processes with an empty PATH.
use super::{CONFIG, FINITE, Project, Result};
use dive_api::{CheckRequest, DistributedOptions, Store, check_file, prepare_core_worker};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Worker {
    process: Child,
    address: SocketAddr,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
fn spawn(
    req: &CheckRequest,
    key: &Path,
) -> std::result::Result<Worker, Box<dyn std::error::Error>> {
    let mut process = Command::new(std::env::current_exe()?)
        .args(["--exact", "distributed::worker_entry", "--nocapture"])
        .env("DIVE_CONSUMER_WORKER_FILE", &req.file)
        .env("DIVE_CONSUMER_WORKER_KEY", key)
        .env(
            "DIVE_CONSUMER_WORKER_TLC",
            if req.compat_tlc { "1" } else { "0" },
        )
        .env_remove("DIVE_CORE_OPT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = process.stdout.take().ok_or("worker stdout")?;
    let mut worker = Worker {
        process,
        address: "127.0.0.1:0".parse()?,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if let Ok(line) = line
                && line.starts_with('{')
            {
                let _ = tx.send(line);
                break;
            }
        }
    });
    let ready: Value = serde_json::from_str(&rx.recv_timeout(Duration::from_secs(15))?)?;
    worker.address = ready["address"].as_str().ok_or("address")?.parse()?;
    assert_eq!(ready["pid"], worker.process.id());
    Ok(worker)
}
#[test]
fn worker_entry() -> Result {
    let Some(file) = std::env::var_os("DIVE_CONSUMER_WORKER_FILE") else {
        return Ok(());
    };
    let key = std::env::var_os("DIVE_CONSUMER_WORKER_KEY").ok_or("key path")?;
    let compat = std::env::var("DIVE_CONSUMER_WORKER_TLC")? == "1";
    let request = CheckRequest {
        file: file.into(),
        seed: Some(7),
        fp: Some(0),
        compat_tlc: compat,
        store: Store::Exact,
        ..CheckRequest::default()
    };
    let worker = prepare_core_worker(&request, Path::new(&key), Duration::from_secs(5))?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    println!(
        "{}",
        serde_json::json!({"address":listener.local_addr()?.to_string(),"pid":std::process::id()})
    );
    std::io::stdout().flush()?;
    worker.serve(&listener, None, &|| false)?;
    Ok(())
}
#[test]
fn two_independent_workers_check_through_public_rust_without_cli_or_java() -> Result {
    let project = Project::new("Finite", FINITE, CONFIG)?;
    let key = project.directory.join("worker.key");
    std::fs::write(&key, [31; 32])?;
    for compat in [false, true] {
        let mut request = project.request(compat);
        request.store = Store::Exact;
        let a = spawn(&request, &key)?;
        let b = spawn(&request, &key)?;
        assert_ne!(a.process.id(), b.process.id());
        request.distributed = Some(DistributedOptions::new(
            vec![a.address, b.address],
            key.clone(),
        ));
        let report = check_file(&request)?;
        let doc = report.to_json();
        assert_eq!(doc["verdict"], "success", "{doc}");
        assert_eq!(doc["statistics"]["distinct"], 3);
        assert_eq!(doc["statistics"]["generated"], 4);
        let work = &doc["core"]["metrics"]["distributed"];
        assert_eq!(work["workers"].as_array().ok_or("workers")?.len(), 2);
        assert_eq!(work["completed_units"], 3);
        assert!(
            work["workers"]
                .as_array()
                .ok_or("workers")?
                .iter()
                .all(|worker| worker["result_units"]
                    .as_u64()
                    .is_some_and(|units| units > 0)),
            "each process must actually return source results"
        );
        assert_eq!(work["failures"], serde_json::json!([]));
    }
    Ok(())
}
