//! Serialized, unreaped process-group ownership (ADR 0112).

use crate::error::{Error, Result};
use std::process::{Command, ExitStatus};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

mod os;

const CLEANUP: Duration = Duration::from_secs(5);
const PULSE: Duration = Duration::from_millis(25);

// Backend operations never reap except `reap`. The private owner is the ONLY
// caller that can signal or reap. Tests inject deterministic interleavings here.
trait Backend: Send + Sync {
    type Child: Send;
    type Status: Clone + Send;
    fn exited(&self, child: &mut Self::Child) -> Result<bool>;
    fn live_members(&self, child: &mut Self::Child, deadline: Instant) -> Result<bool>;
    fn kill_group(&self, child: &mut Self::Child) -> Result<()>;
    fn reap(&self, child: &mut Self::Child) -> Result<Self::Status>;
}

enum Phase<S> {
    Owned,
    // Set BEFORE calling a reaping API. An error/unwind can never re-enable a
    // numeric signal after an uncertain reap.
    Reaping,
    Reaped(S),
    Quarantined(String),
}

struct Group<B: Backend> {
    child: Option<B::Child>,
    phase: Phase<B::Status>,
}

impl<B: Backend> Group<B> {
    fn child(&mut self) -> Result<&mut B::Child> {
        self.child
            .as_mut()
            .ok_or_else(|| command("uninitialised group owner"))
    }

    fn exited(&mut self, backend: &B) -> Result<bool> {
        match &self.phase {
            Phase::Reaped(_) => return Ok(true),
            Phase::Reaping => return Err(command("uncertain reap; refuse further signals")),
            Phase::Quarantined(error) => return Err(command(error)),
            Phase::Owned => {}
        }
        let result = backend.exited(self.child()?);
        if let Err(error) = &result {
            self.phase = Phase::Quarantined(error.to_string());
        }
        result
    }

    fn drain(&mut self, backend: &B, deadline: Instant) -> Result<B::Status> {
        match &self.phase {
            Phase::Reaped(status) => return Ok(status.clone()),
            Phase::Reaping => return Err(command("uncertain reap; refuse further signals")),
            Phase::Quarantined(error) => return Err(command(error)),
            Phase::Owned => {}
        }
        self.drain_owned(backend, deadline)
    }

    fn drain_owned(&mut self, backend: &B, deadline: Instant) -> Result<B::Status> {
        let mut signalled = false;
        loop {
            if Instant::now() >= deadline {
                return Err(command("group cleanup deadline expired without quiescence"));
            }
            // Revalidate waitability before EVERY possible numeric group signal.
            // This does not reap; an external/unknown reaper fails closed.
            let leader_exited = self.exited(backend)?;
            let live_members = backend.live_members(self.child()?, deadline)?;
            // A process-table Z row alone is insufficient: on Linux a dead
            // thread-group leader can coexist with running sibling threads.
            // Require terminal waitid as well, so the eventual wait cannot block
            // on a process that was only *apparently* quiescent in `ps`.
            if leader_exited && !live_members {
                self.phase = Phase::Reaping;
                let status = backend.reap(self.child()?)?;
                self.phase = Phase::Reaped(status.clone());
                return Ok(status);
            }
            if !signalled {
                backend.kill_group(self.child()?)?;
                signalled = true;
            }
            std::thread::sleep(PULSE.min(deadline.saturating_duration_since(Instant::now())));
        }
    }
}

struct Registry<B: Backend> {
    halted: Option<String>,
    // Owning handles, not naked process/group IDs. A retained cancellation
    // snapshot must lock and recheck the lifecycle before it can act.
    entries: Vec<Arc<Mutex<Group<B>>>>,
}

struct Supervisor<B: Backend> {
    backend: B,
    registry: Mutex<Registry<B>>,
}

impl<B: Backend> Supervisor<B> {
    const fn new(backend: B) -> Self {
        Self {
            backend,
            registry: Mutex::new(Registry {
                halted: None,
                entries: Vec::new(),
            }),
        }
    }

    fn spawn(&self, launch: impl FnOnce() -> Result<B::Child>) -> Result<Arc<Mutex<Group<B>>>> {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => {
                poisoned.into_inner().halted = Some("poisoned spawn registry".into());
                return Err(command("poisoned spawn registry; no further children"));
            }
        };
        if let Some(reason) = &registry.halted {
            return Err(command(format!(
                "harness no longer permits children: {reason}"
            )));
        }
        registry
            .entries
            .try_reserve(1)
            .map_err(|e| command(format!("reserving child ownership: {e}")))?;
        // Allocate the owner BEFORE launch. No fallible operation or allocation
        // lies between successful spawn and registered ownership.
        let handle = Arc::new(Mutex::new(Group {
            child: None,
            phase: Phase::Owned,
        }));
        let mut group = handle
            .lock()
            .map_err(|_| command("new group owner was poisoned"))?;
        group.child = Some(match launch() {
            Ok(child) => child,
            Err(error) => {
                registry.halted = Some(format!("child launch/preflight failed: {error}"));
                return Err(error);
            }
        });
        registry.entries.push(Arc::clone(&handle));
        drop(group);
        Ok(handle)
    }

    fn halt(&self, reason: &str) {
        // Recover only to latch and retain ownership, never to silently resume.
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry.halted.get_or_insert_with(|| reason.to_owned());
    }

    fn drain(&self, handle: &Arc<Mutex<Group<B>>>, deadline: Instant) -> Result<B::Status> {
        let (mut group, poisoned) = match handle.lock() {
            Ok(group) => (group, false),
            Err(error) => (error.into_inner(), true),
        };
        let result = group.drain(&self.backend, deadline);
        let reaped = matches!(&group.phase, Phase::Reaped(_));
        drop(group); // Lock order: never acquire the registry while holding a group.
        if poisoned || result.is_err() {
            self.halt("child ownership/cleanup failure");
        }
        let mut registry_poisoned = false;
        if reaped {
            let mut registry = match self.registry.lock() {
                Ok(registry) => registry,
                Err(error) => {
                    registry_poisoned = true;
                    let mut registry = error.into_inner();
                    registry
                        .halted
                        .get_or_insert_with(|| "poisoned completion registry".into());
                    registry
                }
            };
            registry.entries.retain(|entry| !Arc::ptr_eq(entry, handle));
        }
        if poisoned || registry_poisoned {
            return Err(command(
                "poisoned group owner; cleanup attempted, no native result admitted",
            ));
        }
        result
    }

    fn cancel(&self) -> Result<()> {
        let (handles, poisoned) = {
            let (mut registry, poisoned) = match self.registry.lock() {
                Ok(registry) => (registry, false),
                Err(error) => (error.into_inner(), true),
            };
            // Same mutex as the entire spawn/registration transaction. Once
            // latched, no thread can spawn after the cancellation snapshot.
            registry.halted.get_or_insert_with(|| {
                if poisoned {
                    "poisoned cancellation registry".into()
                } else {
                    "cancelled".into()
                }
            });
            (registry.entries.clone(), poisoned)
        };
        let mut failures = Vec::new();
        for handle in handles {
            // Each registered group gets its own bounded attempt. One failed
            // group cannot exhaust the cleanup opportunity of every later one.
            if let Err(error) = self.drain(&handle, Instant::now() + CLEANUP) {
                failures.push(error.to_string());
            }
        }
        if poisoned {
            failures.push("poisoned cancellation registry; cleanup attempted".into());
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(command(failures.join("; ")))
        }
    }

    fn wait(
        &self,
        handle: &Arc<Mutex<Group<B>>>,
        start: Instant,
        timeout: Duration,
    ) -> Result<(B::Status, bool)> {
        loop {
            let exited = {
                let mut group = handle.lock().map_err(|_| command("poisoned group owner"))?;
                group.exited(&self.backend)?
            };
            // Preserve the old priority: a completed native status is checked
            // before the wall-clock deadline.
            if exited {
                return self
                    .drain(handle, Instant::now() + CLEANUP)
                    .map(|s| (s, false));
            }
            if start.elapsed() >= timeout {
                return self
                    .drain(handle, Instant::now() + CLEANUP)
                    .map(|s| (s, true));
            }
            std::thread::sleep(PULSE);
        }
    }
}

struct Lease<'a, B: Backend> {
    supervisor: &'a Supervisor<B>,
    handle: Arc<Mutex<Group<B>>>,
}

impl<B: Backend> Drop for Lease<'_, B> {
    fn drop(&mut self) {
        // Every error/unwind has an owner. Failed drains stay registered and
        // permanently halt future spawns; they are never silently untracked.
        if let Err(error) = self
            .supervisor
            .drain(&self.handle, Instant::now() + CLEANUP)
        {
            self.supervisor.halt(&error.to_string());
        }
    }
}

fn command(message: impl Into<String>) -> Error {
    Error::Command(message.into())
}

static OWNER: LazyLock<Supervisor<os::Os>> = LazyLock::new(|| Supervisor::new(os::Os));

pub(super) fn run(cmd: &mut Command, timeout: Duration) -> Result<(ExitStatus, bool)> {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
    let mut start = Instant::now();
    let handle = OWNER.spawn(|| {
        // The preflight itself is inside the spawn/cancellation transaction.
        // No new observer or native process can start after cancellation latches.
        os::preflight()?;
        start = Instant::now();
        os::spawn(cmd)
    })?;
    let lease = Lease {
        supervisor: &OWNER,
        handle,
    };
    let result = OWNER.wait(&lease.handle, start, timeout);
    if result.is_err() {
        OWNER.halt("native supervision failed");
    }
    result
}

pub(super) fn cancel() -> Result<()> {
    let groups = OWNER.cancel();
    let observer = os::cancel_preflight();
    match (groups, observer) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(observer)) => {
            Err(command(format!("{error}; preflight cleanup: {observer}")))
        }
    }
}

#[cfg(test)]
mod tests;
