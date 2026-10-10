# 0024: Ordered transient frontier spill

- **Status:** accepted
- **Date:** 2026-10-10

## Context

The safety engine retains current and next BFS frontiers as concrete state
vectors. Pinned TLC's `DiskStateQueue` spills its FIFO queue, while dive's
ordered parallel source waves require indexed access with stable concrete IDs.
The existing lossless value codec preserves representation, token identity and
variable order; canonical exact-store encodings cannot replace it here.

## Decision

Add an opt-in `--frontier-memory-bytes N` safety queue, independently budgeting
the current and next encoded frontiers. Zero forces disk. Omitting the option
keeps the existing value-vector reference path. `--frontier-directory DIR`
selects the parent for owned transient directories. The same option works in
both profiles and stores; its TLC counterpart is TLC's disk state queue, not a
Java checkpoint wire format. No dependency edges or third-party dependencies
change.

Each queue retains encoded bytes and offsets only while their actual vector
capacities fit its budget. Once spilled it releases both vectors. Two files
carry version magic (`DIVEFQ01`, `DIVEFI01`), fixed-size disk offsets, and records
containing insertion ordinal, concrete predecessor ID, payload length and an
FNV-1a checksum of ordinal/ID/payload. The checksum detects accidental
corruption, not malicious modification. Readers open independent handles and
read one record, preserving the source-wave and ordered commit algorithms.
The native format is transient and has no recovery or compatibility promise.

A separate 64 MiB record workspace cap and 65,536-node value admission limit
bound codec traversal before allocation. The admission walk neither evaluates
nor normalizes values. Evaluator-driven deferred values, including nested or
cached ones, refuse spill mode by name. This prevents a concrete state hidden
behind VIEW from gaining evaluation effects merely because storage changed.
Callback-free lazy value representations use the lossless codec. The guarantee
is no additional evaluation by admission/encoding; existing tuple/record
constructors hash their children before the queue receives them. Controls
isolate those construction effects from the admission call.

Report the configured budget, peak aggregate resident encoded capacities,
resident bytes after queue cleanup and confirmed completed file writes.
Filesystem errors mark accounting incomplete: a failed partial write can leave
additional unmeasured bytes. Explicit close happens before result certification;
read/write/corruption and cleanup failures cannot report a completed successful
search. Resource stops preserve actual worker execution and partial counts.

## Consequences

Tiny budgets preserve counts, ordered stops, Print effects, concrete traces and
independent witness validation across both profiles/stores and 1/4/8 workers
with chaos scheduling. Storage failure controls exercise corrupt/truncated
records, invalid offsets/lengths/version, missing files, write failures and
cleanup failures.

This bounds retained **encoded frontier storage only**. Peak current-plus-next
storage is at most twice the per-queue budget. One-record codec buffers,
decoded states, per-source candidates, winner vectors, seen/exact stores,
predecessor logs and captured graphs remain outside the budget. This is not a
bounded-total-memory checker. Frontier spill with temporal graph capture is
currently refused; graph/SCC spill and source-candidate spill need separate
work. Physical I/O failures are explicit, so they do not claim TLC-equivalent
completed verdicts. Defaults and semantic search limits remain unchanged.
