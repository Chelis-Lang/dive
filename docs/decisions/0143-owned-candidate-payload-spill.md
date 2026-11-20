# 0143: Owned candidate payload spill without speculative Store promotion

- **Status:** accepted
- **Date:** 2026-11-03

## Context

ADR 0083's frontier and predecessor budgets do not bound the parallel expansion
of one high-fanout source. Interning every emitted compound into the shared Store
before spilling its handle preserves that growing payload allocation. Core also
retains receiver classes and actual constructor history; reference-value encoding
or inferring history authority from flags would lose observable semantics.

## Decision

Explicit Core pure safety BFS has optional `candidate_memory_bytes` and
`candidate_directory` policies, exposed as `--candidate-memory-bytes` and
`--candidate-directory`. Omission preserves the resident path in both profiles;
zero forces transient files. A directory requires a budget. These are
result-preserving storage choices; the TLC-equivalent setting is omission.
Reference/Auto, runtime effects and temporal capture refuse the new policy by
name until their whole routes are implemented. Existing Core safety disk seen,
exact keys, VIEW, safety symmetry, frontier/predecessor spill and completed-level
recovery remain compatible with the candidate policy.

Each worker retains at most one source's encoded occurrence queue before ordered
commit. The queue budget includes occurrence metadata and owned compound payloads,
not just handles or an in-memory index. Existing ordered checksummed transient
records provide spill and fallible cleanup. Every unconsumed queue closes on all
exits; cleanup failures reach the run owner and invalidate certification.

The issuing Arena owns speculative values. Identity closure performs the same
normalization demands as ordinary interning, but produces local closed roots.
Explicit `Heap::intern` retains its persistent contract. Speculative expansion
bypasses evaluator memo sites, including primed step sites, before interning any
key/result, and bypasses persistent materialization-cache insertion. No generator
restart or Arena reset occurs at emission: live frames, thunks and cursors keep
their roots until the generator's existing rollback boundary.

A private live-Store-bound codec emits child-first owned DAGs before rollback.
Persistent subtrees remain checked references to that retained Store; transient
nodes use spool-local references. Actual local constructor ownership supplies
raw/normalized origin tokens. Logical normalization already performed may be
reconstituted after rollback, but encoding introduces no new evaluation demand.
Ordered reads alone import nodes and register their constructor origins. Tokens
cannot be decoded by another codec, process or Store. This is trusted transient
working storage, not a portable checkpoint or independent trace certificate.

A record is limited to 64 MiB and its traversal depth to 128. One identity
closure retains at most 65,536 successful source-handle mappings and 64 MiB of
live child-copy capacity. This per-operation map preserves DAG sharing and is
released before the next closure; already-closed roots are reused. A flags-only
canonical projection shares the original immutable Arena payload through a new
header, respecting both pooled and boxed rollback boundaries.

The decoder's cross-record constructor-origin index is separate codec storage,
not Store payload. Its sorted-vector capacity is bounded by the smaller of the
record limit and 1 MiB per pending source. Exhaustion reports named
`DIVE-S0202` resource-limit, closes owned storage and certifies no word. The index
is retained only until that source's ordered consumption ends, so repeated
aliases still resolve to their actual shared origin. Peak/current index capacity
is reported separately from encoded queue capacity. The per-record traversal
and deduplication workspaces also have explicit bounds derived from the record
limit. One-record codec/writer/reader workspaces are separate from queued encoded
capacity. Active evaluator Arena roots, dynamic names, accepted Store/history,
other caches, seen sets and allocator/OS overhead are outside this component
budget. Existing whole-run resource controls still apply. No total-RSS bound or
complete state/temporal paging follows from this feature.

## Consequences

Ordinary resident execution keeps its existing representation and persistent
interning semantics. The opt-in path trades memo reuse and larger source chunks
for bounded pending output, while preserving counts, ordered events, exact
identities, original values and independently validated words. Corpus and fault
controls compare both profiles and stores at actual widths 1/4/8. High-fanout
controls additionally verify flat shared compound Store growth during emission,
encoded queue capacity bounded independently of fanout, and Store growth only
when ordered consumption imports payloads.

Amendment (#1038): canonical candidates need no constructor-origin index, so the
index and its report fields (`origin_index_bytes`, `peak_origin_index_bytes`,
`origin_index_bytes_limit_per_worker`) are removed. Records use format
`DIVECD02` without the per-node origin word; `DIVECD01` is refused by name.
