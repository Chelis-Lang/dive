# 0152: Compiled source maps and producer-only native debugging

- **Status:** accepted
- **Date:** 2026-11-04

## Context

Static IR locations do not identify actual compiled execution. Inlining, aliases,
constant folding, cached bodies and generator continuations change which
instructions run and where their local values live. Debugger inspection must not
force a thunk, normalize a producer value, repeat effects or observe independent
word/diagnostic reconstruction as producer work.

## Decision

Add an explicit native `dive debug` route and public `debug::Session` around the
existing Core safety checker. `CheckRequest.debugger` is an optional capability;
None is the ordinary setting in both profiles and TLC's debugging-off setting.
TLC `-debugger` transport remains unsupported. This is a native DAP slice, not a
claim of complete TLC debugger parity.

The compiler's opt-in sidecar maps align with the actual value/generator arrays.
They retain source byte ranges, definition/flat-local associations, compilation
call contexts and active INSTANCE substitution prefixes. Reachability starts at
actual plan roots after folding; removed or aliased source expressions receive
no invented executable site. Cached body and memo entries retain their real
instructions. Runtime stacks come from actual instruction entry/exit and frame
bases, rather than replaying a source tree. Native IR origins retain JavaCC
coordinates; DAP presents one-based UTF-16 columns and exclusive end positions.

Only the primary explorer machines install the observer. Fresh replay, validation,
folding and optional analysis machines remain unobserved. Diagnostic call-stack
replay reuses a primary machine, so its entire traversal suspends the capability
with restoration on every return or unwind. It does not filter controller events
or mutate primary snapshots. Inspection copies
existing scalar/storage reads; unforced/by-name locals and compound values remain
unavailable or opaque. Evaluation accepts bounded integer/boolean expressions over
captured scalars, using a fresh existing compiler and machine with finite fuel.
It cannot call source helpers, run startup expressions or grant host access.
The source-only analyzer has no root directory and uses empty search paths, so
its fixed arithmetic imports resolve only to embedded standard modules.

The CLI exchanges real Content-Length-framed DAP over stdio or loopback TCP.
Stdio launch creates the authorized check. The listener starts a pending check
before accepting a client; attach connects to that existing checking session.
Search waits for configurationDone. Breakpoints, stepping, pause, exceptions and
termination control actual producer transitions. Handles expire on every resume.
The final checkResult event carries the ordinary report and native exit authority;
termination/refusal cannot certify an incomplete search. Existing native trace
versions and report contracts are unchanged.

The initial managed route selects one actual local worker and refuses explicit
checkpoint, temporal, sampling, native/logical coverage and host-grant combinations.
Store reclamation recreates machines without retaining observation and is refused.
Remote/distributed observation and worker preparation with a debugger capability
are refused before source or key-file access and observer preparation.
Conditional/log/column breakpoints, reverse
execution, mutation, arbitrary attach targets, compound inspection and broader
source/evaluation contexts remain open. Client paths refer only to loaded sources
or the program already authorized by the CLI. Source-map, activation, event,
expression and framing capacities fail explicitly.

## Consequences

Real framed-client controls cover multivariable helpers, actual locals, all three
forward steps, stdio launch, loopback attach, pause, termination and a freshly
independently validated invariant violation. INSTANCE/unused-effect controls and
both semantic profiles preserve the ordinary result and producer demand.
The failing-invariant replay control checks exactly one primary exception stop,
no callbacks on the replay-only successor, unchanged error/count/word authority
and an independently valid actual error word in both profiles.

Ordinary compilation allocates no source tables. Disabled observation allocates
no controller, performs no observer callback/Arc clone and preserves the original
instruction semantics. This does not establish the outstanding instruction-cost
criterion in #350. Complete maps, coverage/debugger parity, backend combinations
and performance acceptance remain unfinished. No crate dependency edge changes.
