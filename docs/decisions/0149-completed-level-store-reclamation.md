# 0149: Completed-level owned Store reclamation

- **Status:** accepted
- **Date:** 2026-11-04

## Context

Candidate, frontier and predecessor spill (ADRs 0083 and 0143) leaves the
append-only accepted-value Store live. Resident exact membership also retains
canonical Handle rows, while ordinary safety traces already reconstruct from
the predecessor forest and concrete successor ordinals. Copying a frontier
without replacing every old Store owner therefore cannot reclaim expanded
payloads. Compiled plans contain startup handles whose coordinates must remain
valid, and actual raw/normalized construction ownership cannot be recreated from
extensional equality or header flags (ADRs 0131 and 0141).

## Decision

Explicit pure Core safety BFS admits the optional `store_compact_bytes` field,
exposed as `--store-compact-bytes N`. This is a completed-level trigger on the
Store's measured retained-byte lower bound, not a hard live-memory or RSS cap.
Zero forces every completed boundary, including the post-Init boundary. Omission
keeps the resident reference in either profile and is the TLC-equivalent setting.

The owned explorer captures the compiled startup as fixed owned bytes before
search. At a joined level boundary it constructs a fresh Store with precisely
that issued prefix, preserving compiled coordinates, semantic policy, printer
context and the complete encountered name-token order. It copies only live next
frontier roots and actual registered raw/normalized history companions. It
remaps the canonical/original rows and sparse construction views separately,
retaining node/origin aliases, raw order/duplicates and representation classes.
No normalization demand or evaluator-value round trip supplies authority.

Every Machine is replaced at the same boundary, releasing Arena, frame,
interning-table, materialization-cache and comparison-certificate owners.
Memo values/certificates are cleared while statistics, warning order and
evaluation counters remain cumulative. A spilled next frontier is streamed one
paired row at a time into a fresh codec bound to the replacement Store. The old
current/next files are audited and closed before replacement. Read, write,
corruption, cancellation and cleanup failures cannot commit a partial frontier
or certify success.

For this opt-in route, resident exact membership retains the existing closed
mathematical key bytes, independently of Store coordinates. Full key comparison
resolves collisions, including zero hashes. The ordinary resident exact path is
unchanged. Symbolic/deferred state keys outside that encoding refuse explicitly;
disk membership keeps its existing fallible key path and actual one-worker
policy. Predecessor records preserve discovery ids and generation ordinals;
the existing replay reconstructs concrete safety words after reclamation.

Identity-checked Core recovery may change this storage policy. The permanent
prefix is captured from fresh compilation before restoration. Imported expanded
rows are released after membership/frontier reconstruction, and recovery-only
sessions retain no old Store owner. Checkpoint pause, temporal capture, runtime
effects, sampling, Reference and Auto refuse the new policy by name. Accepted
discovery residency, temporal paging/recovery and native distribution remain
required separate work; this decision does not defer them.

## Consequences

Expanded accepted payloads that have no live root can now be released during
real checking and recovered checking. The caller-owned Rust entrypoint updates
its Store owner; external clones intentionally keep their old allocations alive.
Known bytes before/after collections, reclaimed nodes and collection counts are
reported separately from ordinary final Store snapshots and sampled RSS.

The live frontier, permanent prefix and its bytes, all encountered names, seen
keys, predecessor records, selected trace/replay values, mapping workspaces,
temporary overlap of the old/new Stores, allocator overhead and process memory
are not bounded by the trigger. Large live roots are always retained. Existing
frontier/candidate/predecessor/disk budgets and whole-run controls remain
independent. No total-memory, original temporal-capacity or performance
acceptance follows from this feature.
