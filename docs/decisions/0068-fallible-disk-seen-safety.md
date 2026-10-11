# 0068: Fallible disk membership in ordered safety BFS

- **Status:** accepted
- **Date:** 2026-10-11

## Context

ADR0063 supplies an actual fallible disk fingerprint/exact library. Boolean
`StateSet`/`Candidates` operations cannot express I/O failure. The committed
source-prefix transaction of ADR0062 may physically resolve winners before
later ordered deciding events; substituting fallible disk writes into that bulk
resolve would let a future storage error outrank an earlier violation.

A first safety increment can use the existing inline enumerator/sink transaction.
Its key admission happens before predecessor recording and the current state's
invariant, and the sink stops before any later successor's key write. Original
memory results are captured before implementation. Their producer and the library
receipt remain separate from the new engine's source and runtime evidence.

## Decision

Add `CheckOptions::seen_disk` and `CheckRequest::seen_disk`, with CLI opt-in
`--seen-disk-entries N`. The original resident path remains default, including
under `--compat=tlc`. The disk route borrows full unchanged keys from the existing
fingerprint/exact keyer and uses `Result` insertion directly. It never implements
`StateSet` for `DiskSeen`, mirrors keys into the resident set or resolves future
candidates in bulk. Key admission counts are distinct from completed physical
records and subsequent predecessor/frontier publication.

Disk runs execute inline, report requested widths separately, and explain that
immutable concurrent disk read views remain unproved. Existing profile, seed,
key transformation, concrete predecessor, output and counter contracts remain
unchanged. Chaos cannot silently advertise parallel execution on this route.

Validate malformed policy and unsupported option combinations before binding or
owning files. Only explicit `INIT`/`NEXT` roles are admitted. Configured
`PROPERTY` and `SPECIFICATION` roles refuse before binding, including safety-only
`SPECIFICATION`; callback-free role decomposition is a separate follow-up.
Graph/source capture, temporal/property roles, checkpoint/recovery
and DFID are refused; simulation exposes no disk route. Binding and assumptions
of an admitted ordinary request precede actual disk creation, so a filesystem
creation error may retain their output but reports no Init/search work.

Required fingerprint collision metadata remains fallible. The actual-gap scan
runs only at the existing TLC optimistic threshold. A pair-work refusal stops
before postconditions; metadata is never silently omitted to report success.
Exact mode retains its existing absence of probabilistic fingerprint estimates.
Explicit audited close occurs after witness reconstruction and predecessor
cleanup. A close error prevents completion, retains the earlier deciding event,
first storage cause and cleanup truth, and leaves any concrete trace as diagnostic
evidence of an incomplete resource-limit result rather than full agreement.

## Bounds and failures

The policy caps record key bytes, physical entries, probe steps and summary pair
comparisons. There is no resident key/offset collection. At most the library's
two record-sized workspaces are retained, with private files/handles and page
cache outside that statement. Existing caller-owned canonical key computation,
state values, frontiers, predecessors, candidates, traces, graphs and native
archives remain outside it. Optional frontier/predecessor budgets keep their
separate scopes. No aggregate/process-RAM, crash-durability, restart/import or
Java checkpoint interoperability claim is made.

First typed insertion errors preserve generated count, concrete predecessor and
emitted successor ordinal. Partial bytes/staging can exist despite an insertion
not committing; expected disk lengths exclude those unknown bytes. Cleanup never
recursively removes caller files. Real file mutation controls distinguish read,
integrity and cleanup failures at fixed inline positions. Library write/rehash/
allocation poison controls retain their own producer; synthetic summary counters
are seam tests, not fabricated oracle-generated states.

## Consequences

This is actual bounded safety engine storage, not full P7/F-031 certification.
Parallel read views, candidate error ordering, temporal/capture integration,
disk checkpoint restoration and externally sorted collision summaries remain
future increments. The crate graph and independent validator boundary do not
change. `dive-trace` continues to validate concrete words without disk/checker
state. User commands, Rust example and current limits are maintained in
[the safety guide](../user/disk-seen-safety.md).


## Original guarded invariant-error replay boundary (2026-10-11)

The immutable first engine epoch exposed a separate replay authority gap:
`ReplayMembership` gives 3 generated/3 distinct/1 queued in clean TLC and the
resident counterpart, but 4 distinct/2 queued when replay reads the empty
resident reference on a disk run. Preserve the failed source/raw/gate epoch.
The first guarded disk epoch refused invariant-evaluation call-stack replay by name,
retaining the original error in notes and the original stopping prefix/diagnostic
word. Memory replay is unchanged. Positioned fallible replay reads and exact
TLC error-counter continuation remain a future increment; no refusal earns
agreement. Missing/invalid ordinary roles preserve existing binder effect order,
separate from explicitly rejected configured role/mode combinations.

## Verification receipt (2026-10-11)

Producer `da0f857161c57b52e8cb1d6c18ef0cebb0021ea4` closed the eight-stage
local gate with 1,268 tests passing and zero skipped. The unchanged two-job,
two-test-thread run took 1,068.6 seconds; unchanged Linux partition/time limits
remain separate from this measurement. Current raw/source receipts retain all
384 original resident runs, 576 disk runs and 64 fresh resident references,
144 independently valid words and 864 rejected mutants. The independent
selection explicitly distinguishes a real VIEW violation from symmetry-only
SymOrder. Seven focused fault controls pass; the focused run filters 26 other
tests without replacing the complete zero-skip gate.

Four actual CLI and four compiled Rust-guide outputs disclose inline fallback
and cleanup. Required resident CM1/ValidationTestSuite controls pass at 1/4/8
workers; they establish no new disk-corpus or parallel-disk coverage. The first
CM1 driver invocation's absent default binary path is retained separately from
the passing explicit-binary invocation. Full selection, semantic comparisons,
error-class qualification and all immutable failure epochs remain in the
[source-bound evidence](../../conformance/probes/fallible-seen-engine/README.md).


## Positioned replay continuation (2026-10-11)

ADR0072 implements the separate fallible disk membership/insertion replay
transaction. It preserves the original source/ordinal and frozen generated
counter while accounting newly admitted keys and logical queue contributions.
Resource/bound/cleanup failures retain the first positioned cause and diagnostic
original error without completion credit. Public configured role/mode and
parallel disk restrictions above remain; original da0 verification and wrong
b07/refusal observations are not retagged as this new producer.
