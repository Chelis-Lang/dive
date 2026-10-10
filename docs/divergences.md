# Divergences and enhancements

`dive --compat=tlc` matches the pinned TLC exactly and is proven against the full corpus. dive's default profile is TLC-compatible except where the owner has approved a better default. Wherever TLC's behaviour is a handcuff, dive builds a named option that does better. See [ADR 0010](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) for the rule. This file is the terse registry. Its user-facing narrative companion, [`tlc-differences.md`](tlc-differences.md), explains each entry with an example; every row here has a `D-NNN` entry there, kept in sync in the same PR. Every TLC handcuff found gets a row here, *before or with* its implementation.

**Kinds:**
- **preserving**: a result-preserving enhancement. It changes how dive gets the answer, never the answer. It may default to on once the harness shows equality with the TLC golden at every setting.
- **changing**: a result-changing option. It is opt-in unless the owner has approved it as the default. It is named, reported in the output's `options` block, and tested in both modes. Its TLC-matching counterpart is always built and is included in `--compat=tlc`.

**Status** is one of `recorded` (the design keeps it possible), `planned` (has a milestone), `implemented` or `default`.

## Registry

| Name (dive option) | TLC behaviour (the handcuff) | dive default | dive option | Kind | Status | Evidence / tests |
|---|---|---|---|---|---|---|
| Deterministic parallel search (`--workers N`, default `auto`) | Multi-worker BFS is only approximately breadth-first: depth and counterexample length vary with worker count, so exactness needs `-workers 1` (`TLCTrace.java`; tlaplus#883) | Exact BFS results identical to TLC's single-worker run | Any worker count gives the same verdict, counts, depth and chosen shortest counterexample | preserving | **implemented** (`dive check --workers N`, WP-D3: ADR 0015's three-phase level engine; specs using `TLCSet`, live `TLCGet`, randomness, I/O or runtime name constructors under TLC order run in reported sequential mode (#122)) | ADR 0004; harness worker-invariance runs at 1, 3, 4, 8 workers and chaos; `crates/dive-check/tests/invariance.rs`; Examples `DieHard` (TLC depth 9 vs trace 7 at 4 workers); probes in `crates/dive-check/tests/fixtures/tlc-probes/` |
| Replayable randomness (`--seed`, seed always reported) | `RandomElement`/`RandomSubset`/`Randomization` are unseeded in exhaustive mode unless `-seed` is given; state counts vary per run. Draws in `Next` are seeded from the state fingerprint, whose polynomial is random unless `-fp` is given, so `-seed` alone does not replay them. A draw before `Init` can make TLC lose the counterexample (exit 12, no trace) | TLC-like: fresh seed per run, but the seed used is always reported | `--seed S` alone reproduces the run exactly at any worker count (streams keyed by seed, state fingerprint or trace number, and position); `--compat=tlc` reproduces TLC's draws bit for bit | changing (only when a seed is fixed) | implemented in `dive-stdlib` (`random`, both modes) and wired into `dive check` (`--seed`; start-up and per-state streams; replay as TLC does it) | Examples `SpanningTree/SpanTreeRandom.cfg`: 1.9k to 240k distinct states without a seed, stable with `-seed 0`, golden reproduced from the first six draws; `crates/dive-stdlib/tests/fixtures/random/` (six seeds, `RndTraceAssume`, `RndLetRedraw`); D-002 |
| Reproducible simulation (`dive simulate --seed`) | Simulation counts vary even with `-seed 0` and one worker. Each worker has its own generator for choosing steps, and every worker draws `RandomElement` from the same unreseeded `-seed` generator, so a trace's draws depend on which worker ran it | Same seed semantics as above | Bit-identical simulation from the same seed at any worker count: one stream per trace number | changing (only when a seed is fixed) | streams implemented in `dive-stdlib` (`Streams::simulation_trace`, TLC's `tlc_simulation_worker_seeds`); simulation planned (`dive-sim`) | Examples `EWD840_anim` (generated counts 267k to 271k across runs); `crates/dive-stdlib/tests/java_random.rs`, `dive_mode.rs`; D-003 |
| Arbitrary-precision integers (`--int=big`; TLC mode `--int=tlc`) | 32-bit ints; overflow raises `EC.TLC_MODULE_OVERFLOW` (2178); literals above 2^31−1 rejected (`Naturals.java`, `Integers.java`, `SpecProcessor.java`) | **Big integers** (owner-approved default; flips on once both modes pass the harness) | `--int=tlc`, part of `--compat=tlc`: 32-bit with TLC's overflow error, proven against full conformance | changing | implemented in `dive-value` (`IntMode::Big` default, `IntMode::Tlc`; ADR 0012); CLI flag and harness flip pending | Harness fixture `Overflow`; `crates/dive-value/tests/fixtures/specs/Ov*`, `Ar*` (messages matched byte for byte); probe `P1Overflow` in `conformance/corpus/probes/` (default profile: success, 3 states; registered expected divergence); owner decision 2026-10-08; D-004 |
| Sound symmetry with liveness (`--symmetry-liveness=unreduced`) | TLC checks a reduced graph and can both miss and invent violations | **Unreduced**, once implemented: disable symmetry reduction for the whole liveness run and report it | `=refuse` remains available; `=tlc` / `--compat=tlc` reproduces warning 2279 and quotient checking | changing | planned (M6); currently refused | Owner decisions 2026-10-08 and 2026-10-09; probes `SymLiveMissed`, `SymLiveSymProp`; D-005 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | implemented (`dive check --store exact`); disk spill planned (WP-D5) | Charter rule 4; `crates/dive-check/tests/probes.rs` (both stores equal on every probe); D-007 |
| Structured results (`--output json`) | Human and `-tool` message streams; traces via `-dumpTrace json` only | Human-readable output | Versioned JSON: verdicts, stats, counterexamples as data, diagnostics, `options` block | preserving | implemented for parse/analyze/check and trace output; explicit final/short temporal reports use schema 2 | Charter ergonomics; D-008 |
| In-process library API (`dive-api`) | JVM process only | n/a | Embed checking, simulation and trace validation in Rust and `cargo test` | preserving | implemented for front-end, safety, final temporal and independent trace APIs; simulation remains planned | Charter ergonomics |
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
| D-013 | Liveness counterexample selection | `--lasso=short`: shortest prefix, deterministic short cycle | `--lasso=tlc`, included in `--compat=tlc` | partial implementation: bounded final Rust API with WF/SF, finite constant-domain quantifiers, pure constant/state-argument operators and independent scoped ENABLED covers, original property claims and independent validation; explicit final/short CLI access with mixed-profile reports; phase-L/TLC-exact selection pending. ADRs 0017–0018, `LvNoFair`, `LvTwoLoopsRev`, fingerprint series |
| D-014 | Periodic liveness scheduling | `--lncheck=deterministic`: documented level-boundary checks | `--lncheck=default` with TLC's wall-clock trigger in compat mode; `final` for goldens | planned; `LvPeriodic` stopped at 418,795, 167,402 and 162,021 distinct states |
| D-015 | Malformed symmetry declarations | Reject non-bijective functions | TLC acceptance under `--compat=tlc` | checked/TLC closure kernel and pure declaration preparation implemented; full binding and reduction still refused; 1,329 direct kernel probes and SyNonBiject missed-invariant control (ADR 0019) |
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


### PlusCal translator artifact admission (candidate/current limitation)

Clean pinned `pcal.trans` exits 0 for a sequential variable named `Next` or
`vars`, but SANY rejects each generated module. All five SANY-invalid outputs
in the complete translation matrix remain visible. Native generation refuses
unproved generated-name/binder collisions in both profiles and admits an
artifact only after complete-module syntax/name/level analysis. No alternate
naming default is adopted. Minimal example, raw evidence and pending options
are recorded in `docs/tlc-differences.md` and `docs/pluscal-translation.md`.

The sequential normalization follow-up matches pinned automatic-label option
defaults, default model-value initialization and sequential label modifiers;
[its full selection](pluscal-normalization.md) retains branch/name refusals. No
alternate label/fairness default is introduced. Process/procedure/macro and
historical/checksum/map contracts remain pending.

The [single-process increment](pluscal-processes.md) matches unfair constant-ID
and finite-membership process storage/action semantics and the distinct process
`-label` default. Subsequent bounded fairness and multiple-block support is
described below; unproved names remain named refusals.
Constant-ID `self` references that leave the primary output SANY-invalid are
retained without artifact credit; no alternate mathematical default is adopted.

The [bounded process-fairness increment](pluscal-fairness.md) adds weak/strong
declarations, label guards and explicit pinned fairness options. Translation
artifacts remain nonproof results. Unproved/overlapping process domains, procedure fairness, renaming
and unproved domain normalization are still named refusals; full-family
certification remains pending.

D-008 also records a pinned PlusCal fairness line of 87 columns under the
default 78-column setting. Native generation currently preserves the measured
reference junction grouping in both profiles; a width/layout improvement remains
pending. The minimal source and raw output are retained in the fairness corpus.

The [multiple finite-process increment](pluscal-multiple-processes.md) proves
per-block storage, label and fairness ownership for disjoint literal domains.
Overlapping IDs, namespace rewrites and cross-process local references remain
explicit refusals in both profiles. Full translation families remain partial.

The bounded [native macro translator](pluscal-macros.md) retains all invalid
primary outputs as diagnostic evidence. Under D-008, public CLI/API module
admission rejects them in both profiles and never grants artifact/proof credit.
Lexical macro substitution itself remains reference-compatible; general binders,
procedure interactions and name rewrites remain explicit refusals. No new
capture-avoidance or Unicode-string default is adopted.
