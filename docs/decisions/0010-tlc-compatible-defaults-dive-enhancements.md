# 0010: TLC-compatible by default, better by option

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Conformance work keeps finding places where TLC's behaviour is a *handcuff*: a limitation of the reference implementation, not part of TLA+'s meaning.

- **Approximate BFS.** With multiple workers TLC's breadth-first search is only approximate, so exact depth and shortest counterexamples need `-workers 1` (ADR 0004). TLC trades speed for exactness. It can't have both.
- **Unseeded randomness.** `RandomElement`, `RandomSubset` and the `Randomization` module draw from an unseeded random source in exhaustive mode unless `-seed` is given. `SpanTreeRandom`'s distinct-state count varied from about 1.9k to 240k across runs with identical flags.
- **Simulation isn't replayable.** Even `-seed 0` with one worker gives a different number of generated states from run to run.
- **32-bit integers.** Overflow is an error; arbitrary precision is not available.
- **Output built for humans.** Results are a message stream for people and the Toolbox, not data a Rust caller or a coding agent can consume.

The charter's soundness contract requires matching TLC by default (rule 5). The charter's goals also put determinism, performance, and ergonomics for agents and Rust callers first. The owner's direction (2026-10-08): do what the canonical implementation does, and keep the ability to do something better, where "better" means this project's definition (more performant, more deterministic, better for agents and Rust callers).

## Decision

### 1. The default is TLC-compatible

Every observable result matches the pinned TLC under equivalent flags. This covers:

- verdict;
- counts;
- depth;
- error class;
- counterexample validity and length;
- the meaning of every flag that shares a name with a TLC flag.

### 2. Better behaviour is always available as a named option

Wherever TLC imposes a handcuff, dive keeps a named, documented option that does better. "Keep" means the design must not foreclose it. The option need not be built on day one, but no implementation choice may make it impossible later. The option is recorded in the registry in `docs/divergences.md` before or with its implementation.

### 3. Two kinds of "better"

Each kind has its own rule for defaults.

- **Result-preserving enhancements.** These change *how* dive gets the answer, not the answer. Examples: deterministic parallel BFS that still yields TLC's single-worker exact depth and shortest counterexample, faster storage, compilation.
  - They may be on by default, because the observable result equals the TLC-compatible one.
  - The conformance harness must show equality with the TLC golden at every setting. The worker-invariance rule in ADR 0004 is the test.
  - The single-worker constraint in ADR 0004 applies to *generating goldens with TLC*. It is never a limit on dive's own parallelism.
- **Result-changing options.** These change the answer relative to TLC. Examples: arbitrary-precision integers, refusing symmetry with liveness, a fixed default seed, alternative `CHOOSE` orders.
  - They are always **opt-in** and named.
  - The output and the JSON report always say which options were active.
  - They are tested in both modes. Where TLC has no equivalent, they are tested against the TLA+ semantics.

### 4. Nondeterminism must be replayable

Wherever TLC is nondeterministic, the default draws its seed fresh to match TLC's behaviour. Even so, dive *always reports the seed it used*. `--seed` replays a run exactly, including simulation at any worker count. A run that cannot be reproduced from its reported inputs is a bug.

### 5. Process

Whenever an agent finds a TLC handcuff, it:

1. writes the minimal spec and the oracle result (charter, working method);
2. matches TLC by default;
3. adds a row to the registry in `docs/divergences.md`: the handcuff, the TLC-compatible default, the dive option, whether the option preserves or changes results, its status and its tests;
4. writes an ADR when the option shapes architecture, for example the parallel search design or the value representation.

## Consequences

- **Search design.** Parallel search must be designed for determinism from the start: a level-synchronous BFS with deterministic ordering, or an equivalent. A lock-free race that happens to be fast is not acceptable. This is a hard requirement on `dive-check` and `dive-store`.
- **Value representation.** `dive-value` must leave room for arbitrary-precision integers behind an option without penalising the 32-bit fast path.
- **CLI.** Flags that share a name with TLC flags keep TLC's meaning. Options only dive has use distinct names and are listed in `dive --help` under their own heading.
- **Reporting.** The JSON output has an `options` block that records every active enhancement and divergence, and the seed.
- **Open question:** may result-preserving enhancements such as parallel workers default to "on" (for example `--workers auto`)? TLC itself defaults to 1 worker. This ADR permits it, because results are identical by construction and by test. The owner may override.
