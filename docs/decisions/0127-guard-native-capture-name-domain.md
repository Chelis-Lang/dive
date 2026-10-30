# 0127: Guard the actual native temporal name domain

- **Status:** accepted
- **Date:** 2026-10-30

## Context

The original pinned Examples `EWD998Chan` (commit `6a7b26a3`) at N=1,
with unchanged properties, succeeds with 6 distinct / 13 generated / depth2
on the reference and native engines except native TLC/8, which refuses before
search because the reference concrete-pool source inventory includes SubSeq
and concatenation. Pure tuple calls do not create strings, but a complete
static parameter/state/callback type analysis is not available.

Before implementation, pinned TLC `94d0c50` (jar SHA256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`),
W1/seed0/fp0, confirms a minimal tuple SubSeq cycle succeeds at 3 distinct /
4 generated / depth3; a string SubSeq cycle succeeds at 4/5/4. The latter is
not new native string support: its existing native demand remains unsupported.
The conservative reference-pool inventory must remain unchanged.

## Decision

A private freshly bound native producer may discharge only the typed
runtime-name-only sequential restriction, retaining every source/configured/
captured-effect and other capture restriction. Verify actual model scope
ownership and exact startup token/text correspondence to the actual sealed
native Store. The native table refuses every new name. Its bound reference
Interner is append-only: length increases on each newly interned text and entries are never removed.
The admitted startup tokens are checked exactly against the native domain; the
guard relies on monotone length, not later token assignment. Observe that exact Arc across the entire temporal
operation, including planning, capture, periodic/final analysis, attribution,
error reconstruction and independent report validation. Any growth prevents a
semantic result from escaping; actual resource/interruption/bug transport is
preserved. The guard is not a verdict, state-shape fact or independent word proof.

The private Prepared constructor accepts source IR/configuration, never a public
bound model or Rust callable/provider values. Binding constructs a fresh evaluator;
scopes validate against that Core/IR/Interner, and subsequently constructed
callbacks/lazy providers retain that Core. GraphCheck borrows that same model.
No foreign captured scope is admitted; no reference pool scheduling policy or
new closure/provider capability is introduced. Unknown source types may execute
only through existing runtime semantics and guards, with explicit unsupported
on unproved demanded operations. Native exact-original retention follows ADR0121.

## Consequences

This removes a false static refusal for pure tuple paths without implementing
runtime name encounter ordering or a second type system. Source expressions
that actually create reference-domain names during planning or analysis refuse,
even if an intermediate atom or candidate looked valid. The default profile
continues its existing text ordering. Effects, redirects, resource limits,
checked comparisons, original representations and independent word obligations
retain their separate acceptance boundaries. This establishes no capacity,
performance, full-corpus or TLC-parity qualification for EWD998Chan N=3.
