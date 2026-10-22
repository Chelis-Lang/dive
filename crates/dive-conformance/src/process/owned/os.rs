//! Safe Linux/Darwin waitid and a bounded, trusted leaf `ps` observer.

use super::{Backend, command};
use crate::error::Result;
use crate::util::TempDir;
use rustix::process::{
    Pid, Signal, WaitId, WaitIdOptions, geteuid, kill_process, kill_process_group, waitid,
};
use std::fs::File;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const OBSERVER_LIMIT: Duration = Duration::from_secs(1);
const OBSERVER_OUTPUT: u64 = 8 * 1024 * 1024;

pub(super) struct Os;

pub(super) struct OsChild {
    native: Child,
    // Retained on an unproven leaf cleanup. Never drop a still-owned observer
    // as an accidental side effect of an inspection error.
    observer: Option<Observer>,
}

struct Observer {
    child: Child,
    _directory: TempDir,
    stdout: PathBuf,
    stderr: PathBuf,
    reaped: bool,
    uncertain: bool,
}

pub(super) fn spawn(cmd: &mut Command) -> Result<OsChild> {
    cmd.spawn()
        .map(|native| OsChild {
            native,
            observer: None,
        })
        .map_err(|error| {
            command(format!(
                "cannot start {}: {error}",
                cmd.get_program().to_string_lossy()
            ))
        })
}

fn exited(child: &Child) -> Result<bool> {
    match waitid(
        WaitId::Pid(Pid::from_child(child)),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    ) {
        Ok(None) => Ok(false),
        Ok(Some(status)) if status.exited() || status.killed() || status.dumped() => Ok(true),
        Ok(Some(_)) => Err(command("waitid returned a nonterminal status for EXITED")),
        Err(error) => Err(command(format!(
            "nonreaping wait for {}: {error}",
            child.id()
        ))),
    }
}

impl Observer {
    fn spawn() -> Result<Self> {
        use std::os::unix::process::CommandExt as _;
        let directory = TempDir::new("dive-owned-ps")?;
        let stdout = directory.path().join("stdout");
        let stderr = directory.path().join("stderr");
        let out = File::create(&stdout).map_err(|e| command(format!("creating ps stdout: {e}")))?;
        let err = File::create(&stderr).map_err(|e| command(format!("creating ps stderr: {e}")))?;
        // Fixed system leaf, no user argv/shell. Isolate it from BOTH the native
        // group and the harness group. Its private owner handles cancellation.
        let child = Command::new("/bin/ps")
            .args(["-axo", "pid=,pgid=,uid=,stat="])
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .process_group(0)
            .spawn()
            .map_err(|e| command(format!("starting ps: {e}")))?;
        Ok(Self {
            child,
            _directory: directory,
            stdout,
            stderr,
            reaped: false,
            uncertain: false,
        })
    }

    fn exited(&mut self) -> Result<bool> {
        if self.uncertain {
            return Err(command("ps observer wait ownership is uncertain"));
        }
        let result = exited(&self.child);
        self.uncertain |= result.is_err();
        result
    }

    fn wait(&mut self, deadline: Instant) -> Result<ExitStatus> {
        loop {
            if self.exited()? {
                // This is the fixed, audited leaf observer, not an arbitrary
                // native tree. It has no descendant-ownership claim.
                self.uncertain = true; // No signal can follow an uncertain/failed reap.
                let status = self
                    .child
                    .wait()
                    .map_err(|e| command(format!("reaping ps: {e}")))?;
                self.reaped = true;
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(command("ps observer deadline expired"));
            }
            if self
                .stdout
                .metadata()
                .map_err(|e| command(format!("stat ps output: {e}")))?
                .len()
                > OBSERVER_OUTPUT
                || self
                    .stderr
                    .metadata()
                    .map_err(|e| command(format!("stat ps errors: {e}")))?
                    .len()
                    > OBSERVER_OUTPUT
            {
                return Err(command("ps observer output exceeded its transport budget"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn cleanup(&mut self, deadline: Instant) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        // `waitid` must still recognise the exact direct child before a signal.
        if !self.exited()? {
            kill_process(Pid::from_child(&self.child), Signal::KILL)
                .map_err(|e| command(format!("killing ps observer: {e}")))?;
        }
        self.wait(deadline).map(|_| ())
    }

    fn text(&self) -> Result<String> {
        let mut bytes = Vec::new();
        File::open(&self.stdout)
            .map_err(|e| command(format!("opening ps output: {e}")))?
            .take(OBSERVER_OUTPUT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| command(format!("reading ps output: {e}")))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > OBSERVER_OUTPUT {
            return Err(command("ps output exceeded its transport budget"));
        }
        String::from_utf8(bytes).map_err(|_| command("ps output was not UTF-8"))
    }
}

// The owner deliberately keeps a failed observer in its Option until its
// bounded cleanup is proven. No Drop silently converts a live leaf into success.
fn snapshot(storage: &mut Option<Observer>, deadline: Instant) -> Result<String> {
    if let Some(previous) = storage.as_mut() {
        previous.cleanup(deadline)?;
        *storage = None;
    }
    if Instant::now() >= deadline {
        return Err(command("no time remains to start a ps observer"));
    }
    *storage = Some(Observer::spawn()?);
    let observer = storage
        .as_mut()
        .ok_or_else(|| command("missing ps owner"))?;
    let observed = observer.wait(deadline.min(Instant::now() + OBSERVER_LIMIT));
    let result = match observed {
        Ok(status) if status.success() => observer.text(),
        Ok(status) => Err(command(format!("ps observer exited with {status}"))),
        Err(error) => Err(error),
    };
    let cleanup = observer.cleanup(deadline.min(Instant::now() + OBSERVER_LIMIT));
    if observer.reaped {
        *storage = None;
    }
    match (result, cleanup) {
        (Ok(text), Ok(())) => Ok(text),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(command(format!("{error}; observer cleanup: {cleanup}"))),
    }
}

static PREFLIGHT: Mutex<Option<Observer>> = Mutex::new(None);

pub(super) fn preflight() -> Result<()> {
    // A retained preflight observer needs the same owning storage as native
    // observers. Store it under the global supervisor, not a disposable local.
    let mut storage = PREFLIGHT
        .lock()
        .map_err(|_| command("poisoned preflight observer owner"))?;
    let text = snapshot(&mut storage, Instant::now() + OBSERVER_LIMIT * 2)?;
    let mut found = false;
    for line in text.lines() {
        let (pid, _, uid, _) = row(line)?;
        if pid == std::process::id() && uid == geteuid().as_raw() {
            found = true;
        }
    }
    if found {
        Ok(())
    } else {
        Err(command("ps preflight did not contain the harness process"))
    }
}

pub(super) fn cancel_preflight() -> Result<()> {
    let (mut storage, poisoned) = match PREFLIGHT.lock() {
        Ok(storage) => (storage, false),
        Err(error) => (error.into_inner(), true),
    };
    if let Some(observer) = storage.as_mut() {
        observer.cleanup(Instant::now() + OBSERVER_LIMIT)?;
        *storage = None;
    }
    if poisoned {
        Err(command("poisoned preflight observer; cleanup attempted"))
    } else {
        Ok(())
    }
}

fn row(line: &str) -> Result<(u32, u32, u32, bool)> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 4 {
        return Err(command("malformed ps process row"));
    }
    let number = |index: usize| {
        fields[index]
            .parse::<u32>()
            .map_err(|_| command("malformed ps process ID"))
    };
    Ok((
        number(0)?,
        number(1)?,
        number(2)?,
        !fields[3].starts_with('Z'),
    ))
}

fn parse(text: &str, group: u32, expected_uid: u32) -> Result<bool> {
    let mut leader = false;
    let mut live = false;
    for line in text.lines() {
        let (pid, pgid, uid, is_live) = row(line)?;
        if pgid != group {
            continue;
        }
        if uid != expected_uid {
            return Err(command("foreign UID in owned process group"));
        }
        leader |= pid == group;
        live |= is_live;
    }
    if !leader {
        return Err(command(
            "reserved process-group leader missing from ps snapshot",
        ));
    }
    Ok(live)
}

impl Backend for Os {
    type Child = OsChild;
    type Status = ExitStatus;

    fn exited(&self, child: &mut OsChild) -> Result<bool> {
        exited(&child.native)
    }

    fn live_members(&self, child: &mut OsChild, deadline: Instant) -> Result<bool> {
        let text = snapshot(&mut child.observer, deadline)?;
        parse(&text, child.native.id(), geteuid().as_raw())
    }

    fn kill_group(&self, child: &mut OsChild) -> Result<()> {
        kill_process_group(Pid::from_child(&child.native), Signal::KILL)
            .map_err(|e| command(format!("killing owned group {}: {e}", child.native.id())))
    }

    fn reap(&self, child: &mut OsChild) -> Result<ExitStatus> {
        if child.observer.is_some() {
            return Err(command(
                "refuse native reap while observer ownership remains unresolved",
            ));
        }
        child
            .native
            .wait()
            .map_err(|e| command(format!("reaping owned leader: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_snapshots_require_reserved_leader_and_same_uid() -> Result<()> {
        assert!(parse("12 12 5 Z\n13 12 5 S\n99 98 8 S\n", 12, 5)?);
        assert!(!parse("12 12 5 Z\n13 12 5 Z+\n", 12, 5)?);
        for malformed in [
            "",
            "13 12 5 S\n",
            "12 12 9 S\n",
            "12 12 5\n",
            "oops 12 5 S\n",
        ] {
            assert!(parse(malformed, 12, 5).is_err());
        }
        Ok(())
    }
}
