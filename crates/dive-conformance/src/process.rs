//! Running a child with a deadline and explicit process ownership.
//!
//! Linux and macOS retain each process-group leader unreaped until the group
//! has no live members (ADR 0112). Output uses regular files, not pipes.
//! Other targets retain the existing direct-child-only capability; they do not
//! claim descendant cleanup. These are cooperative host controls, not a sandbox
//! against session escape or an absolute bound on uninterruptible OS operations.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod owned;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
static CANCELLED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

/// Permanently prevents further harness children and drains currently owned
/// groups on Linux/macOS. Other targets cannot promise descendant cleanup.
/// Cleanup failure is reported on stderr; it is never a native verdict.
pub fn kill_all_children() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Err(error) = owned::cancel() {
        eprintln!("harness child cleanup failed; external supervision required: {error}");
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        *CANCELLED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
    }
}

/// On `SIGINT` or `SIGTERM`, latches cancellation, attempts bounded owned-group
/// cleanup, then exits with `128 + signal`.
///
/// # Errors
/// Fails if signal notification cannot be registered.
#[cfg(unix)]
pub fn install_signal_handler() -> Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    let mut signals = signal_hook::iterator::Signals::new([SIGINT, SIGTERM])
        .map_err(|e| Error::Command(format!("cannot install signal handler: {e}")))?;
    std::thread::spawn(move || {
        if let Some(sig) = signals.forever().next() {
            kill_all_children();
            std::process::exit(128 + sig);
        }
    });
    Ok(())
}

/// No-op on platforms without Unix signals.
///
/// # Errors
/// Never.
#[cfg(not(unix))]
pub fn install_signal_handler() -> Result<()> {
    Ok(())
}

/// How a child process ended.
#[derive(Debug, Clone)]
pub struct ProcessOutcome {
    /// Exit code, if it exited normally.
    pub exit_code: Option<i32>,
    /// Terminating signal, if any (Unix).
    pub signal: Option<i32>,
    /// Whether the harness killed it on timeout.
    pub timed_out: bool,
    /// Captured stdout (lossy UTF-8).
    pub stdout: String,
    /// Captured stderr (lossy UTF-8).
    pub stderr: String,
    /// Wall-clock time.
    pub elapsed: Duration,
}

#[cfg(unix)]
fn signal_of(status: std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: std::process::ExitStatus) -> Option<i32> {
    None
}

/// Runs `cmd`, capturing stdout and stderr into files under `scratch`.
/// Linux/macOS kill and prove quiescence of the owned group before leader reap.
/// Other targets can kill only the direct child.
///
/// # Errors
/// Fails on launch, cancellation, process/ownership inspection, cleanup or I/O
/// failure. Unproven group cleanup latches the supervisor against further work.
pub fn run_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    scratch: &Path,
) -> Result<ProcessOutcome> {
    let out_path = scratch.join("stdout.txt");
    let err_path = scratch.join("stderr.txt");
    let out = std::fs::File::create(&out_path).map_err(|e| Error::io(&out_path, &e))?;
    let err = std::fs::File::create(&err_path).map_err(|e| Error::io(&err_path, &e))?;
    cmd.stdin(Stdio::null()).stdout(out).stderr(err);
    let start = Instant::now();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let (status, timed_out) = owned::run(cmd, timeout)?;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let (status, timed_out) = direct_child(cmd, timeout)?;
    let elapsed = start.elapsed();
    let read = |p: &Path| {
        std::fs::read(p)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| Error::io(p, &e))
    };
    Ok(ProcessOutcome {
        exit_code: status.code(),
        signal: signal_of(status),
        timed_out,
        stdout: read(&out_path)?,
        stderr: read(&err_path)?,
        elapsed,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn direct_child(cmd: &mut Command, timeout: Duration) -> Result<(std::process::ExitStatus, bool)> {
    let mut child = {
        let cancelled = CANCELLED
            .lock()
            .map_err(|_| Error::Command("poisoned child launch latch".into()))?;
        if *cancelled {
            return Err(Error::Command(
                "harness cancellation prevents new children".into(),
            ));
        }
        cmd.spawn()
            .map_err(|e| Error::Command(format!("cannot start child: {e}")))?
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, false)),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                child
                    .kill()
                    .map_err(|e| Error::Command(format!("killing direct child: {e}")))?;
                return child
                    .wait()
                    .map(|status| (status, true))
                    .map_err(|e| Error::Command(format!("waiting for direct child: {e}")));
            }
            Err(error) => {
                let _ = child.kill();
                let cleanup = child.wait();
                return Err(Error::Command(format!(
                    "waiting for direct child: {error}; cleanup: {cleanup:?}"
                )));
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn captures_exit_and_output() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("proc")?;
        let out = run_with_timeout(
            Command::new("sh").args(["-c", "echo hi; echo err >&2; exit 12"]),
            Duration::from_secs(30),
            dir.path(),
        )?;
        assert_eq!(out.exit_code, Some(12));
        assert_eq!(out.stdout, "hi\n");
        assert_eq!(out.stderr, "err\n");
        assert!(!out.timed_out);
        Ok(())
    }

    #[test]
    fn timeout_kills_the_process_tree() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("proc")?;
        let marker = dir.path().join("grandchild-survived");
        let script = format!("(sleep 2; touch '{}') & sleep 30", marker.display());
        let out = run_with_timeout(
            Command::new("sh").args(["-c", &script]),
            Duration::from_millis(300),
            dir.path(),
        )?;
        assert!(out.timed_out);
        assert_eq!(out.exit_code, None);
        std::thread::sleep(Duration::from_secs(3));
        assert!(
            !marker.exists(),
            "background grandchild outlived the timeout"
        );
        Ok(())
    }
}
