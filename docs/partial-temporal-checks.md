# Inspect bounded partial temporal checks

The Rust diagnostic under #185 constructs actual ordered Phase-L records from a
bound model, then checks written product records after initial registrations
and entire source commits. Its public entry is
`dive_liveness::phase_execution::execute_partial`. The bounded delivery passed
the full 35-stage workspace gate (965 tests, zero skips), the standalone Cargo
example, and all 21 CM1 plus 16 validation-smoke models at 1/4/8 workers.
Ordinary `dive check --compat=tlc` periodic liveness remains
refused. Explicit final-only checking remains a separate mode.

A partial check can find a candidate counterexample. It cannot return `Holds`,
completed temporal exploration or a model certificate. No candidate may be used
as a violation proof until independently replayed against a freshly bound spec.
A check with no candidate says only that this invocation found none among the
written records; subsequent graph growth can reveal one. The diagnostic
continues after candidates deliberately to expose those later changes, unlike
TLC's ordinary deciding-stop path.

The producer accepts a bound `Model`, checking its IR/name/context authority
before planning and worker callbacks. Callers cannot provide graph nodes, a
done mask or a claimed validation flag to create source-produced checks. The
separate supplied-graph `check_partial` and `select_partial` entries grant only
diagnostic projection results. An unwritten record with outgoing adjacency is
rejected; incoming edges to an unfinished record remain legal and are skipped.
This also prevents a caller-invented unwritten path from shortening a witness.
For a null-tableau group, the producer registers an unfinished placeholder only
when a real stored successor slot first mentions that fingerprint. It retains
the original source index/fingerprint, actual table slot, slot-vector position
and generator ordinal (absent for implicit stutter). The original vertex and
shared work quotas are charged before retention. Placeholder adjacency remains
empty until an actual source write; final/full execution is unchanged. Registered
placeholder/product counts do not establish TLC node-pointer-table size, growth
thresholds or physical disk bytes. Those schedule authorities still need their
own source-produced evidence.

Requested/actual pool widths, original concrete graph/safety counts and logical
source prefixes remain separate from graph and SCC work. Partial projection,
retention and completed selection work share the single existing `graph.steps`
ceiling with Observer graph insertion/recursive inspections. Phase-L predicate
and value budgets remain separately bounded and unchanged. Failed selection
work can be unknown; a retained charge is not presented as its exact total.
These quotas are entry/work ceilings, not aggregate memory or byte limits.

The current scope is pure, bounded, admitted source contexts; opaque/effectful,
malformed ownership and unavailable concrete witness recovery remain named
failures. No new result-changing default, clock schedule, exact-store proof,
VIEW/symmetry recovery or host authority is introduced.

See [ADR0048](decisions/0048-bounded-partial-temporal-scc.md) and the
[complete source/capture family](../conformance/probes/tlc-partial-scc/README.md).
The next core step must integrate real progress-clock/quiescence decisions and
TLC's threshold/runtime/force/error/checkpoint order before admitting ordinary
strict temporal mode; it cannot silently substitute final-only or short lassos.

## Rust usage

The [runnable example](../conformance/probes/tlc-partial-scc/user-example.rs)
loads the real `PartialGrowthSafe` source and configuration, binds with
`CheckOptions::tlc(0, 0)` at four workers, and calls `execute_partial`. It asserts
actual worker width, no candidate, shared accounting and `model_proof() == false`.
This is a successful diagnostic, not a proof that the temporal property holds.

For a standalone caller checked out alongside this repository, use edition2024
and path dependencies `dive-sema`, `dive-ir`, `dive-config`, `dive-model`,
`dive-check` and `dive-liveness` from their corresponding `crates/dive-*` paths.
For example, with a caller directory at `examples/partial-caller`:

```toml
[package]
name = "partial-caller"
version = "0.1.0"
edition = "2024"

[dependencies]
dive-sema = { path = "../../crates/dive-sema" }
dive-ir = { path = "../../crates/dive-ir" }
dive-config = { path = "../../crates/dive-config" }
dive-model = { path = "../../crates/dive-model" }
dive-check = { path = "../../crates/dive-check" }
dive-liveness = { path = "../../crates/dive-liveness" }
```

Copy the linked example to that caller's `src/main.rs`, then run
`cargo run --manifest-path examples/partial-caller/Cargo.toml` from the
repository root so its relative model path resolves. Keep this standalone
package outside the workspace members (or add an empty `[workspace]`).
No CLI flag exposes this diagnostic as ordinary model checking.

`PartialExecution::checks()` retains each original OOS/PEM, logical committed
source count, original registered product identities, actual write bits,
source-slot origins and SCC/candidate projection. `inspections()` is the charged
partial-work subtotal; `graph_inspections()` includes original insertion work
under the one graph quota. A `PartialFailure` retains previous finished checks
and the original deciding execution failure, actual statistics/worker facts and
bounded source prefix. Failed-stage work can be unknown. No callback-provided
proof bit, supplied graph or absence of an accepting component becomes `Holds`.

A candidate still needs fresh model/word validation, including its closing
edge, original property claim and a JSON replay. The API deliberately does not
package it as a certified `Violation`. All current reference prefix/witness
fields are compared only in the documented representation: Java `getPath`
exposes concrete fingerprint-only prefixes; native product identities remain
separate, never inferred from that vector. Exact temporal storage, clock timing,
quiescence, deciding-stop counters and ordinary schedule certification remain
pending under #66; this bounded diagnostic increment is tracked by #185.
