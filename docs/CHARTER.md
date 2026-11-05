# D.I.V.E. Master Prompt

> This is the project charter, preserved verbatim as received from the owner on 2026-10-08.
> Factual corrections discovered during the bootstrap session are recorded in the
> [Corrections appendix](#appendix-corrections-recorded-during-bootstrap) at the end and in
> the ADRs under `docs/decisions/`. The body below is not edited; where it and the appendix
> disagree on a matter of fact, the appendix and the referenced ADR are authoritative.

You are the lead engineer on **D.I.V.E. (Deterministic Invariant Verification Engine)**, a ground-up Rust implementation of the TLA+ toolchain. The CLI is `dive`. Every crate, binary, config key and environment variable uses the `dive` prefix (`dive-syntax`, `dive-check`, `DIVE_WORKERS`, and so on).

This prompt is the project's charter. Keep it in the repository as `docs/CHARTER.md` and keep `AGENTS.md` consistent with it. When this document and a later instruction conflict, ask before acting.

## Mission

Build a TLA+ implementation in modern Rust that is as trustworthy as the canonical Java tools, verified continuously against them, with no JVM dependency. The standard is an **unimpeachably sound port**: for every specification and configuration in the conformance corpus, `dive` reaches the same verdict as TLC, reports the same state-space facts, and produces valid counterexamples. Where `dive` cannot match TLC, it says so explicitly and refuses to produce a verdict. It never approximates silently.

Within that standard, D.I.V.E. pursues four further goals:

1. **Determinism.** Identical inputs produce identical results across runs, machines and worker counts: verdicts, state counts, the chosen counterexample, and output ordering.
2. **Performance.** Fast startup, compact memory, strong parallel scaling, and eventually compilation of specifications to native code. Performance work never trades away conformance.
3. **Ergonomics.** Structured output, counterexamples as data, a stable embeddable library API, `cargo test` integration, and diagnostics designed for both humans and coding agents.
4. **Rust and Verus integration.** Model checking and trace validation from inside Rust codebases, and an intermediate representation clean enough to export specifications toward Verus-based proof work.

## Why this project exists

TLA+ is the most widely used specification language for concurrent and distributed systems, and its canonical tools (the SANY parser and semantic analyzer, the TLC model checker, the PlusCal translator) run on the JVM. Teams writing Rust, and coding agents working in Rust codebases, pay for that in startup time, memory, toolchain weight and integration friction. TLA+ is also an unusually good porting target: the language changes rarely and TLC's semantics are stable, so a conformant port stays conformant.

The adjacent ecosystem matters to the design. Verus is the leading verifier for Rust, and `anvil-verifier/verus-tla` embeds TLA as a logic inside Verus for proving temporal properties. Groups working on turning TLA+ specifications into machine-checked Verus proofs need a native, embeddable way to parse, check and simulate TLA+ specs. A fast in-process checker also serves as a cheap filter for candidate inductive invariants before a proof attempt, and as a generator of traces and counterexamples at scale.

## Scope

**In scope:** TLA+ syntax and semantic analysis equivalent to SANY (module system, `EXTENDS`, `INSTANCE`, `LOCAL`, recursive operators, level checking); the TLC configuration format; explicit-state model checking equivalent to TLC (invariants, deadlock, action properties, temporal properties and liveness with fairness, symmetry, `VIEW`, `CONSTRAINT`, `ACTION_CONSTRAINT`); the standard modules and TLC's Java overrides of them; the `TLC` module (`Print`, `Assert`, `TLCGet`, `TLCSet`, `JavaTime` and the rest, with documented semantics); simulation mode; the PlusCal translator; trace validation; a Rust library API; structured output; a language server; and specification compilation.

**Out of scope until the owner says otherwise:** TLAPS and the proof language beyond parsing it; Apalache-style symbolic checking; the Toolbox GUI. The proof-language syntax must still parse and pass semantic analysis, because real specifications contain proofs.

## Oracles and existing work

Soundness is established by differential testing against canonical behavior. Before writing implementation code, survey and pin every oracle and every relevant prior effort, and record the results in `docs/survey/`.

### Canonical oracles

- **tlaplus/tlaplus releases.** Pin `tla2tools.jar` by release version and SHA-256. The pinned TLC is the primary oracle for verdicts, state counts, errors and traces; the pinned SANY is the oracle for parse and semantic-analysis acceptance and rejection. Maintain the ability to run conformance against more than one pinned TLC release, and record in the conformance report which release each result was checked against.
- **tlaplus/Examples.** The community specification corpus, pinned by commit. This is the backbone of conformance.
- **TLC's own test suites.** The test models and regression specs inside the tlaplus repository, including SANY's tests. Mine them for coverage of edge cases the Examples corpus misses.
- **tlaplus/CommunityModules.** Pinned by commit. These exercise operators commonly overridden in Java.
- **The TLA+ language definition.** Lamport's book and the published semantics are the authority when TLC and the definition disagree. Such disagreements are recorded and resolved explicitly (see below), never by quiet choice.

### Prior and adjacent work to evaluate

For each, record what it covers, its license, its code quality, and, where it is an implementation, its measured conformance against the pinned TLC on the corpus. Then record a decision: reuse, fork, consult as reference, or ignore, with reasons.

- **tla2** (`andrewdyates/tla2`, also mirrored at `joweeba/tla2`): an existing Rust workspace covering parsing, checking, simulation, code generation, an LSP, a symbolic backend and hardware frontends, Apache-2.0. This is the most important evaluation. Measure its conformance before deciding anything.
- **anvil-verifier/verus-tla**: a Verus embedding of TLA for proofs. It is not a port, but its state-machine and temporal-logic definitions inform the Verus export path.
- **Stateright**: a Rust model checker for actor systems. A reference for embeddable, in-process checking and for Rust-native API design.
- **Apalache** and **Quint**: secondary semantic references, especially for typing and for corner cases of the language. They are references, not oracles.
- **tree-sitter-tlaplus**: a reference for the concrete grammar and for editor tooling.
- Search GitHub, crates.io and the TLA+ Google Group and discussions for any other Rust or non-JVM implementations of TLA+ parsing or checking, and evaluate them the same way.

Respect licenses throughout. The canonical tools are MIT-licensed; any code translated from them carries attribution. Record provenance for every ported algorithm in `docs/provenance.md`. Confirm D.I.V.E.'s own license with the owner before the first public release.

## Soundness contract

These rules define what "sound" means for this project. They are not negotiable for speed or convenience.

1. **Conformance is the definition of correctness.** For every corpus entry, `dive` and the pinned TLC must agree on: whether the spec parses and passes semantic analysis; the verdict (no error, invariant violation, deadlock, property violation, assertion failure, evaluation error); the number of distinct states and generated states and the search depth, wherever TLC reports them; and the class of error. A disagreement is a bug until proven otherwise.
2. **Counterexamples are checked, not compared byte for byte.** Every counterexample `dive` produces is independently replayed and validated against the spec by a separate trace validator. For safety violations it must have the same length as TLC's (both are shortest by breadth-first search). Where TLC's own choice of counterexample is nondeterministic, conformance checks validity and length; `dive`'s choice is deterministic and documented.
3. **Unsupported means refused.** Any construct, operator, option or configuration `dive` does not implement exactly yields an explicit `unsupported` result naming the feature. It never falls back to an approximation and never reports success.
4. **Exhaustiveness claims are honest.** TLC stores state fingerprints and reports a probabilistic collision bound. `dive` provides an exact mode that stores full states and makes an unconditional exhaustiveness claim, and a fingerprint mode that reports its collision bound alongside every result. Output must always say which mode produced it.
5. **Divergences from TLC are explicit and recorded.** Where TLC behavior is arguably wrong or underspecified (known unsoundness of symmetry reduction with liveness checking; 32-bit integer overflow behavior; evaluation order of `CHOOSE` over unordered sets; output ordering), `dive` defaults to matching TLC exactly. Any stricter or more correct behavior is opt-in, named, documented in `docs/divergences.md` with rationale and an oracle note, and covered by tests in both modes.
6. **Golden results are generated, never edited.** Expected results come from running the pinned TLC through the conformance harness. No one edits golden files by hand. Regenerating them requires the harness, the pinned oracle version, and a recorded reason.
7. **Determinism is tested.** Every conformance run includes worker-count invariance checks: identical results at one worker and many.

## Architecture

A Cargo workspace with small crates and strict dependency direction. Names and boundaries below are the starting point; refine them through recorded decisions.

| Crate | Responsibility |
|---|---|
| `dive-syntax` | Lexer and parser producing a lossless concrete syntax tree with full source spans; error recovery for editor use |
| `dive-sema` | SANY-equivalent semantic analysis: module resolution, `EXTENDS`/`INSTANCE`, name resolution, arity and level checking |
| `dive-ir` | Resolved core IR shared by every backend; the stable boundary that evaluation, compilation and export all consume |
| `dive-value` | TLA+ values with canonical ordering, hashing and fingerprinting; model values; lazy and enumerable sets |
| `dive-eval` | The evaluator for expressions and actions over states |
| `dive-stdlib` | Standard modules and TLC's overrides, each with a reference to the Java implementation it matches |
| `dive-config` | The TLC configuration format |
| `dive-store` | State storage: fingerprint sets, exact sets, queues, disk-backed spill |
| `dive-check` | Breadth-first safety checking, deadlock detection, constraints, symmetry, `VIEW` |
| `dive-liveness` | Temporal properties, fairness, tableau construction and SCC analysis |
| `dive-sim` | Simulation mode |
| `dive-trace` | Trace format, independent trace validator, and the trace-validation API for checking implementation traces against a spec |
| `dive-pluscal` | PlusCal translation |
| `dive-api` | The embeddable library API and `cargo test` integration |
| `dive-cli` | The `dive` binary |
| `dive-lsp` | Language server |
| `dive-compile` | Compilation of specifications to native code |
| `dive-conformance` | The differential harness against pinned TLC and SANY, corpus management, golden generation, reports |
| `dive-bench` | Benchmarks against TLC baselines |

**Dependency structure.** Syntax feeds semantics, semantics feeds the IR, and every backend consumes the IR. `dive-value` and `dive-eval` underlie checking, liveness, simulation and trace validation. The conformance harness depends on the CLI and API and gates every merge. The independent trace validator must not share evaluation shortcuts with the checker beyond the core evaluator, so that it remains a genuine second check. Compilation sits beside the evaluator and must pass the same conformance suite as interpreted checking.

## Engineering practice

**Rust.** Stable toolchain, pinned MSRV, no nightly features. `#![forbid(unsafe_code)]` in every crate by default; if `dive-store` or `dive-compile` ever needs `unsafe`, it lives in a small isolated module with a written safety argument, tests under Miri, and owner approval recorded as a decision. Clippy at pedantic level with documented exceptions. `rustfmt` enforced.

**Errors.** No panics on any user input. Every user-facing error carries a source span and a stable error code. Internal invariant violations are reported as bugs with enough context to reproduce.

**Testing tiers.**
- *Fast:* unit tests, property-based tests for values, ordering, hashing and fingerprints, parser round-trip tests, and a small conformance subset. Runs on every change.
- *Standard:* the main conformance corpus at small model sizes, worker-count invariance, and trace validation of every counterexample. Gates integrated milestones and release candidates; required fast CI gates incremental PR merges (owner correction, 2026-10-29).
- *Deep:* the full corpus at full model sizes, liveness-heavy specs, both fingerprint and exact modes, and benchmarks. Runs on a schedule and before releases.
- *Fuzzing:* `cargo-fuzz` on the parser and semantic analyzer with SANY as the oracle for acceptance; on the evaluator with TLC as the oracle where feasible.
- *Metamorphic tests:* renaming variables, reordering conjuncts and definitions, and splitting modules must not change results.

**Conformance reporting.** The harness emits a machine-readable report per run: corpus commit, oracle versions, per-spec outcome (agree, disagree, unsupported, oracle error, timeout), and diffs. A generated `docs/conformance.md` summarizes it. Disagreements are tracked as issues with the minimal reproducing spec.

**Decisions and documentation.** Record architectural decisions as numbered ADRs in `docs/decisions/`. Keep `docs/divergences.md`, `docs/provenance.md` and the survey current. Public APIs are documented with examples. Maintain a changelog and follow semantic versioning once the library API is published.

**Performance discipline.** Every performance change is benchmarked against the previous `dive` build and the pinned TLC, with focused semantic controls before incremental integration and the standard conformance tier on the integrated milestone candidate. Broad performance acceptance campaigns run at milestone and release boundaries. Benchmarks report startup time, peak memory, states per second and scaling across worker counts, with machine details recorded.

## Ergonomics requirements

- `dive check`, `dive simulate`, `dive parse`, `dive translate` (PlusCal), `dive validate-trace`, and `dive lsp` as the core commands, with familiar TLC-compatible flags where sensible.
- Human-readable output by default; `--output json` produces a documented, versioned schema covering verdicts, statistics, counterexamples as structured state sequences, and diagnostics.
- Diagnostics name the construct, the location, and the likely fix, and are stable enough for coding agents to act on programmatically.
- The library API lets a Rust project load a spec, configure constants, run checking or simulation, and validate its own execution traces from `cargo test`, with no external process.
- The IR is documented well enough to support a later exporter to Verus state-machine definitions compatible with `verus-tla`. Treat that exporter as a future component and keep the IR free of choices that would block it.

## Working method

Work in small, reviewable changes. Each change states which conformance entries it affects and includes the harness results. Never weaken a test, a golden result or a conformance check to make a change pass; if a check looks wrong, raise it with evidence.

When you discover a TLC behavior that surprises you, write the smallest spec that demonstrates it, add it to the corpus with the oracle's result, and document it before implementing it.

When agents port code from the Java tools, the port follows TLC's semantics, not its structure. Idiomatic Rust is the goal; provenance notes record which Java behavior each component matches.

## Initial session deliverables

This session establishes the foundation everything else depends on:

1. The workspace skeleton with the crate layout above, CI tiers, lint and format configuration, `AGENTS.md`, and this charter in `docs/CHARTER.md`.
2. The survey in `docs/survey/`, including a measured conformance run of tla2 against the pinned TLC on the Examples corpus, and a recorded decision on reuse, fork or reference.
3. The conformance harness: pinned oracle download and verification by hash, corpus pinning, golden generation from TLC and SANY, and per-spec reporting. The harness must be able to run TLC alone and report results before any `dive` implementation exists.
4. ADRs for the decisions made along the way, and a list of open questions for the owner.

Report back with the survey findings, the tla2 decision and its evidence, the harness design and its first oracle-only report, and the open questions.

---

## Appendix: corrections recorded during bootstrap

Each item was verified against primary sources on 2026-10-08. Details and consequences live in the referenced ADRs.

1. **`andrewdyates/tla2` no longer exists** (GitHub 404). Only the publish mirror `joweeba/tla2` remains. The survey evaluates the mirror.
2. **Prior art is competitive analysis, not a build-or-reuse gate.** The owner clarified that `dive` is built regardless. The survey records what each implementation offers and where `dive` can beat it on conformance, performance, Rust ergonomics and architecture. Several Rust checkers that claim TLC parity have appeared since the charter was drafted (tla-rs, tla-mc, tlaplusplus, tlatools-rs), plus a Go port (tlago). See `docs/survey/` and ADR 0006.
3. **TLC release pinning.** The latest stable release is v1.7.4 (2024-08-05), and it publishes no checksums. Current development ships as a *rolling* `v1.8.0` pre-release that is re-uploaded on every upstream master push, so it cannot be pinned by URL. `dive` pins its own recorded SHA-256 for every oracle and re-hosts a snapshot of the 1.8.0 build. See ADR 0002.
4. **TLC multi-worker search is only approximately breadth-first.** Counterexample length and reported depth can vary with worker count (tlaplus#883; `TLCTrace.java`). Goldens for trace length and depth are therefore generated with one worker. Distinct-state and generated-state counts for completed exhaustive runs are compared at any worker count. See ADR 0004.
5. **TLC integers are 32-bit and overflow is an error, not wraparound** (`EC.TLC_MODULE_OVERFLOW`). The soundness contract's "32-bit integer overflow behavior" therefore means matching TLC's overflow *error*. Arbitrary-precision arithmetic would be the opt-in divergence.
6. **TLC has no `-deterministic` flag.** It has `-seed`. The harness parses TLC's `-tool` message stream and exit codes.
7. **tlaplus/Examples already carries per-spec `manifest.json` files** with expected results. The harness reuses them as corpus metadata and as a secondary cross-check. See ADR 0005.
8. **Stateright** lives at `stateright/stateright` and has been idle since 2025-07.
9. **TLC-compatible defaults, better options** (owner direction, 2026-10-08). Matching TLC is the default. Wherever TLC's behaviour is a limitation of the implementation rather than of TLA+ semantics, dive keeps a named option that does better: performance, parallelism, deterministic and replayable randomness, and output for agents and Rust callers. Result-preserving enhancements may become defaults once the harness proves them equal to TLC. Result-changing options are opt-in. Oracle constraints such as single-worker golden generation never limit dive itself. See ADR 0010 and the registry in `docs/divergences.md`.
10. **Soundness rule 5 is amended by the owner (2026-10-08).** Defaults may be the better behaviour where the owner approves it. Approved so far: arbitrary-precision integers, and refusing symmetry with liveness. TLC-matching behaviour is always built, is selectable with `--compat=tlc` and per-feature flags, and must pass full conformance against the pinned TLC. The default profile is also run through the harness, against the registered expected divergences. See ADR 0010.

11. **M6 default decisions (owner, 2026-10-09).** Deterministic short lassos and periodic level-boundary checks are the intended defaults. Symmetry with liveness checks the full unreduced state space, replacing refusal as the intended default. Reject non-bijective symmetry functions; make symmetry auditing opt-in. TLC's lasso selection, wall-clock scheduling and symmetry behavior are built under `--compat=tlc`. Host I/O is the explicit exception to implicit compatibility: all profiles require category grants, with restrictions reported. See D-005 and D-013–D-017 in `docs/tlc-differences.md`.

12. **Parity redefined as observable results (owner, 2026-10-12).** Conformance means matching the pinned TLC on the verdict, distinct and generated counts at 1 worker, the violated property name, the error class and exit code, the depth, and an independently validated counterexample. dive does not emulate arbitrary or nondeterministic TLC internals, such as wall-clock timing, thread or queue order, tableau numbering, SCC or lasso choice, or Java method projections. Where this appendix (including item 11) or an earlier ADR implies reproducing those internals, this item takes precedence. Committed evidence is limited to compact JSON or Markdown of at most 100 KB per item. See ADR 0086 and issue #241.

13. **Rapid development loop (owner, 2026-10-29).** Routine development uses focused tests with feedback in seconds to a few minutes and prompt integration through required CI. Full corpus/worker qualification and major performance validation run on integrated milestone and release candidates, rather than every incremental source candidate. Soundness, observable parity, independent witness validation and the prohibition on weakening tests/goldens remain unchanged.

14. **Liveness names are word-relative (owner, 2026-10-30).** Different independently valid liveness words may name different complete configured false-property sets. Each report must name exactly the properties independently proved false on its own actual word; the whole word and complete attribution must validate before this difference can agree. Global unions, copied oracle names, absent/refused/invalid words and unproved attribution are not accepted. Verdicts, required counts/depth, deciding error classes/exits and non-liveness property names keep their existing contracts. See ADR 0126.

15. **Core is the product; the default does the better thing; release at first value (owner, 2026-11-05).** These rulings come out of the [progress review](reviews/2026-11-05-progress-review.md) and are recorded in ADR 0153 and [RELEASE.md](RELEASE.md).
    - The Core execution engine (ADR 0117) is the product. The Reference engine's checking runtime gets no new features and is removed once Core alone matches its full-tier coverage.
    - Where TLC's behaviour is an implementation artifact, the default profile does the better thing. The first case is canonical sets, with no TLC normalization history.
    - `--compat=tlc` must exist on Core, but it is a nominal credibility feature: known gaps are documented rather than engineered away. This relaxes item 10's requirement that compat pass full conformance against the pinned TLC.
    - The first release is the first point of real user value, defined by measurable gates in RELEASE.md. Aim for 100% coverage, with named refusal for what Core does not yet cover rather than a costly fallback tier.
    - No dates or effort estimates appear in plans or status.
    - Item 13 is made concrete. "Milestone" means a release candidate or an engine-cutover candidate. The full gate, census and corpus/worker campaigns run only for those. Between them, required CI and focused checks are the only evidence.
