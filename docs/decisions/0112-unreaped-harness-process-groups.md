# 0112: Retain harness process-group ownership until cleanup

- **Status:** accepted
- **Date:** 2026-10-22

## Context

The conformance harness places each native command in a separate Unix process
group. Its old completion path called `Child::try_wait`, reaping the leader,
before signalling that numeric group ID. Its signal handler also copied numeric
IDs out of the registry before signalling. Neither operation retained ownership
through the later signal, and cancellation could race with a new spawn. These
are source-level races (#430), not evidence that an unrelated process was
actually signalled in a recorded campaign.

Stable `std::process::Child::try_wait` reaps an exited child. The already locked
`rustix` 1.1.5 provides safe `waitid` with `EXITED | NOHANG | NOWAIT`, safe process
signals, and safe process IDs. The cached primary implementation in
`rustix/src/process/{wait,kill}.rs` supports these APIs on Linux and macOS.

## Decision

Add `rustix = "=1.1.5"` with its `process` feature to workspace dependencies and
reference it from the harness under `cfg(unix)`. This is a new direct external
dependency, using the existing locked version. No workspace-internal dependency
edge or unsafe-code exception changes. `std` support supplies the safe
`Pid::from_child` conversion. The existing licence policy applies.

On Linux and macOS, a private owning registry retains each native leader until
its group has no observed live members AND `waitid` observes terminal leader
status. A process-table zombie row alone cannot authorize reap. Every signal and terminal reap is
serialized by that owner's mutex. A retained cancellation handle must recheck
the lifecycle; it cannot signal a group whose owner reached `Reaping` or
`Reaped`. `waitid` observes native completion without reaping. The eventual
`Child::wait` supplies the original normal exit or terminating signal.

The cancellation latch and the entire native spawn/registration transaction
share the registry mutex. Cancellation permanently blocks new work, including
ordinary preflight observers; cleanup observers remain permitted because they
are necessary to drain already-owned work. Successful spawns are owned before
any later fallible operation. Native deadline handling keeps whole-group
`SIGKILL` and completion-before-deadline priority.

A fixed `/bin/ps -axo pid=,pgid=,uid=,stat=` leaf observer supplies bounded group
snapshots. It runs in a separate group with regular output files, a one-second
observation budget, bounded output reads, and its own unreaped direct-child
owner. Missing leaders, foreign UIDs, malformed data or lost wait ownership are
harness infrastructure errors. Observer ownership remains retained until its
cleanup is proved. Preflight checks host observation before starting native
work. No missing snapshot is interpreted as an empty group.

Errors and poisoning latch the supervisor against further work; failed group
owners are not silently removed. Explicit cleanup returns errors. Unwind cleanup
attempts a bounded drain and retains unresolved owners. Cancellation attempts
each registered group with a five-second cleanup budget rather than allowing
one failed group to consume every later group's opportunity. Cleanup failure
is reported; it is never a model verdict or a fabricated native timeout.

## Consequences

The harness adds host inspection and cleanup work. Its wall clock is not a
checker-throughput measurement. Native stdout/stderr files, normal exit,
terminating signal and timeout fields retain their existing meanings. No model
selection, oracle, golden, report-major, semantic limit or checker option changes.

Other targets retain direct-child deadline handling and a serialized cancellation
latch, without claiming descendant containment. Linux/macOS still require
working system process inspection. Cleanup observers are trusted leaf tools,
not arbitrary user commands. These controls do not contain descendants that
deliberately create new sessions, privileged external reapers or changed UIDs.
SIGKILL of the harness and uninterruptible OS operations have no universal
bounded-cleanup guarantee. An unproven drain requires external supervision and
cannot qualify an aggregate report or baseline.

Before qualification, deterministic ownership interleavings and tiny real
Linux/macOS process tests must pass, with the existing timeout/kill assertions
preserved. The first implementation packet is source-only pending review.
