//! Separate, opt-in setup receipts. A cached artifact is never a native exit.

use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::process::{ProcessOutcome, run_with_timeout};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::raw::{Scratch, physical_destination, write_manifest};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(120);

/// Private to the one opt-in preparation path; errors latch before another spawn.
pub(crate) struct Capture {
    root: PathBuf,
    workspace: PathBuf,
    failed: Cell<bool>,
    records: RefCell<Vec<Value>>,
}

struct Invocation {
    scratch: Scratch,
    identity: Value,
}

impl Capture {
    pub(crate) fn new(raw_root: &Path, corpus_root: &Path) -> Result<Self> {
        let destination = physical_destination(raw_root)?;
        // Setup must not add files to the source digest or create a descendant
        // that later recursive model isolation copies into itself. Follow the
        // same physical directory aliases as isolation, before any mkdir.
        let mut pending = vec![corpus_root.to_path_buf()];
        let mut visited = HashSet::new();
        while let Some(directory) = pending.pop() {
            let physical =
                std::fs::canonicalize(&directory).map_err(|error| Error::io(&directory, &error))?;
            if destination.starts_with(&physical) {
                return Err(Error::Config(format!(
                    "raw setup directory {} overlaps corpus directory {}",
                    destination.display(),
                    physical.display()
                )));
            }
            if !visited.insert(physical) {
                continue;
            }
            for entry in
                std::fs::read_dir(&directory).map_err(|error| Error::io(&directory, &error))?
            {
                let entry = entry.map_err(|error| Error::io(&directory, &error))?;
                if entry.file_name() == ".git" {
                    continue;
                }
                let path = entry.path();
                let metadata = match std::fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    // A dangling file alias is not a copied directory. It may
                    // still fail later model isolation; this guard only proves
                    // that capture cannot become a recursive copy destination.
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound
                            && entry
                                .file_type()
                                .map_err(|error| Error::io(&path, &error))?
                                .is_symlink() =>
                    {
                        continue;
                    }
                    Err(error) => return Err(Error::io(&path, &error)),
                };
                if metadata.is_dir() {
                    pending.push(path);
                }
            }
        }
        let namespace = destination.join("setup-status");
        let session = Scratch::new(&namespace, Some(&namespace))?;
        let root = session.path().to_path_buf();
        for name in ["invocations", "workspace", "cache", "verifications"] {
            let path = root.join(name);
            std::fs::create_dir(&path).map_err(|error| Error::io(&path, &error))?;
        }
        write_manifest(
            &root.join("session-started.json"),
            &json!({
                "schema": 1, "kind": "oracle-setup-session", "state": "started",
                "normal_native_completion_required": true,
                "cached_artifacts_are_not_native_results": true
            }),
        )?;
        let workspace = root.join("workspace");
        Ok(Self {
            root,
            workspace,
            failed: Cell::new(false),
            records: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn workspace(&self) -> &Path {
        // Recorded argv/cwd point at this retained workspace, not a dropped
        // capability tempfile. No additional setup or model is executed.
        &self.workspace
    }

    pub(crate) fn cache_hit(&self, phase: &str, identity: &str) -> Result<()> {
        self.not_executed(phase, identity, true)
    }

    pub(crate) fn in_process_verification(&self, phase: &str, identity: &str) -> Result<()> {
        self.not_executed(phase, identity, false)
    }

    fn not_executed(&self, phase: &str, identity: &str, cache_hit: bool) -> Result<()> {
        self.require_ready()?;
        let record = json!({
            "schema": 1,
            "kind": if cache_hit { "oracle-setup-cache" } else { "oracle-setup-verification" },
            "state": "not-executed", "phase": phase, "identity": identity,
            "cache_hit": cache_hit, "native_invocation": false,
            "verification": "in-process",
            "artifact_source": if cache_hit { "cache" } else { "fresh-in-process" }
        });
        let path = self
            .root
            .join(if cache_hit { "cache" } else { "verifications" })
            .join(format!("{}.json", self.records.borrow().len()));
        if let Err(error) = write_manifest(&path, &record) {
            self.failed.set(true);
            return Err(error);
        }
        self.records.borrow_mut().push(record);
        Ok(())
    }

    fn require_ready(&self) -> Result<()> {
        if self.failed.get() {
            return Err(Error::Command(
                "raw setup capture already failed; no later native invocation".into(),
            ));
        }
        Ok(())
    }

    fn start(&self, phase: &str, command: &mut Command, timeout: Duration) -> Result<Invocation> {
        self.require_ready()?;
        let text = |value: &std::ffi::OsStr| {
            value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::Config("raw setup arguments must be UTF-8".into()))
        };
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(text)
            .collect::<Result<Vec<_>>>()?;
        let cwd = match command.get_current_dir() {
            Some(path) => path.to_path_buf(),
            None => {
                std::env::current_dir().map_err(|error| Error::Io(format!("setup cwd: {error}")))?
            }
        };
        let cwd_text = text(cwd.as_os_str())?;
        command.current_dir(&cwd);
        let directory = self.root.join("invocations");
        let scratch = Scratch::new(&directory, Some(&directory))?;
        let identity = json!({
            "schema": 1, "kind": "oracle-setup", "phase": phase,
            "argv": argv, "cwd": cwd_text,
            "timeout": {"seconds": timeout.as_secs(), "nanoseconds": timeout.subsec_nanos()},
            "stdout": "stdout.txt", "stderr": "stderr.txt"
        });
        let mut started = identity.clone();
        started["state"] = json!("started");
        write_manifest(&scratch.path().join("started.json"), &started)?;
        Ok(Invocation { scratch, identity })
    }

    fn finish(
        &self,
        invocation: Invocation,
        outcome: Result<ProcessOutcome>,
    ) -> Result<ProcessOutcome> {
        let result = self.finish_record(invocation, outcome);
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }

    fn finish_record(
        &self,
        invocation: Invocation,
        outcome: Result<ProcessOutcome>,
    ) -> Result<ProcessOutcome> {
        let path = invocation.scratch.path();
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.failed.set(true);
                let mut record = invocation.identity;
                record["state"] = json!("harness-error");
                record["error"] = json!(error.to_string());
                // Partial original files remain owned by the retained path.
                // Unknown ownership is not converted into immutable output hashes.
                write_manifest(&path.join("harness-error.json"), &record)?;
                return Err(error);
            }
        };
        let normal = outcome.exit_code.is_some_and(|code| code >= 0)
            && outcome.signal.is_none()
            && !outcome.timed_out;
        let mut record = invocation.identity;
        record["state"] = json!("completed");
        record["exit_code"] = json!(outcome.exit_code);
        record["signal"] = json!(outcome.signal);
        record["timed_out"] = json!(outcome.timed_out);
        record["elapsed"] = json!({"seconds": outcome.elapsed.as_secs(), "nanoseconds": outcome.elapsed.subsec_nanos()});
        for name in ["stdout", "stderr"] {
            let file = path.join(format!("{name}.txt"));
            let metadata = std::fs::metadata(&file).map_err(|error| Error::io(&file, &error))?;
            record[format!("{name}_sha256")] = json!(sha256_file(&file)?);
            record[format!("{name}_bytes")] = json!(metadata.len());
        }
        write_manifest(&path.join("completed.json"), &record)?;
        self.records.borrow_mut().push(json!({
            "kind": "oracle-setup", "path": path.strip_prefix(&self.root)
                .map_err(|error| Error::Config(format!("setup relative path: {error}")))?,
            "phase": record["phase"], "normal_completion": normal
        }));
        if !normal {
            self.failed.set(true);
            return Err(Error::Command("raw setup invocation timed out, was signalled or lacks a normal exit; no later native invocation".into()));
        }
        Ok(outcome)
    }

    pub(crate) fn run(
        &self,
        phase: &str,
        command: &mut Command,
        timeout: Duration,
    ) -> Result<ProcessOutcome> {
        let result = (|| {
            let invocation = self.start(phase, command, timeout)?;
            let outcome = run_with_timeout(command, timeout, invocation.scratch.path());
            self.finish(invocation, outcome)
        })();
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }

    pub(crate) fn prepared(&self) -> Result<()> {
        self.summary("preparation-completed.json", "prepared")
    }

    pub(crate) fn complete(&self) -> Result<()> {
        self.summary("session-completed.json", "completed")
    }

    fn summary(&self, name: &str, state: &str) -> Result<()> {
        self.require_ready()?;
        let result = write_manifest(
            &self.root.join(name),
            &json!({
                "schema": 1, "kind": "oracle-setup-session", "state": state,
                "records": &*self.records.borrow(),
                "native_results_are_normal_not_necessarily_zero": true
            }),
        );
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    fn fixture() -> Result<(TempDir, Capture)> {
        let parent = TempDir::new("setup-capture-test")?;
        let corpus = parent.path().join("corpus");
        std::fs::create_dir(&corpus).map_err(|error| Error::io(&corpus, &error))?;
        let capture = Capture::new(&parent.path().join("raw"), &corpus)?;
        Ok((parent, capture))
    }

    fn outcome(exit_code: Option<i32>, signal: Option<i32>, timed_out: bool) -> ProcessOutcome {
        ProcessOutcome {
            exit_code,
            signal,
            timed_out,
            stdout: String::new(),
            stderr: String::new(),
            elapsed: Duration::from_millis(2),
        }
    }

    fn pending(capture: &Capture) -> Result<Invocation> {
        let invocation = capture.start(
            "synthetic-status",
            &mut Command::new("not-dispatched"),
            TIMEOUT,
        )?;
        for (name, bytes) in [
            ("stdout.txt", &[0xff, b'\n'][..]),
            ("stderr.txt", b"partial".as_slice()),
        ] {
            let path = invocation.scratch.path().join(name);
            std::fs::write(&path, bytes).map_err(|error| Error::io(&path, &error))?;
        }
        Ok(invocation)
    }

    #[test]
    fn normal_nonzero_is_recorded_without_claiming_semantic_success() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let invocation = pending(&capture)?;
        let path = invocation.scratch.path().to_path_buf();
        assert!(path.join("started.json").is_file());
        let output = capture.finish(invocation, Ok(outcome(Some(13), None, false)))?;
        assert_eq!(output.exit_code, Some(13));
        let bytes =
            std::fs::read(path.join("completed.json")).map_err(|error| Error::io(&path, &error))?;
        let record: Value =
            serde_json::from_slice(&bytes).map_err(|error| Error::Config(error.to_string()))?;
        assert_eq!(record["exit_code"], 13);
        assert_eq!(record["stdout_bytes"], 2);
        assert_eq!(
            record["stdout_sha256"],
            sha256_file(&path.join("stdout.txt"))?
        );
        assert_eq!(record["timed_out"], false);
        capture.prepared()?;
        assert!(capture.root.join("preparation-completed.json").is_file());
        capture.complete()?;
        assert!(capture.root.join("session-completed.json").is_file());
        Ok(())
    }

    #[test]
    fn censored_or_malformed_status_blocks_later_dispatch() -> Result<()> {
        for output in [
            outcome(Some(0), None, true),
            outcome(None, Some(15), false),
            outcome(None, None, false),
            outcome(Some(-1), None, false),
        ] {
            let (_parent, capture) = fixture()?;
            let invocation = pending(&capture)?;
            let path = invocation.scratch.path().to_path_buf();
            assert!(capture.finish(invocation, Ok(output)).is_err());
            assert!(
                capture
                    .start(
                        "must-not-dispatch",
                        &mut Command::new("not-dispatched"),
                        TIMEOUT
                    )
                    .is_err()
            );
            assert!(capture.complete().is_err());
            assert!(path.join("completed.json").is_file());
            assert_eq!(
                std::fs::read(path.join("stdout.txt")).map_err(|error| Error::io(&path, &error))?,
                [0xff, b'\n']
            );
        }
        Ok(())
    }

    #[test]
    fn dispatch_error_and_drop_retain_partial_outputs() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let invocation = pending(&capture)?;
        let path = invocation.scratch.path().to_path_buf();
        // Synthetic adapter error exercises recording without poisoning the
        // process-global owned supervisor with a real failed launch.
        assert!(
            capture
                .finish(
                    invocation,
                    Err(Error::Command("synthetic dispatch failure".into()))
                )
                .is_err()
        );
        assert!(capture.require_ready().is_err());
        drop(capture);
        assert!(path.join("started.json").is_file());
        assert!(path.join("harness-error.json").is_file());
        assert!(!path.join("completed.json").exists());
        assert_eq!(
            std::fs::read(path.join("stderr.txt")).map_err(|error| Error::io(&path, &error))?,
            b"partial"
        );
        Ok(())
    }

    #[test]
    fn cache_record_is_explicitly_not_a_native_completion() -> Result<()> {
        let (_parent, capture) = fixture()?;
        capture.cache_hit("verified-archive", "sha256:fixture")?;
        let path = capture.root.join("cache/0.json");
        let bytes = std::fs::read(&path).map_err(|error| Error::io(&path, &error))?;
        let record: Value =
            serde_json::from_slice(&bytes).map_err(|error| Error::Config(error.to_string()))?;
        assert_eq!(record["state"], "not-executed");
        assert_eq!(record["native_invocation"], false);
        assert!(record.get("exit_code").is_none());
        assert_eq!(
            std::fs::read_dir(capture.root.join("invocations"))
                .map_err(|error| Error::io(&capture.root, &error))?
                .count(),
            0
        );
        Ok(())
    }

    #[test]
    fn source_overlap_is_rejected_before_creating_raw_directory() -> Result<()> {
        let parent = TempDir::new("setup-overlap")?;
        let corpus = parent.path().join("corpus");
        std::fs::create_dir(&corpus).map_err(|error| Error::io(&corpus, &error))?;
        let raw = corpus.join("absent/raw");
        assert!(Capture::new(&raw, &corpus).is_err());
        assert!(!raw.exists());
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn actual_owned_timeout_preserves_partial_and_latches_capture() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let marker = capture.workspace().join("must-not-run");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf partial; exec sleep 10"]);
        assert!(
            capture
                .run("synthetic-timeout", &mut command, Duration::from_secs(1))
                .is_err()
        );
        let mut later = Command::new("/usr/bin/touch");
        later.arg(&marker);
        assert!(
            capture
                .run("must-not-dispatch", &mut later, TIMEOUT)
                .is_err()
        );
        assert!(!marker.exists());
        let records = capture.records.borrow();
        assert_eq!(records.len(), 1);
        let relative = records[0]["path"]
            .as_str()
            .ok_or_else(|| Error::Config("missing invocation path".into()))?;
        let path = capture.root.join(relative);
        assert_eq!(
            std::fs::read(path.join("stdout.txt")).map_err(|error| Error::io(&path, &error))?,
            b"partial"
        );
        assert!(path.join("completed.json").is_file());
        Ok(())
    }
    #[test]
    fn terminal_recording_error_also_blocks_later_dispatch() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let invocation = capture.start(
            "missing-stream",
            &mut Command::new("not-dispatched"),
            TIMEOUT,
        )?;
        let path = invocation.scratch.path().to_path_buf();
        assert!(
            capture
                .finish(invocation, Ok(outcome(Some(0), None, false)))
                .is_err()
        );
        assert!(capture.require_ready().is_err());
        assert!(!path.join("completed.json").exists());
        Ok(())
    }

    #[test]
    fn abandoned_started_setup_is_retained_without_terminal_claim() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let invocation = pending(&capture)?;
        let path = invocation.scratch.path().to_path_buf();
        drop(invocation);
        drop(capture);
        assert!(path.join("started.json").is_file());
        assert!(!path.join("completed.json").exists());
        assert!(!path.join("harness-error.json").exists());
        assert_eq!(
            std::fs::read(path.join("stderr.txt")).map_err(|error| Error::io(&path, &error))?,
            b"partial"
        );
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn actual_normal_nonzero_retains_original_bytes_and_requested_command() -> Result<()> {
        let (_parent, capture) = fixture()?;
        let mut command = Command::new("/bin/sh");
        command
            .current_dir(capture.workspace())
            .args(["-c", "printf normal; printf diagnostic >&2; exit 13"]);
        let output = capture.run("synthetic-normal-nonzero", &mut command, TIMEOUT)?;
        assert_eq!(output.exit_code, Some(13));
        assert_eq!(output.stdout, "normal");
        assert_eq!(output.stderr, "diagnostic");
        capture.complete()?;
        assert_eq!(capture.records.borrow().len(), 1);
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn physical_source_aliases_are_checked_before_capture_creation() -> Result<()> {
        let parent = TempDir::new("setup-alias-overlap")?;
        let corpus = parent.path().join("corpus");
        let external = parent.path().join("external");
        std::fs::create_dir(&corpus).map_err(|error| Error::io(&corpus, &error))?;
        std::fs::create_dir(&external).map_err(|error| Error::io(&external, &error))?;
        let destination_alias = parent.path().join("corpus-alias");
        std::os::unix::fs::symlink(&corpus, &destination_alias)
            .map_err(|error| Error::io(&destination_alias, &error))?;
        let raw = destination_alias.join("absent/raw");
        assert!(Capture::new(&raw, &corpus).is_err());
        assert!(!raw.exists());
        let source_alias = corpus.join("external-alias");
        std::os::unix::fs::symlink(&external, &source_alias)
            .map_err(|error| Error::io(&source_alias, &error))?;
        let raw = external.join("absent/raw");
        assert!(Capture::new(&raw, &corpus).is_err());
        assert!(!raw.exists());
        let dangling = corpus.join("dangling-file");
        std::os::unix::fs::symlink("missing-file", &dangling)
            .map_err(|error| Error::io(&dangling, &error))?;
        let capture = Capture::new(&parent.path().join("safe"), &corpus)?;
        assert!(capture.root.join("session-started.json").is_file());
        assert!(!external.join("absent").exists());
        Ok(())
    }
    #[test]
    fn fresh_digest_verification_is_not_a_cache_hit_or_native_result() -> Result<()> {
        let (_parent, capture) = fixture()?;
        capture.in_process_verification("repository-corpus-digest", "sha256:fresh")?;
        let path = capture.root.join("verifications/0.json");
        let bytes = std::fs::read(&path).map_err(|error| Error::io(&path, &error))?;
        let record: Value =
            serde_json::from_slice(&bytes).map_err(|error| Error::Config(error.to_string()))?;
        assert_eq!(record["kind"], "oracle-setup-verification");
        assert_eq!(record["state"], "not-executed");
        assert_eq!(record["cache_hit"], false);
        assert_eq!(record["native_invocation"], false);
        assert_eq!(record["artifact_source"], "fresh-in-process");
        assert_eq!(record["verification"], "in-process");
        assert!(record.get("exit_code").is_none());
        assert_eq!(
            std::fs::read_dir(capture.root.join("cache"))
                .map_err(|error| Error::io(&capture.root, &error))?
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_dir(capture.root.join("invocations"))
                .map_err(|error| Error::io(&capture.root, &error))?
                .count(),
            0
        );
        Ok(())
    }
}
