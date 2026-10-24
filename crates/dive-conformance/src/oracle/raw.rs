//! Opt-in original-byte TLC capture, independent of classification and goldens.
//!
//! Retained directories deliberately have no removing drop implementation:
//! started/partial output must survive cancellation or a killed harness.

use super::TlcJob;
use crate::error::{Error, Result};
use crate::process::ProcessOutcome;
use crate::util::{TempDir, write_atomic};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Checks before any mkdir: recursive isolation follows directory symlinks,
/// including links to ancestors of a capture directory outside the spec.
pub(super) fn checked_root(root: &Path, job: &TlcJob<'_>) -> Result<PathBuf> {
    let destination = physical_destination(root)?;
    let (relative, recursive) = crate::corpus::isolation(job.module);
    let mut pending = vec![job.corpus_root.join(relative)];
    let mut visited = HashSet::new();
    while let Some(directory) = pending.pop() {
        let physical =
            std::fs::canonicalize(&directory).map_err(|error| Error::io(&directory, &error))?;
        if destination.starts_with(&physical) {
            return Err(Error::Config(format!(
                "raw invocation directory {} overlaps copied source directory {}",
                destination.display(),
                physical.display()
            )));
        }
        if !recursive || !visited.insert(physical.clone()) {
            continue;
        }
        for entry in std::fs::read_dir(&physical).map_err(|error| Error::io(&physical, &error))? {
            let entry = entry.map_err(|error| Error::io(&physical, &error))?;
            if entry.file_name() == ".git" {
                continue;
            }
            let path = entry.path();
            if std::fs::metadata(&path)
                .map_err(|error| Error::io(&path, &error))?
                .is_dir()
            {
                pending.push(path);
            }
        }
    }
    Ok(destination)
}

/// Resolve existing physical ancestry without creating a missing suffix. A
/// normalized `missing/..` may expose an existing symlink, so resolve again
/// until the path is stable. Dangling links fail before any directory is made.
pub(super) fn physical_destination(root: &Path) -> Result<PathBuf> {
    let mut absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| Error::io(root, &error))?
            .join(root)
    };
    loop {
        let mut ancestor = absolute.as_path();
        let mut physical = loop {
            match std::fs::canonicalize(ancestor) {
                Ok(path) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if std::fs::symlink_metadata(ancestor).is_ok() {
                        return Err(Error::io(ancestor, &error));
                    }
                    ancestor = ancestor
                        .parent()
                        .ok_or_else(|| Error::io(ancestor, &error))?;
                }
                Err(error) => return Err(Error::io(ancestor, &error)),
            }
        };
        let suffix = absolute
            .strip_prefix(ancestor)
            .map_err(|error| Error::Config(format!("raw invocation path suffix: {error}")))?;
        for component in suffix.components() {
            match component {
                Component::Normal(name) => physical.push(name),
                Component::ParentDir => {
                    physical.pop();
                }
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => {
                    return Err(Error::Config(
                        "raw invocation suffix must be relative".into(),
                    ));
                }
            }
        }
        if physical == absolute {
            return Ok(physical);
        }
        absolute = physical;
    }
}

pub(super) enum Scratch {
    Temporary(TempDir),
    Retained(PathBuf),
}

impl Scratch {
    pub(super) fn new(temporary_root: &Path, raw_root: Option<&Path>) -> Result<Self> {
        let Some(root) = raw_root else {
            return TempDir::new_in(temporary_root, "tlc").map(Self::Temporary);
        };
        std::fs::create_dir_all(root).map_err(|error| Error::io(root, &error))?;
        let root = std::fs::canonicalize(root).map_err(|error| Error::io(root, &error))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Error::Command(format!("raw invocation clock: {error}")))?
            .as_nanos();
        let path = root.join(format!(
            "tlc-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // Exclusive creation prevents old evidence from being overwritten.
        std::fs::create_dir(&path).map_err(|error| Error::io(&path, &error))?;
        Ok(Self::Retained(path))
    }

    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Temporary(directory) => directory.path(),
            Self::Retained(path) => path,
        }
    }

    pub(super) fn started(&self, command: &Command, job: &TlcJob<'_>) -> Result<()> {
        let Self::Retained(path) = self else {
            return Ok(());
        };
        // Recording an environment would expose inherited credentials. Only
        // the executable, arguments and cwd are captured, before dispatch.
        let text = |value: &std::ffi::OsStr| {
            value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::Config("raw invocation arguments must be UTF-8".to_owned()))
        };
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(text)
            .collect::<Result<Vec<_>>>()?;
        let cwd = command
            .get_current_dir()
            .map(|directory| text(directory.as_os_str()))
            .transpose()?;
        let manifest = json!({
            "schema": 1,
            "state": "started",
            "phase": if job.dump_trace { "trace" } else { "clean" },
            "module": job.module,
            "config": job.config,
            "argv": argv,
            "cwd": cwd,
            "timeout": {"seconds": job.timeout.as_secs(), "nanoseconds": job.timeout.subsec_nanos()},
            "stdout": "stdout.txt",
            "stderr": "stderr.txt",
            "trace_json_if_emitted": "trace.json"
        });
        write_manifest(&path.join("started.json"), &manifest)
    }

    pub(super) fn finished(&self, outcome: &Result<ProcessOutcome>) -> Result<()> {
        let Self::Retained(path) = self else {
            return Ok(());
        };
        let (name, manifest) = match outcome {
            Ok(outcome) => (
                "completed.json",
                json!({
                    "schema": 1,
                    "state": "completed",
                    "exit_code": outcome.exit_code,
                    "signal": outcome.signal,
                    "timed_out": outcome.timed_out,
                    "elapsed": {"seconds": outcome.elapsed.as_secs(), "nanoseconds": outcome.elapsed.subsec_nanos()}
                }),
            ),
            Err(error) => (
                "harness-error.json",
                json!({"schema": 1, "state": "harness-error", "error": error.to_string()}),
            ),
        };
        write_manifest(&path.join(name), &manifest).map_err(|error| {
            let process = match outcome {
                Ok(outcome) => format!(
                    "exit {:?}, signal {:?}, timed_out {}",
                    outcome.exit_code, outcome.signal, outcome.timed_out
                ),
                Err(error) => error.to_string(),
            };
            Error::Command(format!(
                "raw invocation terminal recording failed: {error}; process result: {process}"
            ))
        })
    }
}

pub(super) fn write_manifest(path: &Path, manifest: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| Error::Command(format!("raw invocation manifest: {error}")))?;
    write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_retained_owner_keeps_started_and_partial_files()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = TempDir::new("raw-owner-drop")?;
        let scratch = Scratch::new(&root.path().join("temporary"), Some(root.path()))?;
        let path = scratch.path().to_path_buf();
        // A stopped harness may never return from dispatch to write a terminal
        // marker. This tests retention, not an actual forced-signal execution.
        write_manifest(&path.join("started.json"), &json!({"state": "started"}))?;
        std::fs::write(path.join("stdout.txt"), [0xff, b'\n'])?;
        drop(scratch);
        assert!(path.join("started.json").exists());
        assert_eq!(std::fs::read(path.join("stdout.txt"))?, [0xff, b'\n']);
        assert!(!path.join("completed.json").exists());
        assert!(!path.join("harness-error.json").exists());
        Ok(())
    }
}
