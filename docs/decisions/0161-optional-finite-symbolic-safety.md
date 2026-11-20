# 0161. Optional finite symbolic safety evidence

- **Status:** accepted
- **Date:** 2026-11-20

## Context

A finite protocol can have many states with the same local structure. On the
pinned KeyValueStore Large configuration, a decision diagram represents the
complete reachable set in 1,961 nodes. Explicit Core visits 594,616,815
distinct states. The source binder, golden results and independent trace
validator already supply cross-checks. [Measurements](../developer/large-models.md).

## Decision

Expose a separate source-checkout tool for the finite fragment. A Rust helper
exports the resolved roles and values. Python compiles them into CUDD Boolean
functions. Optional dependencies live in the tool's virtual environment.
Admission checks total Init and updates against the proposed finite layout.
Symmetry counting requires equivariance and a complete supported group.

A completed search reports counts and breadth-first depth. A shortest
violating trace requires independent replay. Property edits may reuse a
completed closure only after comparing the compiled relation and concrete
encoding. A transferred state predicate is an untrusted induction hypothesis.
The receiver establishes Init inclusion, action preservation and implication
of all requested safety obligations, including deadlock freedom when enabled.

## Consequences

Induction can establish safety without visiting states or reporting a
diameter. A failed induction obligation supplies no counterexample.
Symbolic frontier order does not establish a Core prefix count at the first
violating state. The tool exposes these evidence choices in its own report
format. The ordinary Core entry point and its budget and report contracts
continue to apply to `dive check`.
