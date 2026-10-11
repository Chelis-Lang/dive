# 0072: Positioned fallible invariant-error replay

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Pinned TLC `ModelChecker.doNext` enters call-stack replay after an invariant
fails to evaluate on a new state. `doNextSetErr`/`doNextCheckInvariants` continue
through later successors and may admit additional states or meet a second
error, without increasing the generated counter. Resident replay already
reproduces this behavior. The first disk attempt instead inspected its empty
resident reference: `ReplayMembership` gave 4 distinct/2 queued instead of the
clean pinned 3 distinct/1 queued. ADR0068 prevented that wrong answer with a
named refusal. Both old outcomes remain immutable evidence.

Fresh clean reference `ReplayNew` stops at a division-by-zero invariant after
2 generated states, then replay gives 4 distinct states and 2 queued at depth
2 (exit 76, message 2111). Therefore simply retaining the original stopping
counters is also insufficient. The second-error fixture changes the deciding
class to 2103 and exits 75. Exact error-class credit remains limited to known
sites whose existing `verified` diagnostic is true.

## Decision

For the existing opt-in inline INIT/NEXT disk route, replay queries complete
keys through `Result` membership and inserts absent keys through `Result`
insertion, at the original concrete predecessor and emitted successor ordinal.
No Boolean adapter, resident mirror, future candidate resolution or replay
frontier/predecessor publication is introduced. Resident replay and its memory
counterpart are unchanged. Key transformations still use the actual configured
VIEW/symmetry path and the original profile/interner authority.

Replay starts with the original invariant error, frozen generated count and
stopping position. Each completed new insertion increments physical key
admission and the replay distinct contribution before invariant/action-property
checks. The logical queued contribution increments only after those checks
succeed. Duplicates do not become new states, even at a full entry limit. A
second invariant error follows the existing TLC class-selection rule; a further
violation does not replace the original error. Ordered query/insertion failures
halt immediately and retain the original error, positioned first storage cause,
completed replay prefix and diagnostic word. Subsequent cleanup prevents
completion and cannot replace that first cause.

A synthetic query/insertion identity mismatch records typed Corruption at
ReplayInsertion. It does not invent a library I/O-poison flag. The engine stops
and never converts either a library failure or this protocol failure to absence,
duplicate or completed success.

`seen_storage.replay` reports the original error, source/ordinal, frozen generated
count, newly admitted keys, logical queue contribution and any later halt. These
facts do not certify the diagnostic word. I/O, bound and cleanup failures remain
resource-limit results; independently replayable violation certificates are a
separate obligation. Memory reports receive no new replay field.

## Consequences

This extends only ordinary disk safety checking. Public configured PROPERTY and
SPECIFICATION, capture/temporal, checkpoint/recovery and DFID combinations remain
prebinding refusals. The action-property continuation control is a private
already-bound engine seam, not evidence of public role admission. Requested
widths still report actual inline execution. The library's completed writes
have no fsync/crash-durability/import guarantee. Existing per-action successor
vectors and trace reconstruction remain outside all seen/frontier/predecessor
budgets; no total-RAM claim is made. Crate layering and independent trace
validation are unchanged.

Clean raw pin records and the old failed/refusal epochs are linked from
[the study](../../conformance/probes/fallible-invariant-replay/README.md).
Full current-source gate and fresh executable campaign are required before
publication; focused controls alone are not a delivery receipt.
