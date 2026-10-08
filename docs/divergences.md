# Divergences and enhancements

`dive --compat=tlc` matches the pinned TLC exactly and is proven against the full corpus. dive's default profile is TLC-compatible except where the owner has approved a better default. Wherever TLC's behaviour is a handcuff, dive builds a named option that does better. See [ADR 0010](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) for the rule. This file is the terse registry. Its user-facing narrative companion, [`tlc-differences.md`](tlc-differences.md), explains each entry with an example; every row here has a `D-NNN` entry there, kept in sync in the same PR. Every TLC handcuff found gets a row here, *before or with* its implementation.

**Kinds:**
- **preserving**: a result-preserving enhancement. It changes how dive gets the answer, never the answer. It may default to on once the harness shows equality with the TLC golden at every setting.
- **changing**: a result-changing option. It is opt-in unless the owner has approved it as the default. It is named, reported in the output's `options` block, and tested in both modes. Its TLC-matching counterpart is always built and is included in `--compat=tlc`.

**Status** is one of `recorded` (the design keeps it possible), `planned` (has a milestone), `implemented` or `default`.

## Registry

| Name (dive option) | TLC behaviour (the handcuff) | dive default | dive option | Kind | Status | Evidence / tests |
|---|---|---|---|---|---|---|
| Deterministic parallel search (`--workers N`, default `auto`) | Multi-worker BFS is only approximately breadth-first: depth and counterexample length vary with worker count, so exactness needs `-workers 1` (`TLCTrace.java`; tlaplus#883) | Exact BFS results identical to TLC's single-worker run | Any worker count gives the same verdict, counts, depth and chosen shortest counterexample | preserving | planned (M5; design in ADR 0015, proposed) | ADR 0004; harness worker-invariance runs; Examples `DieHard` (TLC depth 9 vs trace 7 at 4 workers); probes in `crates/dive-check/tests/fixtures/tlc-probes/` |
| Replayable randomness (`--seed`, seed always reported) | `RandomElement`/`RandomSubset`/`Randomization` are unseeded in exhaustive mode unless `-seed` is given; state counts vary per run | TLC-like: fresh seed per run, but the seed used is always reported | `--seed S` reproduces the run exactly | changing (only when a seed is fixed) | recorded | Examples `SpanningTree/SpanTreeRandom.cfg`: 1.9k to 240k distinct states without a seed, stable with `-seed 0` |
| Reproducible simulation (`dive simulate --seed`) | Simulation counts vary even with `-seed 0` and one worker | Same seed semantics as above | Bit-identical simulation from the same seed at any worker count | changing (only when a seed is fixed) | recorded | Examples `EWD840_anim` (generated counts 267k to 271k across runs) |
| Arbitrary-precision integers (`--int=big`; TLC mode `--int=tlc`) | 32-bit ints; overflow raises `EC.TLC_MODULE_OVERFLOW` (2178); literals above 2^31−1 rejected (`Naturals.java`, `Integers.java`, `SpecProcessor.java`) | **Big integers** (owner-approved default; flips on once both modes pass the harness) | `--int=tlc`, part of `--compat=tlc`: 32-bit with TLC's overflow error, proven against full conformance | changing | implemented in `dive-value` (`IntMode::Big` default, `IntMode::Tlc`; ADR 0012); CLI flag and harness flip pending | Harness fixture `Overflow`; `crates/dive-value/tests/fixtures/specs/Ov*`, `Ar*` (messages matched byte for byte); owner decision 2026-10-08; D-004 |
| Refuse symmetry with liveness (`--symmetry-liveness=refuse`; TLC mode `=tlc`) | Warns that symmetry with liveness "might cause TLC to miss violations" (`EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), then proceeds | **Refuse** with an explicit diagnostic (owner-approved default) | `--symmetry-liveness=tlc`, part of `--compat=tlc`: warn and proceed exactly as TLC does, proven to match | changing | planned (with dive-liveness) | Owner decision 2026-10-08 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | planned (M5) | Charter rule 4 |
| Structured results (`--output json`) | Human and `-tool` message streams; traces via `-dumpTrace json` only | Human-readable output | Versioned JSON: verdicts, stats, counterexamples as data, diagnostics, `options` block | preserving | planned (M3+) | Charter ergonomics |
| In-process library API (`dive-api`) | JVM process only | n/a | Embed checking, simulation and trace validation in Rust and `cargo test` | preserving | planned | Charter ergonomics |
| Canonical `CHOOSE` order (`--choose-order=canonical`; TLC mode `=tlc`) | TLC picks the first satisfying element in its internal value order. Strings are ordered by the *interning order* of their tokens in the module text, so an unused definition `Unused == "alpha"` above `Init` changes `CHOOSE e \in {"zeta","alpha","mid"} : TRUE` from `"zeta"` to `"alpha"`. Non-empty sets order by size first, tuples by length first | **Canonical order** (owner-approved default, 2026-10-08): text-independent, restoring metamorphic invariance under reordering definitions | `--choose-order=tlc`, part of `--compat=tlc`: TLC's order exactly, including interning order, proven against TLC | changing | implemented in `dive-value` (`ValueOrder::Canonical` default, `ValueOrder::Tlc`; ADR 0012); feeding the interner in SANY/TLC's encounter order is pending in `dive-sema`/`dive-config` | `docs/survey/competitive.md` probes P2 and P7; `crates/dive-value/tests/fixtures/specs/ChooseIntern*`, `PrintChoose`, `PrintSets`; D-006 |
| Output ordering (follows `--choose-order`) | Sets, record fields and function domains print in TLC's value order, so names (strings, model values, field names) print in intern order: `[qfb \|-> 1, qfa \|-> {1}]` when `qfb` is lexed first, `{"qsc", "qsb", "qsa"}` in order of first appearance | Canonical order (the `CHOOSE` order) | `--choose-order=tlc`, part of `--compat=tlc`: TLC's printing order exactly | changing (output text only, never a verdict) | implemented in `dive-value` (ADR 0012) | `crates/dive-value/tests/fixtures/specs/Print*`; D-006 |
| TLA+ comparisons (`--comparisons=tla`; TLC mode `=tlc`) — **default** (Owner decision 2026-10-08; D-009) | TLC refuses some comparisons that TLA+ answers: `[a \|-> 1, b \|-> 1] = <<1>>` is an error while `<<1>> = [a \|-> 1, b \|-> 1]` is `FALSE`; `<<1>> \in [a : {1}]` is an error; whether `{<<1>>, [a \|-> 1, b \|-> 1]}` is an error depends on the order of its elements; `1 \in {1, "a"}` is `TRUE` but `"a" \in {1, 2}` is an error; a record literal refuses `= <<1>>` while the same function built with `:>`/`@@` gives `FALSE` | TLC's rules in both profiles (not adopted). Exact under `--compat=tlc`. The default profile decides set-building errors exactly as TLC does, but a membership test mixing incomparable values can differ (D-009) | Proposed `--comparisons=tla`: answer `=` and `\in` between functions, records, tuples and sets as TLA+ defines them, symmetric and order-independent; keep the error only where TLA+ leaves the answer unspecified (`1 = "a"`) | changing | proposed (open question 12); built in `dive-eval` as `Comparisons::Tla`, not the default | `crates/dive-value/tests/fixtures/specs/Asym*`, `ErrMem*`, `PrintMember`, `RvFcnRcd*`, `RvCanonAsym`; `crates/dive-eval/tests/options.rs`; D-009 |
| Exploration order (`--explore-order=value`; TLC mode `=tlc`) — **canonical by default** (Owner decision 2026-10-08; D-010) | Sets are explored (`\E`, `\A`, filters, maps, function constructors, `x' \in S`) in TLC's value order, so the module text decides which of two violations at the same depth is reported | TLC's order in both profiles (not adopted) | Proposed `--explore-order=value`: explore in the run's value order, canonical in the default profile, making that profile independent of the module text | changing (which violation is reported, and the counts at the stop) | proposed (open question 9); built in `dive-eval` as `ExplorationOrder::Value` | Probe `StringOrderViolation` (TLC reports `InvZeta` after 2 states); `crates/dive-eval/tests/options.rs`; D-010 |
| Logical worker count (`TLCGet("config").worker`) | TLC reports its physical worker count, so specs asserting `worker = 1` (e.g. `MCDieHardest`) refuse to run in parallel | **1 (logical) at any `--workers`** (owner decision 2026-10-08) | `--compat=tlc`: physical count, as TLC reports it | changing (only for specs that read it) | planned (dive-stdlib, D3) | D-011; harness worker-invariance report |

## Oracle facts behind the registry

- **Introspecting specs.** Specs that read `TLCGet("config")` can legitimately behave differently by worker count. For example, Examples `DieHard/MCDieHardest` asserts `TLCGet("config").worker = 1`. dive reports `TLCGet("config").worker` with TLC's meaning, so such specs keep their TLC behaviour under `--workers N`. Whether dive should report the *effective* or the *logical* worker count under deterministic parallel search is an open design point for `dive-stdlib`. ADR 0014 proposes the logical count, 1 (open question 10).

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
- **Very deep cfg sets** (`DIVE-C0030`). `dive-config` refuses constant values with sets
  nested more than 1024 deep. TLC parses them recursively and fails with a stack overflow
  at a depth that depends on `-Xss`. Test: `crates/dive-config/tests/robustness.rs`.
- **Very deep values** (`DIVE-V0030`; ADR 0012).
  - **What is refused.** `dive-value` refuses printing, JSON, and TLC's checked comparison, equality and membership for values nested more than 500 levels deep.
  - **What still works.** Dropping, `==`, `cmp_with`, hashing and FP64 are iterative and handle any depth.
  - **TLC's limit.** TLC recurses up to its JVM stack (`-Xss`).
  - **Tests:** `crates/dive-value/tests/limits.rs` and `review_adversarial.rs` (a million levels for dropping, 200,000 for `==` and fingerprints, on a 2 MiB stack).

## TLC cfg reader artefacts (candidates)

`dive-config` reads every cfg exactly as the pinned TLC's `ModelConfig` does. There are no
disagreements on 886 corpus files or on 99 mutants; see
`crates/dive-config/acceptance/README.md`. The behaviours below look like artefacts of
TLC's reader rather than intended format rules. Each has a minimal case with TLC's
recorded result. dive matches TLC on all of them, and warns (`DIVE-C0100`–`C0102`)
where TLC accepts input that it will later fail to resolve. A future opt-in, result-changing
`--cfg=strict` could reject them. Nothing is implemented.

| Case | TLC behaviour |
|---|---|
| `CfgNegativeInt` | `N = -1` cannot be written: `-` becomes a model value, then 5006 on the next entry. |
| `CfgBigInt`, `CfgHexInt` | Integers above 2^31−1, or written in `\h`/`\o`/`\b` notation, fail as `GENERAL` (1000, exit 255), not as config errors (exit 151). |
| `CfgCommaList` | `INVARIANT A, B` is accepted with `,` as an invariant name; TLC fails later (2229). |
| `CfgKeywordAsValue` | `INIT NEXT` makes `NEXT` the init predicate. |
| `CfgSetTrailingComma` | `{1,}}` is a set containing the model value `}`. |
| `CfgStringRaw` | cfg strings keep escapes verbatim (`"a\nb"` has four characters), unlike TLA+ strings. |

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

## SANY semantic-analysis artefacts (candidates)

`dive-sema` matches SANY's semantic acceptance exactly by default: zero disagreements on
the corpora, the error corpus and 12,441 mutants (`crates/dive-sema/acceptance/README.md`).
The cases below are places where SANY's verdict looks like an artefact of its
implementation rather than of TLA+. Each has a minimal spec with SANY's recorded verdict
in `crates/dive-sema/tests/sany-cases/`.

They are recorded so that future opt-in options could deliberately differ: a lenient mode
for valid specifications SANY rejects, and a strict mode for suspicious ones SANY accepts.
None is implemented.

| Case | SANY behaviour | Possible option |
|---|---|---|
| `UnicodeNat` | `1 \in ℕ` with `EXTENDS Naturals` is "Unknown operator: `ℕ`": `ℕ`, `ℤ` and `ℝ` lex as identifiers that no standard module defines. | lenient: read them as `Nat`, `Int`, `Real` |
| `LetInstanceInner`, `ProofInstanceStep` | An `INSTANCE` of an inner module inside a `LET` or proof step makes the loader read the inner module a second time from the root file's text (`MonolithSpecExtractor`), then reject "Multiply-defined module". | lenient: resolve the inner module |
| `LabelOnAssumeProve` | A label inside an *unnamed* `THEOREM ASSUME ... PROVE` is "not in definition or proof step". | lenient |
| `DupVariable`, `DupConstant`, `DupTheoremName`, `DupStepNames` | Declaring a variable or constant twice, or reusing a theorem or step name, is only a warning (4801). | strict: reject |
| `UnnamedInstanceConflict` | `z == 2` then `INSTANCE P` with `P` defining `z == 1` is a warning; the instantiated `z` is silently dropped. | strict: reject |
| `BoxLetAction` | `[](LET y == 1 IN x' = x + y)` is accepted; the stuttering check on `[]A` looks only at a bare operator application. | strict: reject |
| `TemporalInIf` | `IF []TRUE THEN x' = x ELSE TRUE` is accepted; the mixed action/temporal check covers `/\`, `\/`, `=>`, `<=>` and junction lists, not `IF` or `CASE`. | strict: reject |
