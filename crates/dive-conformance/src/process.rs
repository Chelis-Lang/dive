//! Running a child process with a hard timeout.
//!
//! The child is placed in its own process group, and on timeout the whole
//! group is killed with `SIGKILL`, so a JVM and anything it started cannot
//! outlive the harness. Output goes to files rather than pipes, so a chatty
//! child can never block on a full pipe.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
fn isolate(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

#[cfg(not(unix))]
fn isolate(_cmd: &mut Command) {}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // Best effort: the group may already be gone.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

#[cfg(unix)]
fn signal_of(status: std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: std::process::ExitStatus) -> Option<i32> {
    None
}

/// Runs `cmd`, capturing stdout and stderr into files under `scratch`, and
/// kills its process group if it runs longer than `timeout`.
///
/// # Errors
/// Fails if the process cannot be started or its output cannot be read.
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
    isolate(cmd);
    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| {
        Error::Command(format!("cannot start {}: {e}", cmd.get_program().display()))
    })?;
    let pid = child.id();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| Error::Command(format!("waiting for {pid}: {e}")))?
        {
            break status;
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            kill_group(pid);
            let _ = child.kill();
            break child
                .wait()
                .map_err(|e| Error::Command(format!("waiting for {pid}: {e}")))?;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    // Reap anything the child left running in its group.
    kill_group(pid);
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
