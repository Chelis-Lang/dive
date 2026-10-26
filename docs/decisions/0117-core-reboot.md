# 0117: Rebuild the execution core for performance

- **Status:** accepted
- **Date:** 2026-10-26

## Context

dive is slower than TLC. Measured on 2026-10-26 (Apple M3, MCLamportMutex,
default profile, `--fp 0 --seed 0`; the host was noisy):

| | 1 worker | 8 workers |
|---|---|---|
| TLC | 12.0 s | 4.7 s |
| dive | 19.4 s | 10.2 s |

The 2026-10-21 x86 campaign (`docs/developer/performance.md`) measured 0.32×
TLC at 1 worker and 0.15× at 8. ADR 0006 requires at least 2× TLC (geometric
mean) at 1 and 8 workers, and at least 1× on every benchmark.

The losses are structural. The evaluator reproduces TLC's object design:

- `LazyValue` becomes `Arc<Thunk>` with two mutexes and a state-reference
  cache check (`crates/dive-eval/src/env.rs:525`).
- `Context` chains become `Arc<Frame>` and `Arc<SubstLink>` (`env.rs:381`).
- Each evaluation allocates `Arc` values (`crates/dive-value/src/value.rs:30`).
- The successor state is copied on write (`crates/dive-eval/src/next.rs`).
- INSTANCE substitutions are resolved dynamically.

The JVM's JIT and generational collector make that design cheap for TLC. In
Rust each object costs an allocation, atomic reference-count updates, a drop
walk and a free.

Measured shares of single-thread time:

| Cost | Share | Source |
|---|---|---|
| Memory management | 25–30% | 2026-10-26 profile |
| Set membership | 20–27% | ADR 0104 |
| Forcing lazy values | up to 53% | ADR 0104 |
| INSTANCE substitution | 52.6% on EWD998PCal | ADR 0104 |

The roughly eleven prototypes recorded in ADR 0104 each moved instruction
counts by 1–2% at most, or regressed. ADR 0014 §1 planned a specialized plan
interpreter. The implementation is a tree walker over `dive_ir` expressions
(`crates/dive-eval/src/eval.rs:336`). It has resolved slots, typed variable
reads and caching of unprimed substitutions, but not the specialized plan or a
value model built for Rust.

Tuning within this design cannot reach ADR 0006. The design is described in
`docs/developer/core/`.

## Decision

1. **Build a new execution core.** It consists of two new crates:
   - `dive-core`: binder, plan compiler, values, evaluator and successor
     generation.
   - `dive-explore`: the exploration engine.

   It is designed from the observable contract of ADR 0086 and the semantic
   contract in `docs/developer/core/02-semantic-contract.md`, not from TLC's
   object structure. Its foundation is a value and memory model with these
   parts:
   - 64-bit handles;
   - a hash-consed persistent store, interned only after a successor wins its
     seen-set claim;
   - per-worker transient arenas;
   - additive, incremental hashing;
   - a seen-set probe before materialization;
   - memoized pure evaluation keyed on interned inputs.
2. **Freeze the existing engine as the reference.** That engine is dive-value,
   dive-eval, dive-model, dive-check and dive-sim.
   - It gets bug fixes only.
   - It is the shadow-comparison oracle and the evaluator of the independent
     validator (dive-trace).
   - It is the whole-run fallback for features the new core does not yet
     support. The new core refuses before the run starts, and the report names
     the engine that ran.
3. **The fingerprint stops being contract.** The store key need not be TLC's
   FP64 in any profile. FP64 remains required where it is observable: per-state
   random seeding, `TLCFP` and VIEW random streams. This amends ADR 0104's rule
   to "keep every … fingerprint and successor order" and ADR 0015's
   compatibility-profile store key. Successor order remains contract.
4. **Perf gate.** For this work, ADR 0104's 5% single-thread instruction-count
   bar is replaced by wall-clock time on a quiet host at 1 and 8 workers. Every
   milestone is measured on an x86 reference node. The method is in
   `docs/developer/core/09-performance-method.md`.
5. **Proceed by gated milestones.** A two-week spike (M0) must reach these
   targets against TLC on x86 at 1 worker:
   - 1.5× on MCLamportMutex and SimpleRegular;
   - 2.5× on CoffeeCan1000BeansSafety.

   It must also preserve counts. If it does not, the design is revisited before
   further investment. The milestones are in `docs/developer/core/10-plan.md`.
6. **Layering.** `dive-core` may depend on `dive-ir`, `dive-config` and
   `dive-value`. It uses `dive-value` only at the binder boundary, to convert
   startup constants computed by the reference binder. `dive-explore` depends on
   `dive-core` and `dive-store`. `dive-api` dispatches between the engines. The
   rows are added to `xtask/src/layering.rs` with the first code PR.

7. **Better than TLC by default, where TLC is semantically wrong.** The
   default profile evaluates soundly where TLC's lazy-value cache reuse is
   stale (`docs/developer/core/02-semantic-contract.md` §3a). `--compat=tlc`
   reproduces TLC. The owner approved this result-changing default on
   2026-10-26, under ADR 0010. Each such behaviour is registered as a D-entry
   in the PR that implements it. Caching the errors of pure evaluations is
   result-preserving and allowed in both profiles.

## Consequences

- Correctness work concentrates in one new code base, and parity fixes to the
  old engine stop.
- For a period, two engines exist. Each new-core change is shadow-compared
  against the old engine across the corpus, not only against the goldens.
- Validator independence improves, because the validator no longer shares the
  checker's evaluator.
- ADR 0014 §1's plan interpreter is superseded by
  `docs/developer/core/04-plan-compiler.md`.
- Revisit at each milestone gate. If M0 misses its kill threshold, this ADR is
  amended with the measurements and a revised design.
