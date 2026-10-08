# Divergences and enhancements

`dive` matches the pinned TLC by default (charter, soundness contract rule 5). Wherever TLC's behaviour is a handcuff, dive also keeps a named option that does better. See [ADR 0010](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) for the rule. This file is the registry. Every TLC handcuff found gets a row here, *before or with* its implementation.

**Kinds:**
- **preserving**: a result-preserving enhancement. It changes how dive gets the answer, never the answer. It may default to on once the harness shows equality with the TLC golden at every setting.
- **changing**: a result-changing option. It is always opt-in, named, is reported in the output's `options` block, and is tested in both modes.

**Status** is one of `recorded` (the design keeps it possible), `planned` (has a milestone), `implemented` or `default`.

## Registry

| Name (dive option) | TLC behaviour (the handcuff) | dive default | dive option | Kind | Status | Evidence / tests |
|---|---|---|---|---|---|---|
| Deterministic parallel search (`--workers N`) | Multi-worker BFS is only approximately breadth-first: depth and counterexample length vary with worker count, so exactness needs `-workers 1` (`TLCTrace.java`; tlaplus#883) | Exact BFS results identical to TLC's single-worker run | Any worker count gives the same verdict, counts, depth and chosen shortest counterexample | preserving | recorded | ADR 0004; harness worker-invariance runs; Examples `DieHard` (TLC depth 9 vs trace 7 at 4 workers) |
| Replayable randomness (`--seed`, seed always reported) | `RandomElement`/`RandomSubset`/`Randomization` are unseeded in exhaustive mode unless `-seed` is given; state counts vary per run | TLC-like: fresh seed per run, but the seed used is always reported | `--seed S` reproduces the run exactly | changing (only when a seed is fixed) | recorded | Examples `SpanningTree/SpanTreeRandom.cfg`: 1.9k to 240k distinct states without a seed, stable with `-seed 0` |
| Reproducible simulation (`dive simulate --seed`) | Simulation counts vary even with `-seed 0` and one worker | Same seed semantics as above | Bit-identical simulation from the same seed at any worker count | changing (only when a seed is fixed) | recorded | Examples `EWD840_anim` (generated counts 267k to 271k across runs) |
| Arbitrary-precision integers (`--int=big`) | 32-bit ints; overflow raises `EC.TLC_MODULE_OVERFLOW` (2178); literals above 2^31−1 rejected (`Naturals.java`, `Integers.java`, `SpecProcessor.java`) | Match TLC's overflow error | Unbounded integers | changing | recorded | Harness fixture `Overflow`; owner question 2 |
| Strict symmetry with liveness (`--strict-symmetry`) | Warns that symmetry with liveness "might cause TLC to miss violations" (`EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), then proceeds | Match TLC (warn and proceed), clearly flagged in output | Refuse the combination | changing | recorded | Owner question 3 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | planned (M5) | Charter rule 4 |
| Structured results (`--output json`) | Human and `-tool` message streams; traces via `-dumpTrace json` only | Human-readable output | Versioned JSON: verdicts, stats, counterexamples as data, diagnostics, `options` block | preserving | planned (M3+) | Charter ergonomics |
| In-process library API (`dive-api`) | JVM process only | n/a | Embed checking, simulation and trace validation in Rust and `cargo test` | preserving | planned | Charter ergonomics |
| Deterministic `CHOOSE` documentation | TLC picks by its internal value order; undocumented | Match TLC's order exactly | (none planned; recorded so any future change is explicit) | — | to be characterised | — |
| Output ordering | To be characterised | Match TLC | Canonical ordering for machine output | preserving | to be characterised | — |

## Oracle facts behind the registry

- **Introspecting specs.** Specs that read `TLCGet("config")` can legitimately behave differently by worker count. For example, Examples `DieHard/MCDieHardest` asserts `TLCGet("config").worker = 1`. dive reports `TLCGet("config").worker` with TLC's meaning, so such specs keep their TLC behaviour under `--workers N`. Whether dive should report the *effective* or the *logical* worker count under deterministic parallel search is an open design point for `dive-stdlib`.

## Refusals where SANY accepts

These are not divergences, because `dive` gives no different answer: it refuses with
an explicit error code and never approximates (soundness rule 3).

- **Very deep syntax** (`DIVE-S0040`; ADR 0009). SANY's parser is limited only by the
  Java stack.
  - `dive-syntax` refuses nesting of more than 256 expressions, proofs or modules.
  - It also refuses a module unit whose syntax tree is more than 2048 levels high, such
    as a chain of thousands of `+`, because rowan frees trees recursively.
  - Tests: `crates/dive-syntax/tests/roundtrip.rs`. No corpus module comes near either
    limit.

## SANY acceptance artefacts (candidates)

`dive-syntax` matches SANY's parser acceptance exactly by default. Zero disagreements were found across 13,646 modules; see ADR 0009. SANY rejects the inputs below, and they look like artefacts of its JavaCC implementation rather than TLA+ semantics. Each has a minimal spec with SANY's recorded verdict in `crates/dive-syntax/tests/sany-cases/`.

They are recorded so that a future opt-in `--syntax=lenient` (result-changing) could accept them deliberately. They are not implemented.

| Case | SANY behaviour |
|---|---|
| `SetEnumCommaIn` | `{a, b \in S}` is a syntax error. |
| `SetMapIn` vs `SetMapInUnicode` | `{f(a) \in b : a \in S}` is rejected with `\in` but accepted with `∈`. |
| `RecordKeywordSecond` | A keyword works as a record field only in first position. |
| `InstanceKeywordName` | Top-level `INSTANCE ACTION` is rejected, but `LOCAL INSTANCE ACTION` is accepted. |
| `LexStepInExpr` | `3 <4> 5` is rejected because `<4>` lexes as a proof-step token. |
| `BadJunctionLexeme` | `a./\` lexes as a token that no grammar rule accepts. |
| `OpArgPlus` | `F(+)` lexes `(+)` as `⊕`. |
