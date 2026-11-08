# 0141: Owned Core safety recovery at completed levels

- **Status:** accepted
- **Date:** 2026-11-02

## Context

ADR 0083 supplies fallible membership, transient queue/predecessor storage and
bounded trusted local recovery. Its Reference implementation serializes reference
values. Core instead evaluates owned handles with raw/normalized construction
history (ADRs 0117 and 0130). Converting those handles to reference values loses
receiver and history authority; treating transient frontier tokens as persistent
identity would bind a restored row to another live owner. A usable increment must
pause and resume actual safety checking, including existing VIEW and safety
symmetry, rather than leave an unused serialization layer.

## Decision

Explicit Core checkpoints use a completed-level transaction. Init enumeration or
an entire ordered BFS source level finishes before a pause. Checkpoint-enabled
runs retain every discovered concrete canonical/original row, its key, parent and
successor ordinal. Ordinary Core does not retain these archive rows. Resume rebuilds
membership and the predecessor forest in discovery order, restores the saved
frontier and source level, and continues without enumerating Init as search work.
Membership uses the same VIEW/symmetry path and fallible resident/exact/disk adapter
as uninterrupted execution. Counter reset/retain remains ADR 0083's existing policy.

`DIVECC01` identifies the Core backend. Its existing checksummed, bounded,
atomic-no-replace envelope contains four length-delimited sections: source and
semantic identity, compiled startup Store image, completed Store image, and ordered
search image. Reference `DIVECP02` is unchanged. Identity pins executable, complete
source/config bytes and bindings, local override classes, seed/fingerprint,
semantic options and Core strategy switches. Worker width, transient directories
and storage/memory budgets may change. Auto pause remains a static whole-Reference
selection; Auto recovery selects the encoded backend before evaluation. Explicit
backend mismatches reject and a reached native refusal never retries recovery.

Store format `DIVEST01` preserves issued coordinates, name tokens, node payloads
and the exact registered raw/normalized origin table. It serializes child-first
owned DAG nodes and validates bounds, node shapes, coordinates, ownership and
origin phase. Fresh compilation must produce an identical startup prefix. Restored
nodes extend that checked prefix; process-local frontier codec tokens, filenames,
arenas, caches and runtime hosts are never serialized or reused. Fresh private
frontier files bind restored coordinates to a newly checked owner. No reference
`Value` conversion or authority inferred solely from history flags is permitted.
New evaluator-driven deferred providers after startup refuse explicitly.
Amendment (#791): the raw/normalized origin table is retired. `DIVEST01` keeps
its layout with an always-empty table; a nonempty table or a history-flagged
node is refused as legacy construction history.

The initial workflow admits currently supported pure Core safety, fingerprint or
exact membership, VIEW, safety symmetry, requested worker widths and optional
frontier/predecessor spill and fallible disk membership (actual one worker).
Temporal graphs/SCCs, runtime effects, distribution and unrepresented deferred
providers remain explicit unsupported results. Disk files themselves remain
transient: a checkpoint reconstructs keys from the saved concrete discoveries,
without serializing a second growing collection of disk keys.

## Consequences

A snapshot is trusted local continuation data, never independent reachability or
property evidence. Model verdicts still require exhaustive checking and actual
reported words still undergo independent validation. Corruption, identity,
filesystem, cancellation and cleanup failures cannot certify success. Checksums
provide integrity, not authenticity against deliberate rewriting.

Archive bounds (64 MiB aggregate payload, 65,536 discoveries/source level limit)
are admission bounds, not a process memory cap. DAG, parsing and codec workspaces
and ordinary Store/search memory remain separate. A recovery-only request releases
expanded discovery rows after membership reconstruction and may continue beyond
the input archive's discovery limit under ordinary configured search budgets;
it does not implicitly write a second checkpoint. Backend formats are private,
versioned and tied to executable identity, rather than a portable value protocol.
