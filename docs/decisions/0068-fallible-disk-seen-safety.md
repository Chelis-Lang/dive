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
