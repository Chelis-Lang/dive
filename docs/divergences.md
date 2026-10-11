# Divergences and enhancements

`dive --compat=tlc` selects pinned TLC behavior on supported paths, with named refusals for unfinished features; complete toolchain parity remains in progress. dive's default profile is TLC-compatible except where the owner has approved a better default. Wherever TLC's behaviour is a handcuff, dive builds a named option that does better. See [ADR 0010](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) for the rule. This file is the terse registry. Its user-facing narrative companion, [`tlc-differences.md`](tlc-differences.md), explains each entry with an example; every row here has a `D-NNN` entry there, kept in sync in the same PR. Every TLC handcuff found gets a row here, *before or with* its implementation.

**Kinds:**
- **preserving**: a result-preserving enhancement. It changes how dive gets the answer, never the answer. It may default to on once the harness shows equality with the TLC golden at every setting.
- **changing**: a result-changing option. It is opt-in unless the owner has approved it as the default. It is named, reported in the output's `options` block, and tested in both modes. Its TLC-matching counterpart is always built and is included in `--compat=tlc`.

**Status** is one of `recorded` (the design keeps it possible), `planned` (has a milestone), `implemented` or `default`.

## Registry

| Name (dive option) | TLC behaviour (the handcuff) | dive default | dive option | Kind | Status | Evidence / tests |
|---|---|---|---|---|---|---|
| Deterministic parallel search (`--workers N`, default `auto`) | Multi-worker BFS is only approximately breadth-first: depth and counterexample length vary with worker count, so exactness needs `-workers 1` (`TLCTrace.java`; tlaplus#883) | Exact BFS results identical to TLC's single-worker run | Any worker count gives the same verdict, counts, depth and chosen shortest counterexample | preserving | **implemented** (`dive check --workers N`, WP-D3: ADR 0015's three-phase level engine; specs using `TLCSet`, live `TLCGet`, randomness, I/O or runtime name constructors under TLC order run in reported sequential mode (#122)) | ADR 0004; harness worker-invariance runs at 1, 3, 4, 8 workers and chaos; `crates/dive-check/tests/invariance.rs`; Examples `DieHard` (TLC depth 9 vs trace 7 at 4 workers); probes in `crates/dive-check/tests/fixtures/tlc-probes/` |
| Replayable randomness (`--seed`, seed always reported) | `RandomElement`/`RandomSubset`/`Randomization` are unseeded in exhaustive mode unless `-seed` is given; state counts vary per run. Draws in `Next` are seeded from the state fingerprint, whose polynomial is random unless `-fp` is given, so `-seed` alone does not replay them. A draw before `Init` can make TLC lose the counterexample (exit 12, no trace) | TLC-like: fresh seed per run, but the seed used is always reported | `--seed S` alone reproduces the run exactly at any worker count (streams keyed by seed, state fingerprint or trace number, and position); `--compat=tlc` reproduces TLC's draws bit for bit | changing (only when a seed is fixed) | implemented in `dive-stdlib` (`random`, both modes) and wired into `dive check` (`--seed`; start-up and per-state streams; replay as TLC does it) | Examples `SpanningTree/SpanTreeRandom.cfg`: 1.9k to 240k distinct states without a seed, stable with `-seed 0`, golden reproduced from the first six draws; `crates/dive-stdlib/tests/fixtures/random/` (six seeds, `RndTraceAssume`, `RndLetRedraw`); D-002 |
| Reproducible simulation (`dive simulate --seed`) | Simulation counts vary even with `-seed 0` and one worker. Each worker has its own generator for choosing steps, and every worker draws `RandomElement` from the same unreseeded `-seed` generator, so a trace's draws depend on which worker ran it | Same seed semantics as above | Bit-identical simulation from the same seed at any worker count: one stream per trace number | changing (only when a seed is fixed) | streams implemented in `dive-stdlib` (`Streams::simulation_trace`, TLC's `tlc_simulation_worker_seeds`); finite pure single-worker safety simulation implemented (`dive-sim`, ADR 0020); spec random operators, other modes and worker scaling remain named refusals | Examples `EWD840_anim` (generated counts 267k to 271k across runs); `crates/dive-stdlib/tests/java_random.rs`, `dive_mode.rs`; D-003 |
| Arbitrary-precision integers (`--int=big`; TLC mode `--int=tlc`) | 32-bit ints; overflow raises `EC.TLC_MODULE_OVERFLOW` (2178); literals above 2^31−1 rejected (`Naturals.java`, `Integers.java`, `SpecProcessor.java`) | **Big integers** (owner-approved default; flips on once both modes pass the harness) | `--int=tlc`, part of `--compat=tlc`: 32-bit with TLC's overflow error, proven against full conformance | changing | implemented in `dive-value` (`IntMode::Big` default, `IntMode::Tlc`; ADR 0012); CLI flag and harness flip pending | Harness fixture `Overflow`; `crates/dive-value/tests/fixtures/specs/Ov*`, `Ar*` (messages matched byte for byte); probe `P1Overflow` in `conformance/corpus/probes/` (default profile: success, 3 states; registered expected divergence); owner decision 2026-10-08; D-004 |
| Sound symmetry with liveness (`--symmetry-liveness=unreduced`) | TLC checks a reduced graph and can both miss and invent violations | **Unreduced**, once implemented: disable symmetry reduction for the whole liveness run and report it | `=refuse` remains available; `=tlc` / `--compat=tlc` reproduces warning 2279 and quotient checking | changing | planned (M6); currently refused | Owner decisions 2026-10-08 and 2026-10-09; probes `SymLiveMissed`, `SymLiveSymProp`; D-005 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | implemented (`dive check --store exact`); ordered safety-frontier spill implemented (`--frontier-memory-bytes`); seen/exact-store and graph spill still planned (ADR 0024) | Charter rule 4; `crates/dive-check/tests/probes.rs` (both stores equal on every probe); D-007 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | implemented (`dive check --store exact`); separately budgeted predecessor spill implemented; seen/graph spill still planned (ADR 0031) | Charter rule 4; `crates/dive-check/tests/probes.rs` (both stores equal on every probe); D-007 |
| Structured results (`--output json`) | Human and `-tool` message streams; traces via `-dumpTrace json` only | Human-readable output | Versioned JSON: verdicts, stats, counterexamples as data, diagnostics, `options` block | preserving | implemented for parse/analyze/check and trace output; explicit final/short temporal reports use schema 2 | Charter ergonomics; D-008 |
| In-process library API (`dive-api`) | JVM process only | n/a | Embed checking, simulation and trace validation in Rust and `cargo test` | preserving | implemented for front-end, safety, final temporal and independent trace APIs; finite pure safety simulation added with independent validation; broader modes remain partial | Charter ergonomics |
| Canonical `CHOOSE` order (`--choose-order=canonical`; TLC mode `=tlc`) | TLC picks the first satisfying element in its internal value order. Strings are ordered by the *interning order* of their tokens in the module text, so an unused definition `Unused == "alpha"` above `Init` changes `CHOOSE e \in {"zeta","alpha","mid"} : TRUE` from `"zeta"` to `"alpha"`. Non-empty sets order by size first, tuples by length first | **Canonical order** (owner-approved default, 2026-10-08): text-independent, restoring metamorphic invariance under reordering definitions | `--choose-order=tlc`, part of `--compat=tlc`: TLC's order exactly, including interning order, proven against TLC | changing | implemented in `dive-value` (`ValueOrder::Canonical` default, `ValueOrder::Tlc`; ADR 0012); the interner is fed in SANY/TLC's encounter order by `dive-model` (ADR 0012, "For the front end") | `docs/survey/competitive.md` probes P2 and P7, in `conformance/corpus/probes/` (`P2Choose_str` is a registered expected divergence of the default profile); `crates/dive-value/tests/fixtures/specs/ChooseIntern*`, `PrintChoose`, `PrintSets`; D-006 |
| Output ordering (follows `--choose-order`) | Sets, record fields and function domains print in TLC's value order, so names (strings, model values, field names) print in intern order: `[qfb \|-> 1, qfa \|-> {1}]` when `qfb` is lexed first, `{"qsc", "qsb", "qsa"}` in order of first appearance | Canonical order (the `CHOOSE` order) | `--choose-order=tlc`, part of `--compat=tlc`: TLC's printing order exactly | changing (output text only, never a verdict) | implemented in `dive-value` (ADR 0012) | `crates/dive-value/tests/fixtures/specs/Print*`; D-006 |
| TLA+ comparisons (`--comparisons=tla`; TLC mode `=tlc`) — **default** (Owner decision 2026-10-08; D-009) | TLC refuses some comparisons that TLA+ answers: `[a \|-> 1, b \|-> 1] = <<1>>` is an error while `<<1>> = [a \|-> 1, b \|-> 1]` is `FALSE`; `<<1>> \in [a : {1}]` is an error; whether `{<<1>>, [a \|-> 1, b \|-> 1]}` is an error depends on the order of its elements; `1 \in {1, "a"}` is `TRUE` but `"a" \in {1, 2}` is an error; a record literal refuses `= <<1>>` while the same function built with `:>`/`@@` gives `FALSE` | **TLA+ comparisons** (owner-approved default); set construction still follows TLC's rules | `--comparisons=tla`: answer `=` and `\in` between functions, records, tuples and sets as TLA+ defines them, symmetric and order-independent; keep the error only where TLA+ leaves the answer unspecified (`1 = "a"`) | changing | implemented default in `dive-eval` as `Comparisons::Tla`; TLC comparisons built and tested under `--compat=tlc` | `crates/dive-value/tests/fixtures/specs/Asym*`, `ErrMem*`, `PrintMember`, `RvFcnRcd*`, `RvCanonAsym`; `crates/dive-eval/tests/options.rs`; D-009 |
| Exploration order (`--explore-order=value`; TLC mode `=tlc`) — **canonical by default** (Owner decision 2026-10-08; D-010) | Sets are explored (`\E`, `\A`, filters, maps, function constructors, `x' \in S`) in TLC's value order, so the module text decides which of two violations at the same depth is reported | **Canonical order** (owner-approved default) | `--explore-order=value`: explore in the run's value order, canonical in the default profile, making that profile independent of the module text | changing (which violation is reported, and the counts at the stop) | default (owner decision 2026-10-08) in `dive-eval` (`ExplorationOrder::Value`); `dive check` notes at every default-profile stop that the result may differ under `--compat=tlc`; detecting whether the order mattered is planned | Probe `StringOrderViolation` (TLC reports `InvZeta` after 2 states); `crates/dive-eval/tests/options.rs`; Examples `MissionariesAndCannibals` (226 generated vs TLC's 225) and `acp/ACP_NB_WRONG_TLC` (12,645 distinct vs 11,852), same violation, property and depth, registered in `conformance/expected-divergences.toml`; D-010 |
| Independent counterexample validation (`dive validate-trace`; every `dive check` counterexample) | TLC prints its counterexamples unchecked, and `-dumpTrace json` cannot be read back: sets and sequences are both arrays, strings and model values both strings | Every counterexample is replayed against the spec by a separate validator that shares no search code with the checker; a failure is an internal error, never a verdict | `dive validate-trace` checks any trace, including TLC's own `-dumpTrace json` and a Rust program's implementation traces (`dive_api::trace`) | preserving (never changes a verdict) | implemented in `dive-trace`, `dive-api`, `dive validate-trace` and the harness (ADR 0016 amendments 1–2); bounded independent deadlock/fairness enabledness; `dive check` wiring with WP-D2 | `dive-conformance trace-controls`: 12 of 15 TLC safety counterexamples valid, 3 refused by name, every mutant rejected; `crates/dive-api/tests/trace_controls.rs`; D-012 |
| Logical worker count (`TLCGet("config").worker`) | TLC reports its physical worker count, so specs asserting `worker = 1` (e.g. `MCDieHardest`) refuse to run in parallel | **1 (logical) at any `--workers`** (owner decision 2026-10-08) | `--compat=tlc`: physical count, as TLC reports it | changing (only for specs that read it) | implemented (`dive-check` `run_config`) | D-011; harness worker-invariance report |

## Oracle facts behind the registry

- **Introspecting specs.** Specs can legitimately depend on `TLCGet("config").worker`. The owner-approved default reports logical 1; `--compat=tlc` reports the requested physical count, matching TLC. `MCDieHardest` therefore runs in the default profile at any width but fails its assumption under multi-worker compat mode. Physical threads remain visible in the result (D-011).

- **Finite temporal prefixes (D-008).** `QtEmpty` prints temporal violation 2116
  and a finite initial-state trace, but exits with safety status 12 rather than
  ordinary temporal status 13. The harness preserves both facts and verifies
  the specific complete stream shape. TLC-exact phase-L checker behavior remains
  pending; this parser correction does not enable a new checker default.

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

## Successor-generation artefacts (candidates)

`dive-eval`'s successor generator reproduces TLC's enumeration rule by rule
(`crates/dive-eval/src/next.rs`). These TLC behaviours look like artefacts rather than
intended semantics. dive matches TLC on each, or refuses where TLC's answer depends on
history it does not track. Nothing different is implemented. Fixtures and TLC's
transcripts are in `crates/dive-eval/tests/fixtures/next/`.

| Case | TLC behaviour |
|---|---|
| `NxInLazyCup` | `r' \in [a : {1}] \cup [a : {1, 2}]` generates `[a \|-> 1]` twice per state (10 generated, 3 distinct; 7 for `[a : {1, 2}]`). The count also depends on whether TLC already normalized and cached the same lazy set object. dive refuses the case. |
| `NxFastPath` | The all-assigned shortcut runs only while warnings are on, so `-nowarning` changes "states generated": 7 by default, 37 with `-nowarning` (4 distinct either way). |
| `NxForallOrder` | `\A` in an action visits its first binding, then the others last first. |

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

## Approved M6 additions (2026-10-09)

| ID | Capability | Intended default | TLC counterpart | Status and evidence |
|---|---|---|---|---|
| D-013 | Liveness counterexample selection | `--lasso=short`: shortest prefix, deterministic short cycle | `--lasso=tlc`, included in `--compat=tlc` | partial implementation: bounded final Rust API with WF/SF, finite constant-domain quantifiers, pure constant/state-argument operators and independent scoped ENABLED covers, original property claims and independent validation; explicit final/short CLI access with mixed-profile reports; bounded ordered parallel capture with 1/4/8/chaos controls; explicit bounded final-only pinned selection now exposed through schema-2 API/CLI with fresh independent replay; ordinary periodic and remaining recovery/error boundaries stay refused. ADRs 0017–0018, `LvNoFair`, `LvTwoLoopsRev`, fingerprint series |
| D-014 | Periodic liveness scheduling | `--lncheck=deterministic`: documented level-boundary checks | `--lncheck=default` with TLC's wall-clock trigger in compat mode; `final` for goldens | planned; `LvPeriodic` stopped at 418,795, 167,402 and 162,021 distinct states |
| D-015 | Malformed symmetry declarations | Reject non-bijective functions | TLC acceptance under `--compat=tlc` | bounded checked/TLC closure and pure safety reduction implemented; default non-bijections refuse before search, compat reproduces SyNonBiject; staged startup warnings match source-backed probes; liveness/randomness and unverified declaration/value shapes still refused (ADR 0019) |
| D-016 | Checking declared symmetry | Audit only with `--check-symmetry` | No audit, as in TLC | planned; `SymAsym`, `SymAsymLate` |
| D-017 | Host access from a specification | No access without explicit category grants | Explicit grants also required under `--compat=tlc`; restrictions are reported | planned; existing named refusals, CommunityModules `AllTests`, Examples `TLC/TestGraphs` and `ewd998/EWD998ChanTrace` |

## Temporal constructor restrictions (candidate, D-013)

The pinned TLC accepts the syntax but cannot check temporal CASE (`CdCaseOverlap`,
2213, exit 255) or an action-guarded temporal IF (`CdAction`, 2214, exit 255).
Minimal specs and raw results are in
`crates/dive-api/tests/fixtures/temporal-conditionals/`. Pure state- and
temporal-guarded IF works. Broader support may become a named TLA+ formula option;
it is not an adopted default. Both checker profiles retain explicit refusals.

D-013 finite-predicate coverage now includes constant-domain quantified actions
and state predicates. Six clean `action-quantifiers` probes match final truth/counts;
independent ENABLED uses necessary covers rather than the checker enumerator.
Exact TLC temporal ordering, timing and selection remain pending.

D-013 scoped predicate coverage includes pure LET values/operators and
independent covers over their captured contexts. Six clean `temporal-let`
probes match final truth/counts; exact TLC temporal selection/timing remains open.

D-013 INSTANCE predicate coverage includes pure value/variable substitutions,
nested contexts and priming. Six pinned `Is*` probes match final truth/counts
in both profiles/stores. Operator substitutions and independent substituted
disabledness proofs remain refused; strict TLC temporal selection is pending.

D-013 finite-data coverage includes constant-domain set maps, filters and
function constructors with retained local contexts. Seven pinned `Bc*` probes
match final truth/counts in both profiles/stores; bounded state-dependent
predicate domains are now supported, while effects remain refused. Strict TLC temporal selection/timing remains pending.

D-013 independent validation now supports finite constructor bindings in necessary
ENABLED covers (`constructor-covers` probes); primed domains, uncovered reads
and bounds remain named refusals. No strict temporal compatibility claim changes.

D-013 state-dependent predicate domains are read at each concrete state/edge,
with pure symbolic bindings only for static level/purity analysis. Eight `Df*`
probes match final truth/counts in both profiles/stores, and every violation
validates independently. Infinite/oversized concrete domains and dynamic
temporal quantifier expansion remain named refusals.

D-013 finite predicate coverage includes bounded CHOOSE and pure Min/Max-style
operators over constant or state-dependent domains. Domain levels are retained
even when bodies ignore their binders. Five `Cf*` probes match final truth/counts
in both profiles/stores; existing canonical/TLC choice ordering is unchanged.

D-013 pure value/action helpers now retain action-level caller arguments (for
example `IsStrictPrefix(log, log')`). Four `Aa*` probes match final truth/counts
in both profiles/stores with independent periodic validation. Temporal-helper
action arguments, temporal-valued/higher-order forwarding and effects remain
refused; this does not enable strict TLC temporal checking.

D-013 bounded final/short fairness uses separate recurring-edge conditions and
reachable products, without increasing resource defaults. Three five-process
`fairness-factoring` probes match final truth/counts in both profiles/stores;
fresh independent validation rejects an unfair stuttering mutant. DiningPhilosophers
also matches; the remaining LM1 refusal names VIEW graph capture. Exact TLC
planning, scheduling, lasso choice and parallel capture remain pending.

### Empty symmetry declarations (candidate)

The pinned TLC normal CLI accepts SyEmpty in SANY, then exits 255 with
GENERAL and `IllegalArgumentException` for `SYMMETRY Sym`, `Sym == {}`.
Its direct closure kernel throws for zero or one supplied function. The new
bounded kernel returns `TooFewGenerators`; binding/reduction remain refused.
No alternative empty-declaration default is adopted. Raw evidence:
`crates/dive-model/tests/fixtures/symmetry-group/` and the corresponding
under-consideration entry in `docs/tlc-differences.md`.

D-013 finite lazy-domain bindings now support bounded `SUBSET S`, `[S -> T]`,
record sets and Cartesian products, including nested constructors and the
singleton empty-domain function with an infinite range. Eight pinned `Ld*`
models match final truth/counts in both profiles/stores at requested workers
1/4/8; actual final capture remains sequential. Preflight preserves raw operand
normalization order and bounds component and generated value slots. Deferred,
infinite, oversized and excessive-work domains remain named failures. This does
not change the strict temporal compatibility refusal.
## D-018: DFID depth exhaustion is incomplete

The pinned `-dfid N` is a maximum level, despite the CLI describing an initial
depth. `DfidBound` (`Init == x = 0`, `Next == x' = x + 1`) exits zero at
`-dfid 2` after 2 generated/2 distinct and at `-dfid 4` after 7/4, without
completion message 2193. Native DFID reports a depth-bound stop as
incomplete with no exhaustive-proof claim. `--compat=tlc` retains the measured
TLC exit status while exposing the incomplete termination; the default uses an
explicit incomplete disposition. ADR 0021; clean controls under
`crates/dive-check/tests/fixtures/dfid/`.

**D-002 DFID replay finding:** the same seed 7/aril 3/fingerprint 0 yields
both `0,1,3` and `0,2,4` on `DfidBranch`. `DFIDWorker` self-seeds its Java
generator instead of using the CLI seed. Native default replay is implemented;
TLC mode reports all actual worker seeds and distinguishes replay overrides.

## D-019: DFID cross-root witness prefix

Pinned `DFIDWorker.run` leaves `curLevel` from the previous initial root when
`doNext` checks the next root; `printInvariantTrace` walks that stale stack.
`DfidRootsMixed` clean controls emit both the valid `1,2` and truncated `1`
witness for the same invariant violation. The truncated word does not exhibit
the violated invariant. Native DFID keeps the actual concrete root/path and
certifies it independently. TLC printed-witness reproduction is a named
unsupported display mode, never a validated witness. Raw primary observations
are retained; structured native trace selection does not claim equality in this
case. See ADR 0021 and [D-019](tlc-differences.md#d-019-dfid-can-truncate-a-counterexample-after-switching-initial-roots).


D-013 bounded deferred-domain support admits closed pure captured filters over
finite structural inputs and maps over those filters. Six clean `finite-deferred-audit`
probes match final truth/counts in both profiles/stores with independent periodic
validation. Preflight bounds input and conservative output work before predicates
run; cached results do not bypass the original input bound. Infinite/opaque/
effectful domains and recursive scoped functions still refuse. This is staged
F-007 evidence, not strict temporal certification; F-006 recursive scope support
remains open. The original full ten-row study retains its nine refusals/bounds
and one timeout until a new immutable whole-selection report is completed.


D-013 filter admission additionally refuses opaque captured Rust operator
arguments and unproved captured thunk/function effects before invocation. This
preserves the F-006 gap; IR call-site effects alone do not prove operator values
supplied dynamically through the Rust scope API pure. The native no-output
regression and retained pre-fix repro are implementation controls, not TLC
conformance evidence.

D-013 retained temporal expression arguments now support direct/local calls and
forwarded nested substituted bodies in the explicit final/short path, including
supported ENABLED expressions. Raw controls and the complete native matrix live
in `conformance/probes/temporal-forwarding/`. Exported INSTANCE temporal-formal
calls rejected by the pinned oracle remain named refusals; recursive/higher-order
scope support remains partial. No result-changing default or resource limit changed.

D-013 temporal endpoint checks also refuse missing scoped expression/definition
metadata without panics. Public evaluator construction and scope admission now reject malformed IDs,
redirect signatures/cycles and foreign captures before interning or callbacks
(ADR 0028, #129). Bare indices still cannot prove foreign provenance, and this
does not certify arbitrary caller-supplied semantic annotations; F-035 stays
partial. The same malformed-input checks apply in both compatibility profiles.

D-013 planning progress now includes a bounded, separate TLC normalization and
pre-tableau grouping projection (ADR 0025, `tlc-planning` controls). It preserves
predicate occurrence tags, exact DNF encounter order, recurrence collection,
EA/AE classification and first-encounter OOS/PEM bins. Original property names
remain available independently of combined DNF. This is diagnostic planning
support only: no new default or strict integrated-liveness
claim. Existing opaque/scope/effect and construction-limit refusals remain.

D-013 diagnostic planning now also projects bounded TLC particle/tableau
numbering and promise identities (ADR 0027). Actual pinned graph fields and
independent periodic/closing-witness controls cover this artifact; it never
reports a temporal verdict. Empty/nonpositive direct structures, actions,
unchecked group/tag/bin metadata, opaque/effectful scopes and resource limits
remain named refusals. Strict integrated TLC liveness, phase-L timing, SCC and
witness selection remain unsupported by this stage. Original configured/Action
claims and derived role names stay separate; no new default is adopted.



### PlusCal translator artifact admission (candidate/current limitation)

Clean pinned `pcal.trans` exits 0 for a sequential variable named `Next` or
`vars`, but SANY rejects each generated module. All five SANY-invalid outputs
in the complete translation matrix remain visible. Native generation refuses
unproved generated-name/binder collisions in both profiles and admits an
artifact only after complete-module syntax/name/level analysis. No alternate
naming default is adopted. Minimal example, raw evidence and pending options
are recorded in `docs/tlc-differences.md` and `docs/pluscal-translation.md`.

D-008 public Rust admission additionally checks model/evaluator ownership at
independent proof entry points. Opaque deferred bindings refuse without invoking
caller callbacks; native captures are inspected structurally. Original panic and
pre-interning observations are retained in
`crates/dive-api/tests/fixtures/public-context/`. These are native safety controls,
not TLC parity certification or a new result-changing default.

The sequential normalization follow-up matches pinned automatic-label option
defaults, default model-value initialization and sequential label modifiers;
[its full selection](pluscal-normalization.md) retains branch/name refusals. No
alternate label/fairness default is introduced. Process/procedure/macro and
historical/checksum/map contracts remain pending.

D-013 diagnostic phase-L receipts now retain concrete TLC successor slots and
logical capacity history, with ordered null-tableau bins and separate initial
consistency (ADR 0029). All receipts say graph_insertion=false. Tableau graph
revisitation, wider diagnostic execution and unproved state-kind IF planning
remain explicit refusals; strict integrated liveness remains unsupported. The
original native error-order gaps (#147/#149) are retained and receive no match credit.

D-013 planning/error-order update (#147): `[]<>(IF x = 1 THEN 1 \div 0 = 0
ELSE x = 2)` raises TLC code2179 before any exploration, even if a reachable
state would not select the erroneous branch. The TLC-shaped native planner now
preserves that error and the reference's distinct user-call/boolean fallbacks.
No alternate default is adopted: independent formula evaluation keeps its
existing semantics, and strict integrated liveness remains unsupported. Complete
24 current controls, original18 and the native false completion are retained in
`conformance/probes/state-if-planning/`. That capture predates the bounded
[pure state-IF admission](tlc-state-if.md) in explicit final TLC mode (#168);
ordinary periodic scheduling and upstream-refused temporal-argument IF forwarding remain refused.
This is a native limitation repair, not family certification.

The [single-process increment](pluscal-processes.md) matches unfair constant-ID
and finite-membership process storage/action semantics and the distinct process
`-label` default. Subsequent bounded fairness and multiple-block support is
described below; unproved names remain named refusals.
Constant-ID `self` references that leave the primary output SANY-invalid are
retained without artifact credit; no alternate mathematical default is adopted.

D-013's next staged graph path now consumes concrete source receipts into actual
native graph nodes for one null-tableau OOS. The safety pool retains original
successor values and reports its true execution width; graph/predicate commits
run inline in logical source order. Twelve complete pinned graph projections
match fields at widths 1/3/4/8; all fifteen selected rows and clean TLC outcomes
are preserved in `conformance/probes/tlc-phase-l-graph/`. Resource and opaque-scope
refusals are explicit. This is graph evidence, not strict temporal verdict or
TLC stopping/SCC/lasso/scheduling credit; #140 remains open.

Checkpoint evidence for D-007 is preserved in
`conformance/probes/native-checkpoint/README.md`. TLC recovery resets counters;
original totals are an opt-in proposed native capability, not an approved default.
An incomplete native pause must never report a complete model-checking verdict.

The [bounded process-fairness increment](pluscal-fairness.md) adds weak/strong
declarations, label guards and explicit pinned fairness options. Translation
artifacts remain nonproof results. Unproved/overlapping process domains, procedure fairness, renaming
and unproved domain normalization are still named refusals; full-family
certification remains pending.

D-008 also records a pinned PlusCal fairness line of 87 columns under the
default 78-column setting. Native generation currently preserves the measured
reference junction grouping in both profiles; a width/layout improvement remains
pending. The minimal source and raw output are retained in the fairness corpus.

D-008 public Rust admission also covers ordinary/native checker startup and
bound graph capture (#157): malformed arena IDs and foreign predicate/action
contexts refuse before effects; error-report metadata cannot index an invalid
variable declaration. Stable context codes and original panic evidence are
retained in `conformance/probes/public-ir-admission/`; F-035 stays partial.

The [multiple finite-process increment](pluscal-multiple-processes.md) proves
per-block storage, label and fairness ownership for disjoint literal domains.
Overlapping IDs, namespace rewrites and cross-process local references remain
explicit refusals in both profiles. Full translation families remain partial.


Pure supported VIEW and safety symmetry now compose with D-007 native recovery
(ADR 0038). Saved visited keys are recomputed through the configured search
path, while actual concrete discoveries/predecessors remain unchanged. The
separate concrete witness validator rejects altered equivalent-key Init/Next
states. Runtime/graph/liveness recovery remain refused; no default
or independent archive-certificate claim changes. Captures and the initial
unused-standard-body refusal are preserved in
`conformance/probes/native-checkpoint-keys/README.md`.

### D-008 progress: native PlusCal artifact admission


The bounded [native macro translator](pluscal-macros.md) retains all invalid
primary outputs as diagnostic evidence. Under D-008, public CLI/API module
admission rejects them in both profiles and never grants artifact/proof credit.
Lexical macro substitution itself remains reference-compatible; general binders,
procedure interactions and name rewrites remain explicit refusals. No new
capture-avoidance or Unicode-string default is adopted.



The recursive procedure selection adds a D-008 artifact-admission case: a
zero-parameter same-procedure adjacent call/return produces a blank conjunction.
Pinned `pcal.trans` reports success, but SANY rejects the generated module.
`Recursive_C_self_tail_no_params` retains the raw source/output and rejection
before native changes; the full P-syntax selection is retained too. This is
candidate evidence, never an admitted artifact or finite-loop proof. Native
public admission keeps invalid primary output rejected in both profiles; no
silent repair/default is adopted.

### D-013 historical increment: ordered temporal execution

The earlier staged source records this scope; the later explicit final CLI
contract is described in [the current user guide](tlc-final-api.md).

The bounded Rust execution path now commits real multiple-OOS/tableau graphs in
source order, with initial consistency before initial invariants and immediate
done-state re-enumeration. Fifteen completed pinned graph-field cases match at
1/3/4/8 workers; all 26 cases and eight clean TLC stopping controls are retained
in `conformance/probes/tlc-phase-execution/`. Logical prefix counters, speculative
batch emissions and recursive enumeration work remain separate. Independent
validation rejects altered words, closing edges and incomplete property claims.

This remains intermediate: accepting-sink witnesses, exact TLC SCC/lasso choice,
initial error replay and periodic checking are not certified. Ordinary strict
liveness stays refused; no defaults or fixed budgets increase. F-020/F-021/F-022
remain partial/audit-required, F-023 missing and #140 open. Previous complete
studies keep their original outcomes and receive no new corpus verdict credit.

### D-013 historical library increment: pinned final-only SCC and witness selection

This section preserves the first library producer and its selected controls.
The later CLI exposure does not retroactively change that receipt.

The bounded `dive_api::tlc_temporal` Rust mode now constructs its own ordered
graph and reproduces TLC's deciding component, greedy postfix and fingerprint-
bucket closure. It retains configured claims separately from derived role names.
At 1/3/4/8 workers, 19 completed pinned controls match their SCC/witness fields;
all 30 controls, including 11 uncredited failures, remain available in
`conformance/probes/tlc-final-check/`. Fresh independent validation accepts the
supported original words and rejects changed Init, Next/closing edges, loop
targets and incomplete claims. A 4,096-graph independent census checks acceptance
and selected path labels.

This is an explicit final-only TLC/fingerprint library mode, not ordinary strict
CLI parity or periodic-schedule support. Exact-store temporal proof is refused:
a fingerprint graph must not silently merge concrete states while claiming exact
identity. Accepting-sink reconstruction, initial error replay, symmetry/VIEW/ALIAS,
periodic hooks, unsupported scopes/effects and fixed resource bounds remain named
limitations. Holds results retain collision qualifications; backend candidates
never carry their own validation certificate. F-019 through F-022 remain partial/
audit-required, F-023 missing and #140 open. Old broad studies retain their original
outcomes; no default or ordinary strict conformance credit changes.

See [final-only Rust API guide](tlc-final-api.md) for examples, resolved schedule,
actual evidence and refusal boundaries. The API admits startup sources before
binding creates a host; effectful ASSUMEs, unused top-level constants, config
replacement candidates and hooks return named refusal with zero used workers.
Standard-module classification does not bypass this admission. Explicit sequential
requests/fallback execution remain refused until their observer path is certified.


**Explicit final/TLC integration update:** `dive check --compat=tlc --lncheck final
--lasso tlc` now exposes the independently certified bounded final-only path.
Actual counts/worker/stage metadata and original property identities are retained,
including TLC's stuttering marker. This does not enable periodic checking or lift
exact-store, accepting-sink, recovery, effect or resource refusals. The separate
`dive-final-tlc` harness mode requires a final-schedule golden and fresh independent
replay; ordinary schema-1 results and earlier studies are unchanged. Configuration
alias targets are admitted through actual module contexts before any startup
binding effect. See [the user guide](tlc-final-api.md).


### D-013 progress: explicit final-mode safety-only routing

A freshly bound model with no implied temporal obligations now routes through
bounded ordinary safety checking in explicit final/TLC mode. This checks real
invariants, initial/action properties, constraints and supported deadlocks;
absence of a temporal obligation is not a fabricated NoOOS/product result.
Finite witnesses require fresh independent in-memory and JSON replay. Reports
name `bounded-safety-only`, preserve actual stopping counts and worker facts,
and state `temporal_certificate: false` and `product_constructed: false`.

The same entry/work ceilings stay active. They bound retained graph data, not
aggregate memory: a graph-state limit of three on `x'=x+1` refuses while the
engine truthfully records the deciding fourth inserted state. It never reports
success or completed capture. Constraint/property combinations whose stopping
order the independent validator has not proven remain explicitly refused before
pool work. No ordinary schedule, liveness-family or broad corpus credit follows.
Original all-refusal studies and the constant `Next == FALSE` binding rejection
are preserved. See [the safety-only user contract](tlc-final-api.md#safety-only-route)
and `conformance/probes/tlc-final-safety` for clean paired evidence.

D-013 conditional-admission progress (#168): pure state IF atoms are admitted
through the corrected shared planner and independent final-word replay. This
removes the older duplicate syntax refusal without changing conversion/error
order, resource defaults or ordinary periodic scheduling. All original refusal
and failed-capture histories remain available. Effects, opaque bindings and
unverified errors remain explicit; no whole-family certification is claimed.
See [conditional user guide](tlc-state-if.md), ADR 0036 and
`conformance/probes/tlc-state-if/`.

Temporal-kind IF remains a named bounded final-mode refusal (#170). Its
shared-guard normalization/numbered-particle repair is now implemented as
[diagnostic language evidence](tlc-temporal-if.md), with actual final execution
and independent witnesses still pending. D-013 and temporal families stay partial.

The proven direct temporal-IF subset now runs in explicit final/TLC mode after
shared-object tag repair, exact particle/edge checks and fresh independent
in-memory/JSON witness replay. The unchanged full selected controls retain
upstream 2213/2214 errors and source-prefix 2116/exit-12 cases as named refusals,
without matching-error credit. Compound guards and INSTANCE claim/role identity
are separately checked. Existing arena/work ceilings and whole-arena forwarding
admission remain; no aggregate memory bound or periodic-family claim is added.
See [the user guide](tlc-temporal-if.md) and
`conformance/probes/tlc-temporal-if-final/`.


D-008 now includes native single finite process procedure ownership (ADR 0044).
No result-changing default is introduced: all owned call/return generation is
matched to the pinned reference. Explicit source `stack` stays unsubstituted;
generated stack operations index the owner. Invalid primary artifacts remain
public rejections; multiple blocks/renaming/fairness and strict #124 witnesses
remain pending, as documented in `docs/pluscal-process-procedures.md`.

A further D-008 candidate is a procedure/parameter named `ProcSet` in a process
algorithm. Four retained C/P primary translations exit 0 but their clean SANY
runs exit 4 because generated ProcSet conflicts with the source name. Native
public admission rejects these in both profiles; generation is diagnostic-only,
with no corrected-output option or naming default adopted. The minimal source
and exact disposition are in the D-008 user log and process procedure `names`
study, not credited as artifact matches.

D-008's later shared-procedure increment (ADR 0045) admits multiple disjoint
finite process blocks with individual owned frames. Procedure definitions and
slots are generated once, with reference constant/procedure/member scheduling.
No mathematical default changes: both profiles preserve reference generation
and reject invalid enclosing artifacts. Scope rewriting, overlapping/unproved
domains, process-declared fairness and strict #124 replay remain explicit
limitations. See `docs/pluscal-multiple-procedures.md`; prior single-process
limitations above describe that historical increment.

The shared-procedure D-008 study also retains two constant-caller local `self`
initialization artifacts: pinned translation exits 0 but SANY exits 4 because
call actions leave `self` unbound. An empty P process similarly produces invalid
zero-exit output. Native public admission rejects these without adopting a
corrected-output default; all source/raw outputs and dispositions stay selected.
See the minimal user log examples and shared procedure acceptance manifest.

**D-008 PlusCal procedure fairness status:** native bounded finite processes can
now generate caller-owned weak/strong fairness for syntactically reachable
procedures, including process/procedure label modifiers and pinned fairness
options. The [fair procedure guide](pluscal-procedure-fairness.md) describes exact
use and admission. Generation is nonproof; strict runtime-name witness replay,
scoped renaming, unproved multiline fairness domains and sequential procedure
modifiers remain separately refused or pending. Original malformed modifier
controls and primary SANY-invalid artifacts remain uncredited historical evidence.


D-007 native pure safety recovery now uses the deterministic worker pool at the
requested width, with cross-width saved-level continuation and ordered counter-limit
stop evidence (ADR 0038 addendum). Boundary-zero pauses remain actual 1 before the
pool starts. TLC-reset/explicit-retain policies, archive trust/bounds and remaining
runtime/graph/liveness/spill refusals are unchanged. See
`conformance/probes/native-checkpoint-parallel/README.md`; no full F-032 claim.

D-007 native recovery additionally composes with the existing opt-in frontier
and predecessor budgets, independently or together. This removes a native
limitation; it is not a new TLC defect or result-changing default. Saved concrete
frontiers/parents are reconstructed into fresh transient stores in reference
order. Archive/discovery workspace is excluded from both budgets; no total RAM,
Java byte-interop or archive certificate claim. Counter reset/Retain and pure
VIEW/symmetry witnesses retain their existing semantics. See
[the runnable recovery guide](native-checkpoint.md#recovery-with-transient-disk-storage)
and [separate preserved study](../conformance/probes/native-checkpoint-spill/README.md).

## D-020: fair-algorithm checksum collision (candidate)

The pinned PlusCal translator computes `7c28162a` for every tested `--fair`
algorithm, because its ternary checksum expression hashes only `"fair"` and
omits the normalized AST. Changing `x=0` to `x=9` without changing the translation
leaves `Validator` reporting `NO_DIVERGENCE`. C/P primary bodies still have different
TLA CRCs. No native corrected default is adopted. Bounded sequential full AST
serialization now reproduces the reference CRC, with `fair_header_crc_only=true`
reporting this weakness. `algorithm_checksum_verified` means verified reference
CRC derivation, not source identity or proof. Unproved AST shapes remain named
refusals; `--rewrite --checksums tla` explicitly disables algorithm CRC. See D-020 in the
[user log](tlc-differences.md), D-008 for separate artifact admission, and
`crates/dive-pluscal/rewrite-acceptance/fair-checksum/` for all six primary/four
source-edit controls.

Bounded final scope evidence (#134) now distinguishes configured names from
local/formal/INSTANCE derived roles; see [the scoped final guide](tlc-final-scopes.md).
Exported INSTANCE temporal-formal and opaque/effectful combinations remain named
refusals. Original expanded-action attribution is still a missing native field,
not an approved result-changing default. Ordinary periodic scheduling remains
unsupported; the whole small-model study from the prior IF slice is unchanged.



D-007 native recovery admits pure runtime string/name constructors, preserving
saved encounter order and the typed actual inline fallback under TLC value or
exploration order. Canonical order may use the deterministic pool. Effects,
checker/register reads and independent TLC name-history witness proof remain
explicitly unsupported; no archive certificate or total-memory claim is added.
See `conformance/probes/native-checkpoint-runtime-names/README.md` and
`docs/native-checkpoint.md#pure-strings-and-recovery-encounter-order`.

D-008 checksum continuation retains strict generated-operator CST comparison,
including an initially missing empty-Else TRUE conjunct now reproduced exactly.
The checksum-only producer did not lift multiline-generation refusal. The later
bounded non-junction sequential generator is documented in `docs/pluscal-multiline.md`;
multiline junction/macro/process/procedure layouts and empty-Then generation still
refuse explicitly. All 320 primary controls and original
failures are in `crates/dive-pluscal/checksum-expression-acceptance/`. D-020
remains compatibility reproduction only; no stronger checksum default.

### Expanded temporal identity evidence (#134)

Completed bounded final TLC results now preserve the original name on each
expanded temporal Action separately from original configured claims and derived
role labels. This matches the pinned binder and adds no semantic divergence.
Conjunctive `PROPERTY P` can expose Action names `P;P` while the trace claims `P`
once. The complete18 source-bound selection retains two exported-formal failures
as refusals; ordinary schedules and broad family certification remain pending.
See [the identity guide](tlc-property-origins.md).

Callback-free captured concrete values now have a bounded matching admission
in explicit final mode; this resolves a native limitation, not a new divergence.
Stored representations, context/name authority, fixed recursive work/depth and
deferred/opaque refusals are documented in [the guide](material-capture.md).
No ordinary scheduling or whole-corpus certification is implied.


D-008 finite-process Full checksum continuation: declared-fairness records and
source-owned modifier lists have native post-Explode byte/CRC controls in
`crates/dive-pluscal/process-checksum-acceptance/`. Exact-CST/SANY/Validator
admission and finite model/witness stages remain separate. Unproved process
options/calls/name scopes stay Full refusals in both profiles. D-020 remains
reference reproduction only; no stronger checksum default or family certificate.

Pure operator-formal forwarding now has bounded explicit-final TLC admission;
this resolves a native refusal rather than adding a result-changing default.
Known user targets preserve caller scopes and are inspected even when unused;
opaque/native/recursive/higher-order and configured-origin-ambiguous targets
remain refused. Clean accepted INSTANCE operator-formal controls are separate
from older exported temporal-expression failures2213. Full periodic/corpus/family
parity is unclaimed; see [the operator guide](temporal-operators.md).

## D-021: PlusCal option context is absent from checksum headers (candidate)

A clean pinned `pcal.trans -wf`/`-sf` or automatic-label translation can give
`PCAL_DIVERGENCE_EXISTS` in a fresh unconfigured Validator, but `NO_DIVERGENCE`
with the same settings. Declared-fairness `-nof` and fair-header cases retain
separate measured outcomes. Source `PlusCal options (...)` comments do carry
context into the reference Validator; the source context continuation admits only proved fairness/label settings and
refuses unknown/malformed/conflicting contexts before generation/CRC instead of
silently ignoring their semantics.

No stronger checksum/default policy is adopted. Explicit typed/CLI options have
proved bounded process AST serialization; replacement reports record them.
D-020's fair-only CRC weakness remains. See [D-021 user evidence](tlc-differences.md#d-021-pluscal-checksums-need-the-generation-options),
[option use](pluscal-process-checksum-options.md), and all clean configured,
unconfigured and original ignored-option native captures in
`crates/dive-pluscal/process-option-checksum-acceptance/validator-context/`.

D-008 process checksum options update: supported explicit fairness and inserted
label settings now have stage-separated raw/full controls; unproved procedure/macro ASTs,
scoped renaming and unproved source-option declarations remain named Full refusals.
F-034/F-035 are still partial; CRC derivation, artifact admission and finite
model/witness checks do not certify the whole translator or liveness.

D-021 native masking repair: original bounded source declarations are checked
before hiding previous translation output. Full and body-only high-level
rewriting refuse deciding declarations overlapping old output, including
modified-block grants. Proved declarations outside old output are resolved by
the bounded source-context continuation. The retained
missing-fairness reproduction and current source-span regression are distinct
from reference CRC/Validator limitations. See the user log and issue #182.

The subsequent bounded [native operator actual slice](native-temporal-operators.md)
accepts Len/Head/Cardinality with separately proved constant/state arguments and
stored-input admission. It changes no result default or schedule. Other native,
configured/opaque/effectful and unsupported state-storage/error boundaries retain
named refusals; all20 clean probe outcomes remain accounted for separately.

D-021 source parsing study: pinned unknown/conflicting declarations can print
`Unrecoverable error`, exit 0, and write a partially configured translation.
Both primary syntax studies are retained under
`crates/dive-pluscal/source-option-semantics-acceptance/prestudy/` before native
implementation. Unknown/malformed/partial-error contexts remain named refusals;
no silent corrected default or unsupported-success credit is adopted.

### D-008 macro checksum stage qualification

Bounded nonrecursive macro Full serialization retains expanded reference AST
bytes separately from module admission. The original 240-row primary selection
has 28 translated/SANY-invalid artifacts; none earns artifact or proof credit.
Embedded ASCII-star decoration is now selected without source mutation, while
two C/P arbitrary trailing-prose native syntax
disagreements remain archived, with exact parser-consumed embedded end ownership
now matching the paired controls. Full procedural/scoped layout and multiline/junction actuals
remain explicit refusals. See [macro checksum scope](pluscal-macro-checksums.md).
D-020 fair-header-only CRC weakness remains unchanged in both profiles.


### D-008 embedded marker ownership repair

A minimal comment containing `--fair algorithm A` (assigning `x := 1`) and a
complete `--algorithm B` (assigning `x := 2`) on a later line exposed a native
selector defect: the pinned translator emitted `x' = 1`, while source5 emitted
`x' = 2` and admitted the module. Exact selector bytes in shipped main `fe35c73f5`
match the original base; the source5 runtime evidence is not retagged as a main
runtime. Original 16-case controls retain four raw and eight public wrong-answer
rows. The repair uses pinned line order, preferring unfair only when both markers
share the same line, in both profiles. Expanded split/tab/space and malformed
controls preserve real reference rejection boundaries. See the
[macro checksum study](../crates/dive-pluscal/macro-checksum-acceptance/README.md).
Artifact admission and CRC equality still never establish model correctness.

D-008 bounded DEFINE progress: [DEFINE helpers](pluscal-definitions.md) can be
serialized and emitted with reference declaration ordering. Raw AST/CRC equality
is separate from enclosing-module SANY/native admission and finite-model
checking. Invalid primary artifacts stay uncredited; recursive, symbolic,
action/temporal and unproved scope shapes remain named refusals. Neither profile
inlines helpers or silently repairs the reference output.

D-008 bounded LOCAL/function DEFINE progress: [the executable guide](pluscal-definition-forms.md)
preserves local export visibility, pre-state helper lookup and function domains.
Domains and bodies share the action/temporal/cycle guard. Exact raw CRC equality
is distinct from public enclosing-module admission and model proof; invalid
reference outputs remain uncredited. Both profiles keep these same semantics.
