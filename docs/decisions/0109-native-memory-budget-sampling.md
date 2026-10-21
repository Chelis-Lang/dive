# ADR 0109: Native memory-budget boundary sampling

- **Status:** accepted (2026-10-21).
- **Supersedes:** ADR 0107's delayed-only memory sampling and silent unavailable-memory fallback.

## Context

A short check could complete before the first resident-memory observation and
publish success despite an explicit one-byte ceiling. Both profiles reproduced
this on the documented Counter example. The watchdog deliberately deferred its
first observation by 100 ms on Linux and 500 ms on macOS to avoid a fixed child
process cost. A configured ceiling did not establish that memory was observed.

## Decision

Every finite-memory budget takes an initial observation before model binding and
a terminal observation before its owner publishes a holding result. Periodic
samples remain. Nested already-armed operations share one watchdog and do not
repeat boundary sampling for each internal phase. Earlier genuine errors and
independently validated witnesses retain their actual precedence; a future
binding error is never evaluated after initial known exhaustion. Source and
configuration loading errors remain earlier than arming. A terminal clock check
also prevents publishing success after a deadline merely because the watchdog
thread was delayed. Known zero-time exhaustion is published before spawning it.
Terminal exhaustion is reported at `finalization`, preserving the actual
completed capture and progress without claiming another search/product phase.

Use the safe `memory-stats` 1.2.0 API only on the macOS target of `dive-check`.
It invokes the native current-task Mach RSS interface without a child process.
The dependency is MIT OR Apache-2.0 and uses the already-locked `libc` on macOS.
Its Windows dependencies add lockfile metadata but are not compiled by this
macOS-only edge. Keep Linux's existing checked `/proc/self/status` RSS plus swap
parser; the dependency's smaps implementation has different cost and scope.
Workspace code continues to forbid unsafe. No OS FFI is copied into dive.

Unavailable physical RAM cannot silently turn default/auto into unlimited.
Unavailable RSS is a typed `DIVE-R0002` measurement error, never zero memory,
`unsupported`, a fabricated stopped observation or a holding result. Watchdog
thread startup errors retain `DIVE-R0001`. Fallible `Budget::poll` retains one
atomic load on its ordinary path; this changes the unpublished Rust API.
Configured/requested ceilings remain separate from additive runtime sampling
metadata. The observed maximum is not a true process peak. Linux counts RSS and
swap; macOS counts RSS without a compressed/swap or physical-footprint claim.

Sampling is a guard, not an allocation-time hard cap. Transient excursions
between observations remain possible. This decision does not remove any
liveness count bound or solve interruption inside a long evaluator operation.
Unrestricted dive memory is `--max-memory none`. Finite ceilings retain the
TLC `-Xmx` counterpart and host-resource policy of ADR 0107 in both profiles.

## Alternatives and evidence

Existing workspace dependencies expose no suitable direct safe current-RSS API.
`getrusage` is historical peak memory and can include previous library runs.
`sysinfo` is broader and its private Darwin task-info path can replace a failed
query with zero-filled data. `libproc` requires bindgen/Clang for this narrow
need. Keeping `ps` imposes a process-spawn cost on short checks.

An isolated safe probe pinned to libc 0.2.190 measured three warmed 10,000-call
native medians of 459–500 ns and cold samples of 1,125–1,958 ns, versus 20-call
`ps` medians of 1.135–1.192 ms. The diagnostic ran on a shared macOS host and
makes no quiet-machine or integrated-check performance claim. Dependency/source
and licence checks passed; raw observations remain outside the repository.
Focused failure/precedence controls, the full checker gate, and trivial-check
wall-time diagnostics passed. Normal-input instructions remained within 1% on
every ADR 0104 cell and on a genuine parallel path. The
[qualification](../developer/performance.md#native-memory-budget-sampling-qualification)
records the complete matrix, source identity and measurement limitations.
