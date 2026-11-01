# 0083: Storage, checkpoint, recovery and DFID

- **Status:** accepted
- **Date:** 2026-10-12

## Context

TLC offers a disk state queue, a disk fingerprint set, checkpoint and recovery,
and a depth-first iterative-deepening mode (`-dfid`). dive implements these as
functional features. Earlier ADRs added them one increment at a time, each with
its own admission and receipt scaffolding. This ADR states the decisions that
remain, in their current form. The rescue reset (ADR 0086) keeps these features
and simplifies them.

## Decision

**Defaults.** In-memory storage is the default and the reference path in both
profiles. Every disk route is opt-in, and none of them may change enumeration
order, stop selection, counts or counterexamples. A resource stop is never
reported as exhaustive success.

**Frontier spill.** `--frontier-memory-bytes N` budgets the encoded current and
next frontiers, and `0` forces disk. `--frontier-directory DIR` chooses the
parent directory for owned transient files. The TLC counterpart is TLC's disk
state queue. The files are transient and carry no recovery or compatibility
promise.

The native core's safety implementation uses the same ordered checksummed
transient queue with paired canonical/original handle rows. A narrow codec
retains the live append-only Store and issues a private nonreused process token;
it does not expose a raw handle constructor or serialize Store/history for
restart. It decodes at most 1,024 state pairs per chunk, outside the encoded
queue capacity. Full retained-record validation precedes certification and
cooperates with the actual run owner. Already stopped runs close/clean promptly
without validating unprocessed records. This implements the existing transient
scope; core disk membership, temporal storage and recovery remain separate.

**Predecessor log.** `--predecessor-memory-bytes N` and
`--predecessor-directory DIR` move the parent/ordinal log to a fixed-record
transient file. A directory without a budget is rejected before any work. This is
not a total memory bound and not a checkpoint.

**Disk seen set.** `dive_store::seen::DiskSeen` is a fallible on-disk seen-state
store, with explicit cleanup and first-failure poisoning. It is opt-in through
`--seen-disk-entries N`. A storage failure stops the run with its first cause and
is never treated as absent, duplicate or success. The disk route runs inline and
reports the requested width separately. It admits INIT/NEXT and pure safety roles.
Temporal roles, checkpointing and DFID refuse by name. Invariant-error replay on
this route uses the same fallible membership.

**Formats.** Transient and persisted files have a version magic and per-record
checksums. Checksums detect accidental corruption, not tampering. Structural
storage format 2 keeps literal tuples and tuple functions distinct. Old version-1
containers are refused explicitly rather than read lossily. Canonical keys and
fingerprints did not change.

**Checkpoint and recovery.** `--checkpoint FILE --checkpoint-after-levels N`
pauses after completed BFS levels, and `--recover FILE` resumes. The archive is
one immutable, atomically committed file. It pins the executable, model, config
and option identity, and recovery refuses on any mismatch. Recovery restores the
visited keys, discovery IDs and ordered frontier, and does not re-enumerate Init.
Supported VIEW and symmetry keys are recomputed through the configured key path
on restore. TLC's counter reset is the default.
`--checkpoint-counters retain` is an explicit override. Archives are trusted
local inputs, not reachability certificates.

**Trace transport.** Embedded traces use version 1. The opt-in stored-value
trace representation was withdrawn by stage V (ADR 0016 amendment 3). This is
separate from the structural storage codec used by spill and checkpoint files.

**DFID.** `dive dfid --max-level N` (TLC: `-dfid N`) runs a native iterative-deepening engine in
`dive-check`. It shares binding, evaluation, the state codec and fingerprints
with BFS. A depth-bound stop is reported as incomplete (`exhaustive: false`).
`--compat=tlc` keeps TLC's exit status. dive's default derives replayable worker
streams from the reported seed. Every claimed violation is validated
independently. Unimplemented combinations (liveness, checkpointing, disk storage)
refuse by name. TLC's per-iteration worker random order is not an emulation
target. Parity is the observable result.

## Consequences

- Storage options are orthogonal to semantics. The conformance gate can run any
  of them against the same goldens.
- Receipts, epoch indexes and admission proofs that earlier increments attached
  to these routes are not product commitments, and may be simplified.

Supersedes: 0021, 0024, 0031, 0033, 0038, 0055, 0060, 0063, 0068, 0072, 0075.
