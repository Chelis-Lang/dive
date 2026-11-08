# 0156: Default Store reclamation trigger

- **Status:** accepted
- **Date:** 2026-11-07

## Context

ADR 0149 added completed-level Store reclamation as an opt-in
(`--store-compact-bytes`). It preserves every result: verdict, counts, depth,
counterexample replay and warnings. Under ADR 0153, result-preserving
improvements become defaults once the harness shows they match the goldens.

Measured on the Examples corpus at 1 worker, the Store of most pure safety
searches stays well below 256 MiB (PaxosCommit peaks near 82 MB, MCBoulanger
below 64 MB). A collection was expensive: the permanent startup image drops
the cached `CONCRETE` materialization proof, so every membership test against a
compiled constant re-walked the value afterwards. One collection doubled
MCNanoMedium's run time (callgrind 110G -> 244G Ir).

## Decision

When `store_compact_bytes` is omitted, Core uses a 256 MiB trigger for pure
fingerprint safety BFS in both profiles. It is not used with exact membership,
frontier, candidate, predecessor or disk storage budgets, runtime effects,
temporal properties, sampling, checkpoints, the debugger or distributed
workers. An explicit value, including recovery, keeps the ADR 0149 behaviour
and its named refusals.

Under the default, a reclamation refusal or allocation failure skips
reclamation for the rest of the search instead of failing it. Every replacement
is built before anything is installed, so the original Store, Machines and
frontier stay valid. Cancellation still stops the search. The report records
`trigger_source` and the `skipped` reason.

The replacement Store restores the source's `CONCRETE` bit on each unchanged
permanent node. The node, its children and every name token are identical, so
the cached proof still holds.

## Consequences

With the restored proof, a 64 MiB trigger costs MCNanoMedium +0.7% Ir for one
collection and PaxosCommit +0.5% Ir for five, while PaxosCommit's peak RSS falls
from 319 MB to 217 MB. Searches whose Store grows past 256 MiB release expanded values with no live
root. Smaller searches pay nothing beyond one startup image capture. The
trigger is not a memory cap; ADR 0149's list of unbounded owners still applies.
Exact membership and spill combinations keep resident behaviour unless they
request reclamation explicitly.
