# 0010: Better by default where approved; TLC-compatible always built and proven

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

### 1. Two profiles: dive's default and `--compat=tlc`

- **`--compat=tlc` (the TLC-compatible profile).**
  - Every observable result matches the pinned TLC under equivalent flags: verdict, counts, depth, error class, counterexample validity and length, and the meaning of every flag that shares a name with a TLC flag.
  - This is the profile the charter's conformance contract (rule 1) is enforced against, over **the full corpus**.
  - The TLC-matching behaviour is always *built*, never merely recorded. Every result-changing enhancement ships with its TLC-matching counterpart, and the harness proves that counterpart matches.
- **The default profile (dive).** This is TLC-compatible except where the owner has approved a better default. Approved defaults are listed in the registry in `docs/divergences.md`.
- **Per-feature switches.** Each enhancement also has its own flag, for example `--int=big|tlc` or `--symmetry-liveness=refuse|tlc`.

`--compat=tlc` is one global switch, set by the `--compat=tlc` flag, the `DIVE_COMPAT=tlc` environment variable, the `compat = "tlc"` config key, or `Profile::Tlc` in the library API. It puts **every** behaviour into its precisely TLC-equivalent form at once, for clean testing and for users who want to validate dive against TLC:

- Result-changing behaviours are switched, for example integers and symmetry with liveness.
- So are result-preserving defaults and observable conventions: the default worker count (TLC's 1), seeding (unseeded unless `-seed` is given), the meaning of flags shared with TLC, exit codes, and the shape of the default human-readable output where TLC defines it.
- A newly added option must declare its TLC-equivalent setting, or that it has no observable TLC counterpart. Otherwise it fails review.
- Per-feature flags given explicitly alongside `--compat=tlc` override it for that feature only. The output's `options` block then reports the run as `tlc+overrides`, listing each override, so a "pure TLC" claim is never made for a mixed run.
- The conformance harness runs dive under pure `--compat=tlc` for the strict tier.

### 2. Better behaviour is always available

Wherever TLC imposes a handcuff, dive builds a named, documented option that does better. Examples of handcuffs:

- single-worker exactness;
- unseeded or unreplayable randomness;
- 32-bit integers;
- a silent soundness hazard;
- output designed for humans only.

Until the option exists, the design must not foreclose it. Every option is recorded in the registry before or with its implementation.

### 3. Two kinds of "better"

**Result-preserving enhancements** change *how* dive gets the answer, not the answer. Examples: deterministic parallel BFS that still yields TLC's single-worker exact depth and shortest counterexample, faster storage, compilation.

- They are on by default in both profiles. The owner confirmed this for parallel workers on 2026-10-08.
- The harness must show equality with the TLC golden at every setting. The worker-invariance rule in ADR 0004 is the test.
- ADR 0004's single-worker constraint applies only to *generating goldens with TLC*. It never limits dive's own parallelism.

**Result-changing enhancements** change the answer relative to TLC. Examples: arbitrary-precision integers, refusing symmetry with liveness.

- They may be the **default** only with owner approval, recorded in the registry. Otherwise they are opt-in.
- In every case, the TLC-matching behaviour is built and selectable, and `--compat=tlc` passes full conformance.
- The default profile is also run through the harness. It must agree with the TLC goldens on every entry except the registered expected divergences. Each of those has a minimal spec, a justification, and a check against TLA+ semantics.
- The output and the JSON `options` block always say which profile and options were active. They also flag when a result would differ under `--compat=tlc`, where that can be known; integer overflow is one such case.

### 4. Nondeterminism must be replayable

Where TLC is nondeterministic, the default draws a fresh seed as TLC does. dive *always reports the seed* it used, and `--seed` replays a run exactly, including simulation at any worker count. A run that cannot be reproduced from its reported inputs is a bug.

### 5. Process

Whenever an agent finds a TLC handcuff, it:

1. writes the minimal spec and records the oracle result (charter, working method);
2. builds or keeps TLC-matching behaviour under `--compat=tlc`;
3. adds a row to the registry in `docs/divergences.md`: the handcuff, the default, the TLC-compat behaviour, the better option, its kind, its status and its tests;
4. proposes a better default to the owner when it changes results, through `docs/open-questions.md`. It never adopts one silently;
5. writes an ADR when the option shapes architecture, as with the value representation or the parallel search design.

### Owner decisions recorded (2026-10-08)

- **Parallel workers** are on by default. They are result-preserving.
- **Arbitrary-precision integers** are the intended default. TLC's 32-bit integers with its overflow error are built as `--int=tlc` (included in `--compat=tlc`), and full conformance to TLC is proven in that mode. The default flips to big integers once both modes pass the harness.
- **Symmetry with liveness:** the default is **refuse**, with an explicit diagnostic. TLC instead warns and proceeds, and can miss violations. TLC's behaviour is built as `--symmetry-liveness=tlc` (included in `--compat=tlc`), and the harness proves it matches.
- **`CHOOSE` order:** the default is a **canonical**, text-independent order (open question 8, answered 2026-10-08). TLC's interning-dependent order is built as `--choose-order=tlc` (included in `--compat=tlc`) and proven to match. The charter's metamorphic tests hold in the default profile; under `--compat=tlc` they exempt string and model-value `CHOOSE`.

## Consequences

- **Search design.** Parallel search must be designed for determinism from the start: a level-synchronous BFS with deterministic ordering, or an equivalent. A lock-free race that happens to be fast is not acceptable. This is a hard requirement on `dive-check` and `dive-store`.
- **Value representation.** `dive-value` must leave room for arbitrary-precision integers behind an option without penalising the 32-bit fast path.
- **CLI.** Flags that share a name with TLC flags keep TLC's meaning. Options only dive has use distinct names and are listed in `dive --help` under their own heading.
- **Reporting.** The JSON output has an `options` block that records every active enhancement and divergence, and the seed.
- **Defaults for result-preserving enhancements:** the owner confirmed on 2026-10-08 that these may default to on. For example, parallel workers may be on by default (`--workers auto`), even though TLC defaults to 1. This rests on results being identical by construction and by harness test.
