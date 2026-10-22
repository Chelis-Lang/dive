//! Deterministic lifecycle controls plus tiny real Unix process trees.

use super::*;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Leader {
    #[default]
    Running,
    Exited,
    Reaped,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Fault {
    #[default]
    None,
    MembersOnce,
    LostWait,
    UnwindAfterReap,
}

#[derive(Default)]
struct Fake {
    leader: Leader,
    live: bool,
    fault: Fault,
    events: Vec<&'static str>,
}

struct FakeBackend;
type FakeChild = Arc<Mutex<Fake>>;

impl Backend for FakeBackend {
    type Child = FakeChild;
    type Status = i32;

    fn exited(&self, child: &mut FakeChild) -> Result<bool> {
        let mut child = child.lock().map_err(|_| command("poisoned fake"))?;
        assert_ne!(
            child.leader,
            Leader::Reaped,
            "nonreaping observation after reap"
        );
        child.events.push("observe");
        if child.fault == Fault::LostWait {
            return Err(command("injected lost wait ownership"));
        }
        Ok(child.leader == Leader::Exited)
    }

    fn live_members(&self, child: &mut FakeChild, _: Instant) -> Result<bool> {
        let mut child = child.lock().map_err(|_| command("poisoned fake"))?;
        assert_ne!(child.leader, Leader::Reaped);
        child.events.push("members");
        if child.fault == Fault::MembersOnce {
            child.fault = Fault::None;
            return Err(command("injected missing membership observation"));
        }
        Ok(child.live)
    }

    fn kill_group(&self, child: &mut FakeChild) -> Result<()> {
        let mut child = child.lock().map_err(|_| command("poisoned fake"))?;
        assert_ne!(child.leader, Leader::Reaped, "numeric signal after reap");
        child.events.push("kill");
        child.live = false;
        child.leader = Leader::Exited;
        Ok(())
    }

    fn reap(&self, child: &mut FakeChild) -> Result<i32> {
        let mut child = child.lock().map_err(|_| command("poisoned fake"))?;
        assert!(!child.live);
        assert_eq!(child.leader, Leader::Exited);
        child.events.push("reap");
        child.leader = Leader::Reaped;
        if child.fault == Fault::UnwindAfterReap {
            // Inject unwind after a backend has reaped. The owner's Reaping
            // phase must survive poisoning and prevent ANY subsequent signal.
            std::panic::resume_unwind(Box::new("injected backend unwind"));
        }
        Ok(23)
    }
}

fn fake(live: bool) -> FakeChild {
    Arc::new(Mutex::new(Fake {
        live,
        leader: if live {
            Leader::Running
        } else {
            Leader::Exited
        },
        ..Fake::default()
    }))
}

#[test]
fn retained_handle_after_reap_cannot_signal_or_reap_again() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    let child = fake(true);
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    assert_eq!(owner.drain(&handle, Instant::now() + CLEANUP)?, 23);
    let original = child
        .lock()
        .map_err(|_| command("fake lock"))?
        .events
        .clone();
    owner.cancel()?;
    assert_eq!(owner.drain(&handle, Instant::now() + CLEANUP)?, 23);
    assert_eq!(
        child.lock().map_err(|_| command("fake lock"))?.events,
        original
    );
    assert!(
        owner
            .spawn(|| Err(command("must not reach this launch")))
            .is_err()
    );
    Ok(())
}

#[test]
fn cancellation_serializes_spawn_registration_and_latches_future_work() -> Result<()> {
    let owner = Arc::new(Supervisor::new(FakeBackend));
    let child = fake(true);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (cancel_tx, cancel_rx) = mpsc::channel();
    let worker_owner = Arc::clone(&owner);
    let worker_child = Arc::clone(&child);
    let worker = std::thread::spawn(move || {
        worker_owner.spawn(|| {
            entered_tx
                .send(())
                .map_err(|_| command("entered channel"))?;
            release_rx
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| command("release channel"))?;
            Ok(worker_child)
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| command("spawn did not enter"))?;
    // Deterministic overlap proof, independent of scheduler timing: the launch
    // closure is blocked on release_rx and MUST still hold the registry mutex.
    // Moving launch outside the transaction makes this assertion fail.
    assert!(matches!(
        owner.registry.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));
    let cancel_owner = Arc::clone(&owner);
    let cancellation = std::thread::spawn(move || {
        cancel_tx.send(()).map_err(|_| command("cancel channel"))?;
        cancel_owner.cancel()
    });
    cancel_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| command("cancel did not enter"))?;
    release_tx.send(()).map_err(|_| command("release failed"))?;
    let handle = worker
        .join()
        .map_err(|_| command("spawn worker unwound"))??;
    cancellation
        .join()
        .map_err(|_| command("cancellation unwound"))??;
    assert!(matches!(
        &handle.lock().map_err(|_| command("owner lock"))?.phase,
        Phase::Reaped(23)
    ));
    let mut invoked = false;
    assert!(
        owner
            .spawn(|| {
                invoked = true;
                Ok(fake(false))
            })
            .is_err()
    );
    assert!(!invoked);
    let events = child
        .lock()
        .map_err(|_| command("fake lock"))?
        .events
        .clone();
    assert_eq!(events.iter().filter(|&&e| e == "kill").count(), 1);
    assert_eq!(events.iter().filter(|&&e| e == "reap").count(), 1);
    Ok(())
}

#[test]
fn two_cancel_snapshots_share_one_terminal_transition() -> Result<()> {
    let owner = Arc::new(Supervisor::new(FakeBackend));
    let child = fake(true);
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    std::thread::scope(|scope| -> Result<()> {
        let first = scope.spawn(|| owner.cancel());
        let second = scope.spawn(|| owner.cancel());
        first
            .join()
            .map_err(|_| command("first cancellation unwind"))??;
        second
            .join()
            .map_err(|_| command("second cancellation unwind"))??;
        Ok(())
    })?;
    assert_eq!(owner.drain(&handle, Instant::now() + CLEANUP)?, 23);
    let events = &child.lock().map_err(|_| command("fake lock"))?.events;
    assert_eq!(events.iter().filter(|&&e| e == "kill").count(), 1);
    assert_eq!(events.iter().filter(|&&e| e == "reap").count(), 1);
    Ok(())
}

#[test]
fn missing_observation_holds_new_work_but_retains_owner_for_cleanup() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    let child = fake(true);
    child.lock().map_err(|_| command("fake lock"))?.fault = Fault::MembersOnce;
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    assert!(owner.drain(&handle, Instant::now() + CLEANUP).is_err());
    assert_eq!(
        owner
            .registry
            .lock()
            .map_err(|_| command("registry"))?
            .entries
            .len(),
        1
    );
    assert_ne!(
        child.lock().map_err(|_| command("fake lock"))?.leader,
        Leader::Reaped
    );
    assert!(owner.spawn(|| Ok(fake(false))).is_err());
    assert_eq!(owner.drain(&handle, Instant::now() + CLEANUP)?, 23);
    assert!(owner.spawn(|| Ok(fake(false))).is_err());
    Ok(())
}

#[test]
fn lost_wait_ownership_never_authorizes_a_later_numeric_signal() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    let child = fake(true);
    child.lock().map_err(|_| command("fake lock"))?.fault = Fault::LostWait;
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    assert!(owner.drain(&handle, Instant::now() + CLEANUP).is_err());
    assert!(owner.cancel().is_err());
    assert!(owner.spawn(|| Ok(fake(false))).is_err());
    let events = &child.lock().map_err(|_| command("fake lock"))?.events;
    assert_eq!(events, &["observe"]);
    assert_eq!(
        owner
            .registry
            .lock()
            .map_err(|_| command("registry"))?
            .entries
            .len(),
        1
    );
    Ok(())
}

#[test]
fn poisoned_owner_after_uncertain_reap_cannot_signal() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    let child = fake(false);
    child.lock().map_err(|_| command("fake lock"))?.fault = Fault::UnwindAfterReap;
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.drain(&handle, Instant::now() + CLEANUP);
    }));
    assert!(outcome.is_err());
    assert!(owner.drain(&handle, Instant::now() + CLEANUP).is_err());
    assert!(owner.spawn(|| Ok(fake(false))).is_err());
    let events = &child
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .events;
    assert_eq!(events, &["observe", "members", "reap"]);
    Ok(())
}

#[test]
fn poisoned_registry_retains_children_and_reports_cleanup_failure() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    let child = fake(true);
    let _handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _registry = owner
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::panic::resume_unwind(Box::new("injected registry unwind"));
    }));
    assert!(outcome.is_err());
    assert!(owner.cancel().is_err()); // Cleanup success cannot hide poisoning.
    assert_eq!(
        child.lock().map_err(|_| command("fake lock"))?.leader,
        Leader::Reaped
    );
    assert!(owner.spawn(|| Ok(fake(false))).is_err());
    Ok(())
}

#[test]
fn failed_launch_has_no_unregistered_child_and_halts_later_work() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    assert!(
        owner
            .spawn(|| Err(command("injected launch failure")))
            .is_err()
    );
    assert_eq!(
        owner
            .registry
            .lock()
            .map_err(|_| command("registry"))?
            .entries
            .len(),
        0
    );
    let mut invoked = false;
    assert!(
        owner
            .spawn(|| {
                invoked = true;
                Ok(fake(true))
            })
            .is_err()
    );
    assert!(!invoked);
    Ok(())
}

#[test]
fn completed_status_precedes_zero_deadline_and_timeout_keeps_kill_status() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    for (live, timed_out) in [(false, false), (true, true)] {
        let handle = owner.spawn(|| Ok(fake(live)))?;
        assert_eq!(
            owner.wait(&handle, Instant::now(), Duration::ZERO)?,
            (23, timed_out)
        );
    }
    Ok(())
}

#[test]
fn nonlive_snapshot_alone_cannot_authorize_terminal_reap() -> Result<()> {
    let owner = Supervisor::new(FakeBackend);
    // Models an apparently zombie leader while waitid still says nonterminal.
    let child = fake(false);
    child.lock().map_err(|_| command("fake lock"))?.leader = Leader::Running;
    let handle = owner.spawn(|| Ok(Arc::clone(&child)))?;
    assert_eq!(
        owner.wait(&handle, Instant::now(), Duration::ZERO)?,
        (23, true)
    );
    let events = &child.lock().map_err(|_| command("fake lock"))?.events;
    let kill = events.iter().position(|&event| event == "kill");
    let reap = events.iter().position(|&event| event == "reap");
    assert!(kill.is_some() && reap.is_some() && kill < reap);
    Ok(())
}

fn real(
    owner: &Supervisor<os::Os>,
    cmd: &mut Command,
    timeout: Duration,
) -> Result<(ExitStatus, bool)> {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0).stdin(std::process::Stdio::null());
    let start = Instant::now();
    let handle = owner.spawn(|| os::spawn(cmd))?;
    let lease = Lease {
        supervisor: owner,
        handle,
    };
    owner.wait(&lease.handle, start, timeout)
}

#[test]
fn real_exit_signal_and_binary_output_remain_authentic() -> Result<()> {
    use std::os::unix::process::ExitStatusExt as _;
    let owner = Supervisor::new(os::Os);
    let directory = crate::util::TempDir::new("owned-status")?;
    let output = directory.path().join("output");
    let (status, timed_out) = real(
        &owner,
        Command::new("sh")
            .args(["-c", "printf '\\377\\000'; exit 23"])
            .stdout(std::fs::File::create(&output).map_err(|e| Error::io(&output, &e))?),
        Duration::from_secs(5),
    )?;
    assert_eq!(status.code(), Some(23));
    assert!(!timed_out);
    assert_eq!(
        std::fs::read(&output).map_err(|e| Error::io(&output, &e))?,
        [255, 0]
    );
    let (status, timed_out) = real(
        &owner,
        Command::new("sh").args(["-c", "kill -TERM $$"]),
        Duration::from_secs(5),
    )?;
    assert_eq!(status.signal(), Some(15));
    assert!(!timed_out);
    Ok(())
}

#[test]
fn real_normal_exit_drains_lingering_descendant_before_reap() -> Result<()> {
    use std::io::Read as _;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt as _;
    let owner = Supervisor::new(os::Os);
    let directory = crate::util::TempDir::new("owned-descendant")?;
    let marker = directory.path().join("survived");
    let (mut endpoint, inherited) =
        UnixStream::pair().map_err(|e| command(format!("socket pair: {e}")))?;
    endpoint
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| command(format!("socket timeout: {e}")))?;
    let input: OwnedFd = inherited.into();
    let mut native = Command::new("sh");
    native
        .arg("-c")
        .arg(
            r#"sh -c 'printf R >&0 || exit 72; IFS= read -r gate || exit 0; touch "$1"' owned-descendant "$1" <&0 & exit 7"#,
        )
        .arg("owned-test")
        .arg(&marker)
        .stdin(std::process::Stdio::from(input))
        .process_group(0);
    let start = Instant::now();
    let handle = owner.spawn(|| os::spawn(&mut native))?;
    // Command retains the parent's copy of its Stdio FD after spawn. Close it
    // before observing EOF, so only actual child/descendant holders can keep the
    // peer alive. The descendant blocks on read until killed or test teardown.
    drop(native);
    let lease = Lease {
        supervisor: &owner,
        handle,
    };
    // Require an authentic descendant-ready byte before starting normal drain.
    // A failed exec or a fixture that never held the socket cannot pass by EOF.
    let mut ready = [0_u8; 1];
    endpoint
        .read_exact(&mut ready)
        .map_err(|e| command(format!("descendant readiness: {e}")))?;
    assert_eq!(ready, [b'R']);
    let (status, timed_out) = owner.wait(&lease.handle, start, Duration::from_secs(5))?;
    assert_eq!(status.code(), Some(7));
    assert!(!timed_out);
    // A no-drain mutation leaves the descendant holding the socket: this read
    // times out (FAILS), rather than letting immediate marker absence pass. No
    // PID is signalled after reap. Unwinding closes endpoint; the fake exits on
    // EOF, without touching the marker, even in that failed mutation.
    let mut byte = [0_u8; 1];
    assert_eq!(
        endpoint
            .read(&mut byte)
            .map_err(|e| command(format!("descendant still holds socket: {e}")))?,
        0
    );
    assert!(!marker.exists());
    assert_eq!(
        owner
            .registry
            .lock()
            .map_err(|_| command("registry"))?
            .entries
            .len(),
        0
    );
    Ok(())
}

#[test]
fn real_cancellation_drains_two_owned_groups_and_blocks_next_launch() -> Result<()> {
    use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
    let owner = Supervisor::new(os::Os);
    let mut leases = Vec::new();
    for _ in 0..2 {
        let mut command = Command::new("sh");
        command.args(["-c", "exec sleep 30"]).process_group(0);
        let handle = owner.spawn(|| os::spawn(&mut command))?;
        leases.push(Lease {
            supervisor: &owner,
            handle,
        });
    }
    owner.cancel()?;
    for lease in &leases {
        let (status, timed_out) = owner.wait(&lease.handle, Instant::now(), Duration::ZERO)?;
        assert_eq!(status.signal(), Some(9));
        assert!(!timed_out); // Cancellation is not relabelled as a native deadline.
    }
    let mut invoked = false;
    assert!(
        owner
            .spawn(|| {
                invoked = true;
                os::spawn(Command::new("true").arg("ignored"))
            })
            .is_err()
    );
    assert!(!invoked);
    assert_eq!(
        owner
            .registry
            .lock()
            .map_err(|_| command("registry"))?
            .entries
            .len(),
        0
    );
    Ok(())
}
