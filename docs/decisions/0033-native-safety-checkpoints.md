# ADR 0033: Native completed-level safety checkpoints

- Status: accepted for the bounded first slice; broader recovery remains pending
- Date: 2026-10-10
- Tracking: issue #152

## Context

Pinned TLC persists its queue, predecessor trace, fingerprint set and ordered
intern table. Recovery skips Init. Generated counters are not persisted: they
restart from the recovered distinct count. Our pinned pause/full/recover probes
record 2,843,484/5,123,201/3,703,060 generated states respectively. The paused
search has 1,511 queued states despite TLC's timed developer-stop output calling
the graph complete. The raw evidence and source hashes live in
`conformance/probes/native-checkpoint/`.

A native snapshot needs an actual resume path, concrete predecessor identity,
ordered names restored before evaluation, explicit incomplete status, and
admission that prevents malformed archives from producing model verdicts.
Native snapshot bytes are not Java checkpoint bytes or independent certificates.

## Decision

Add a separate `dive_check::checkpoint::check` request/result and
`dive_api::check_file_checkpoint` entry. CLI callers use `--checkpoint FILE
--checkpoint-after-levels N` or `--recover FILE`, exclusively. Zero pauses after
initial enumeration; N pauses after N complete expansion levels, before the next
frontier expands. A pause is a resource stop, never exhaustive success.

The first slice accepts pure concrete INIT/NEXT safety, runs inline with one
actual worker, and explicitly refuses temporal graphs, VIEW, SYMMETRY, runtime
names/effects/checker reads, evaluator-driven deferred state values and transient storage modes.
Requested width remains reported separately. Recovery does not enumerate Init:
it restores visited keys, discovery IDs/ordinals and the ordered concrete
frontier, then continues the existing safety engine. Existing witness reconstruction may
still replay saved initial/successor ordinals; it does not seed the recovered queue.

Both profiles default to TLC's generated/initial counter reset.
`--checkpoint-counters retain` explicitly preserves original totals; a TLC run
using it is labelled `tlc+overrides`. Making retain the default is an open owner
question. Counter policy is reported for pauses and resumed runs.

The native schema pins executable SHA-256, semantic/oracle/schema identity,
complete serialized IR, complete config binding, exact config source bytes for
file requests, profile, store, seed, fingerprint polynomial and evaluation
settings. Recovery imports the ordered intern table before binding, checks its
startup prefix without callbacks, and verifies the final table before assumptions.

The archive is a single immutable file: magic, payload length and SHA-256, then
versioned JSON containing lossless native state bytes, keys, predecessor records,
frontier IDs, ordered names and counters. A same-directory private staging file
is synced before an atomic no-replace hard-link commit, followed by staging
cleanup and directory sync. Existing archives are never overwritten. Failed
commit/cleanup/sync remains an explicit failure, even when a complete archive
has become visible. Recovery requires the exact complete length and checksum.
SHA-256 detects accidental corruption; it does not authenticate a rewritten file.

Admission bounds are independent: 64 MiB payload, 65,536 discovered states and
names, 1 MiB per lossless state/key/name, 1,024 variables, 65,536 levels, and
bounded value shape/node/depth inspection before decoding. Array enqueueing is
bounded; inconsistent counts, parents, ordinals, frontier suffixes, tokens, keys
or executable/model identity refuse. Every decoded discovery must reconstruct its
key and a unique visited entry. This is trusted-local recovery, not a proof that
a hostile author selected a complete reachable state set.

## Consequences

The snapshot retains concrete discoveries and codec workspaces in memory; its
archive bounds are not a total search-memory limit. Actual queue/store/trace
spilling, parallel recovery, graph/liveness recovery, runtime streams/effects,
VIEW/SYMMETRY and distributed recovery remain separate increments. A later
schema must explicitly add and validate each interaction. Java import is deferred.

Ordinary checking is unchanged. Both stores/profiles, completed-level boundaries,
worker requests and chaos scheduling compare with uninterrupted safety runs.
Violation witnesses are checked independently, and malformed/checksummed archive
controls cannot obtain recovery receipts or successful verdicts.

Native generated/initial counters are bounded by the largest positive TLC `long`
(9,223,372,036,854,775,807). Imported larger values refuse before binding;
continuation uses checked increments and stops explicitly before exceeding that
schema bound. Wrapped counts can never become a completed-search report.
