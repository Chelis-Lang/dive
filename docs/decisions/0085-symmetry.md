# 0085: Symmetry, current decisions

- **Status:** accepted
- **Date:** 2026-10-12

## Context

ADR 0019 defines symmetry reduction: canonical keys, TLC's group construction,
concrete first occurrences and symmetry with liveness. The owner later answered
its open questions (open questions 15–17, divergences D-005, D-015 and D-016).
Later ADRs extended symmetry to storage and recovery routes. This ADR records the
current decisions in one place. It refines ADR 0019 and does not replace it.

## Decision

**Reduction.** Symmetry is applied only through the configured dedup-key path.
dive explores, checks and reports the concrete first occurrence, and validates
counterexamples on concrete states. Distinct and generated counts must match TLC
at 1 worker. Under `--compat=tlc`, the canonical representative and the
fingerprint are TLC's. In the default profile, any representative of the same
orbit is acceptable, because counts depend only on the partition.

**One key path for every route.** The resident store, the disk seen set,
fallible invariant-error replay and checkpoint restore all compute symmetry and
VIEW keys through the same configured live-search path. Checkpoint restore
recomputes each saved key and checks that it is unique. It does not trust stored
keys. No storage route may introduce its own canonicalization.

**Declarations.** A non-bijective symmetry function is rejected by default, with
a diagnostic that names it (D-015). `--compat=tlc` accepts it, as TLC does.
`--check-symmetry` is an opt-in audit of whether the declared symmetry really is
a symmetry of the spec (D-016, planned).

**Liveness.** The intended default is `--symmetry-liveness=unreduced`: disable
reduction for the whole liveness run and say so (D-005). `refuse` and TLC's
quotient-graph behaviour (`tlc`, selected by `--compat=tlc`) are explicit modes.
Until these modes are implemented, symmetry combined with temporal properties is
refused by name.

**Other modes.** DFID, simulation with randomness, and unsupported value or
startup shapes refuse symmetry by name until they are supported.

## Consequences

- Symmetry parity is judged on counts, verdicts and validated counterexamples,
  not on which orbit member TLC printed, except under `--compat=tlc` where the
  representative is TLC's.
- Making the sort-based canonical form a fast path (ADR 0019 §2) remains a
  result-preserving performance item for the default profile.

Supersedes: none. Refines 0019. The symmetry parts of 0038, 0068 and 0072 are
carried here; those ADRs are consolidated in 0083.
