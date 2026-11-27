# Divergences and enhancements

`dive --compat=tlc` selects pinned TLC behavior on supported paths, with named refusals for unfinished features; complete toolchain parity remains in progress. dive's default profile is TLC-compatible except where the owner has approved a better default. Wherever TLC's behaviour is a handcuff, dive builds a named option that does better. See [ADR 0010](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) for the rule. This file is the terse registry. Its user-facing narrative companion, [`tlc-differences.md`](tlc-differences.md), explains each entry with an example; every row here has a `D-NNN` entry there, kept in sync in the same PR. Every TLC handcuff found gets a row here, *before or with* its implementation.

**Kinds:**
- **preserving**: a result-preserving enhancement. It changes how dive gets the answer, never the answer. It may default to on once the harness shows equality with the TLC golden at every setting.
- **changing**: a result-changing option. It is opt-in unless the owner has approved it as the default. It is named, reported in the output's `options` block, and tested in both modes. TLC-matching settings are included where supported by nominal `--compat=tlc`; ADR 0153 documents the construction exception.

**Status** is one of `recorded` (the design keeps it possible), `planned` (has a milestone), `implemented` or `default`.

Ids are never reused. D-032 to D-035 were never assigned: the ADR 0162 design draft proposed D-031 to D-035, and the simulation entries were registered as D-036 to D-040 instead. D-025 is reserved (see [D-026](#d-026-run-budgets-stop-a-run-with-a-structured-result)).

## Registry

| Name (dive option) | TLC behaviour (the handcuff) | dive default | dive option | Kind | Status | Evidence / tests |
|---|---|---|---|---|---|---|
| Finite predecessor function equation (D-046) | TLC reports an unbounded CHOOSE on the published LuPastry+ routing table | Construct the unique solution over the finite integer domain in predecessor order | The TLC comparison profile reports the named unbounded CHOOSE refusal | changing | default on Core | ADR 0165 and `crates/dive-api/tests/core_prefix_function.rs` |
| Finite symbolic, compiled and occupancy safety (explicit or automatic opt-in, D-030) | Explicit state counts at termination | Core BFS | A completed symbolic search reports exact counts. Refutations report validated shortest witnesses and induction reports proof obligations. | changing evidence | opt-in | `scripts/symbolic/check_source.py` compares TLC goldens and concrete Core results |
| Forced Boolean formals (Core; D-047) | A formal already forced by an earlier conjunct is read as a cached Boolean in action position; otherwise its argument is expanded | Expand the argument in action position independent of demand order | Nominal compat keeps the same expansion and documents the count gap | changing | default on Core | Exact ForcedFormal count pair; UnforcedFormal agrees; unrelated observables remain required |
| Boolean recursive predicates (Core; D-029) | Action generation expands duplicate witnesses and can demand an already-unnecessary alternative | Evaluate effect-safe Boolean predicates and count emitted successors | Nominal compat retains Boolean evaluation and documents count/short-circuit gaps | changing | default on Core | Exact ActionCount/VoteProof count pairs and pinned OrError result; unrelated observables remain required |
| Canonical finite sets (default Core; D-028) | Set construction errors and membership can depend on prior normalization of the same TLC object | Canonical finite literal and mapped sets | Nominal compat keeps canonical producers and names unsupported comparison shapes | changing | default on Core | ADR 0153; `core_receiver_consumers` |
| Deterministic parallel search (`--workers N`, default `auto`) | Multi-worker BFS is only approximately breadth-first: depth and counterexample length vary with worker count, so exactness needs `-workers 1` (`TLCTrace.java`; tlaplus#883) | Exact BFS results identical to TLC's single-worker run | Any worker count gives the same verdict, counts, depth and chosen shortest counterexample | preserving | **implemented** (`dive check --workers N`, WP-D3: ADR 0015's three-phase level engine; specs using `TLCSet`, live `TLCGet`, randomness, I/O or runtime name constructors under TLC order run in reported sequential mode (#122)) | ADR 0004; harness worker-invariance runs at 1, 3, 4, 8 workers and chaos; `crates/dive-check/tests/invariance.rs`; Examples `DieHard` (TLC depth 9 vs trace 7 at 4 workers); probes in `crates/dive-api/tests/fixtures/checker/tlc-probes/` |
| Replayable randomness (`--seed`, seed always reported) | `RandomElement`/`RandomSubset`/`Randomization` are unseeded in exhaustive mode unless `-seed` is given; state counts vary per run. Draws in `Next` are seeded from the state fingerprint, whose polynomial is random unless `-fp` is given, so `-seed` alone does not replay them. A draw before `Init` can make TLC lose the counterexample (exit 12, no trace) | TLC-like: fresh seed per run, but the seed used is always reported | `--seed S` alone reproduces the run exactly at any worker count (streams keyed by seed, state fingerprint or trace number, and position); `--compat=tlc` reproduces TLC's draws bit for bit | changing (only when a seed is fixed) | implemented in `dive-stdlib` (`random`, both modes) and wired into ordinary safety checking, including actual-bound configured RandomElement; randomness combined with VIEW or SYMMETRY uniformly refuses before binding; non-VIEW configured native random targets use the ordinary per-state stream; temporal checking admits randomness fixed at start-up in constant-level definitions (validation of its violations still refuses); strict initial-predecessor replay, randomness drawn during graph capture and recovery randomness remain explicit refusals (#178/#180, D-002) | Examples `SpanningTree/SpanTreeRandom.cfg`: 1.9k to 240k distinct states without a seed, stable with `-seed 0`, golden reproduced from the first six draws; `crates/dive-stdlib/tests/fixtures/random/` (six seeds, `RndTraceAssume`, `RndLetRedraw`); D-002 |
| Reproducible simulation (`dive simulate --seed`) | Simulation counts vary even with `-seed 0` and one worker. Each worker has its own generator for choosing steps, and every worker draws `RandomElement` from the same unreseeded `-seed` generator, so a trace's draws depend on which worker ran it. `num` counts traces per worker, and the reported trace and counts depend on the worker count (`crates/dive-sim/tests/fixtures/temporal/oracle.json` C11, W1, W4; ADR 0162 §6 makes dive's result identical at any worker count) | Same seed semantics as above | Bit-identical simulation from the same seed at any worker count: one stream per trace number | changing (only when a seed is fixed) | streams implemented in `dive-stdlib` (`Streams::simulation_trace`, used in both profiles; TLC's `tlc_simulation_worker_seeds` is ported and tested but not used, so `--compat=tlc` does not reproduce TLC's stream); safety simulation implemented at any worker count with the lowest deciding trace id and a total `--traces` (`dive-sim`, ADR 0020, ADR 0162 §7, `crates/dive-api/tests/simulate_workers.rs`); spec random operators remain named refusals | Examples `EWD840_anim` (generated counts 267k to 271k across runs); `crates/dive-stdlib/tests/java_random.rs`, `dive_mode.rs`; D-003 |
| Implied initial properties in simulation (D-036) | `-simulate` never checks a state-level `PROPERTY` conjunct; BFS reports 2108 | Checked on every initial state, 2108, in both profiles | Nominal compat keeps checking and documents the gap | changing | implemented (ADR 0162, S1; `crates/dive-api/tests/simulate_safety_roles.rs`) | `crates/dive-sim/tests/fixtures/temporal/oracle.json` U12, B-U12 |
| Concrete loop closure in simulation liveness (D-037) | Loops close on view or symmetric fingerprints: a spurious counterexample under `VIEW`, a missed violation under `SYMMETRY` | Close only on exact state equality; VIEW and SYMMETRY do not affect simulation | Same rule under `--compat=tlc` (TLC's view lassos are not behaviors and cannot validate); documented gap | changing | implemented (ADR 0162, S3; `crates/dive-api/tests/simulate_temporal.rs`) | `oracle.json` V1, B-V1, Y2, Y3, B-Y2 |
| Temporal coverage of a sample (D-038) | Success is silent when no trace could decide a liveness property | Per-trace classes and per-property checked/unchecked counts; never reports `holds` | n/a | preserving (report fields only) | implemented (ADR 0162, S3; `crates/dive-api/tests/simulate_temporal.rs`) | `oracle.json` C7, U1 to U3, U9 |
| Extend truncated traces (`--liveness-extend`, D-039) | Every trace ends at `-depth` | `auto`: extend an undecided trace by up to `depth` steps (owner-approved default, 2026-11-20) | `0` under `--compat=tlc`; any explicit step count. The default profile also reports `bad-prefix` refutations of temporal properties, off under `--compat=tlc` | changing | implemented (ADR 0162, S3; `crates/dive-api/tests/simulate_temporal.rs`) | `oracle.json` C7, U1 |
| Sampling unbounded sets (`--sample-unbounded`, D-040) | Init or successor sets over `Nat` fail with GENERAL 1000 in `-simulate` and `-generate` | `geometric`: draw members of `Nat`, `Int`, `Seq(S)` and finite-domain tuples, records and functions over them with a stated, seeded distribution, named in the report notes with every set sampled; `\E` draws are recorded as step witnesses that the validator checks (owner-approved default, 2026-11-20) | `off` (and `--compat=tlc`): TLC's GENERAL 1000 error | changing | implemented (ADR 0162, S6; `crates/dive-api/tests/simulate_unbounded.rs`, `crates/dive-trace/tests/witnesses.rs`) | `crates/dive-sim/tests/fixtures/temporal/oracle.json` G3 to G6 |
| Constructed initial states (`--constructive-init`, D-041) | `-simulate` and `-generate` enumerate Init and check every initial state before sampling | Off: Init is enumerated once, as in TLC | Opt-in `--constructive-init`: each trace builds its initial state with randomized Init choices; an initial violation is found only if a trace constructs it | changing (opt-in) | implemented (ADR 0162, S2; `crates/dive-api/tests/simulate_workers.rs`) | `crates/dive-sim/tests/fixtures/temporal/oracle.json` G2 |
| Every simulation trace as a file (`--trace-dir`, `--trace-format`, `--trace-ids`, D-042) | `-simulate file=PREFIX` writes each non-violating trace unvalidated, named by worker, without `ALIAS`, under a module header that does not parse; the violating trace is only in `-dumpTrace json\|tla`; traces depend on the worker count | The report keeps the deciding trace and 10 others | `--trace-dir DIR` writes every trace, validated, as it commits in trace order, identical at any worker count, in dive's format or TLC's `json` or `tla` layout (with `ALIAS` records); `--trace-ids A..B` regenerates those traces. No `_TTrace_` exploration module is written | preserving (opt-in output) | implemented (ADR 0162, S5; `crates/dive-api/tests/simulate_trace_dir.rs`) | `crates/dive-sim/tests/fixtures/trace-formats` |
| Eventually-always properties in simulation (D-043) | `-simulate` reports `<>[]P` violated on a lasso or a stopped trace that satisfies it, when `P` is false before the repeating part; model checking reports no violation | Each property is checked on the trace's actual behaviors, in both profiles | Same rule under `--compat=tlc`; documented gap | changing | implemented (ADR 0162, S4; `crates/dive-api/tests/simulate_temporal.rs`) | `crates/dive-sim/tests/fixtures/temporal/oracle.json` E1, B-E1, T5, B-T5 |
| `RandomElement` in simulation (D-044) | `-simulate` draws `Init`'s `RandomElement` once per run, so every trace starts in the same state; a step whose draw disables `Next` is reported as a deadlock (2114), including a draw inside `ENABLED`; a draw in an invariant decides a violation (2110) on one draw | Each trace draws from its own stream keyed by trace, step, action and attempt, identical at any worker count and replayed by `--trace-ids`; `Init` draws per trace, so a print in `Init` prints per trace; the trace document records each step's draws and the start-up draws, and the validator checks each as a possible result of its operator (`RandomElement`, `RandomSubset`, `RandomSetOfSubsets`) without enumerating the set; a step whose draws admit no successor is drawn again, then the trace stalls, never a deadlock; a draw in an invariant, constraint, property or `ENABLED` refuses by name | Same rules under `--compat=tlc`; documented gap | changing | implemented (ADR 0162 §8; `crates/dive-api/tests/simulate_random_ops.rs`, `crates/dive-api/tests/simulate_random_init.rs`) | `crates/dive-sim/tests/fixtures/random-ops/oracle.json` N1, I1, E1, D1, D2, S1 to S4, R1, V1, B1, C1 to C3 |
| Simulation configuration record (`TLCGet("config")` under `dive simulate`, D-045) | `Simulator.createConfig` narrows `traces` (`workers * num`) to a 32-bit int, so an unbounded run reports `-workers` and a large total wraps; `aril` is always `"0"` because the record is built before `-aril` applies | TLC's fields and values (`mode` `"simulate"`/`"generate"`, `traces`, `depth`, `aril` `"0"`, `sched` `"random"`, no `fingerprint`); `traces` is the exact total, -1 when unbounded at any worker count; `worker` follows D-011; trace documents record the fields (`run`) for the validator when the spec reads the record; `--trace-ids` refuses to evaluate the record | `--compat=tlc`: `traces` narrowed to 32 bits as TLC does; its total is dive's `--traces`, never `num` per worker (documented gap) | changing (only for specs that read `traces` above 2^31 − 1 or unbounded at several workers) | implemented (`crates/dive-api/tests/tlcget_config.rs`) | `conformance/corpus/tlcget-config` CfgCheck, CfgSimulate, CfgGenerate (#1249) |
| Arbitrary-precision integers (`--int=big`; TLC mode `--int=tlc`) | 32-bit ints; overflow raises `EC.TLC_MODULE_OVERFLOW` (2178); literals above 2^31−1 rejected (`Naturals.java`, `Integers.java`, `SpecProcessor.java`) | **Big integers** (owner-approved default; flips on once both modes pass the harness) | `--int=tlc`, part of `--compat=tlc`: 32-bit with TLC's overflow error, proven against full conformance | changing | implemented in `dive-value` (`IntMode::Big` default, `IntMode::Tlc`; ADR 0012); CLI flag and harness flip pending | Harness fixture `Overflow`; `crates/dive-value/tests/fixtures/specs/Ov*`, `Ar*` (messages matched byte for byte); probe `P1Overflow` in `conformance/corpus/probes/` (default profile: success, 3 states; registered expected divergence); owner decision 2026-10-08; D-004 |
| Sound symmetry with liveness (`--symmetry-liveness=unreduced`) | TLC checks a reduced graph and can both miss and invent violations | **Unreduced**: disable symmetry reduction for the whole genuine temporal run and report it | `=refuse` remains available; `=tlc` / `--compat=tlc` requests quotient checking, currently refused | changing | unreduced/refuse implemented; TLC quotient still refused | Owner decisions 2026-10-08 and 2026-10-09; probes `SymLiveMissed`, `SymLiveSymProp`; D-005 |
| Exact state storage (`--store exact`) | Fingerprints only, with a probabilistic collision bound | Fingerprint mode, reporting the bound (charter rule 4) | Full-state storage with an unconditional exhaustiveness claim | preserving (strengthens the claim) | implemented (`dive check --store exact`) for safety and, on Core, temporal properties (ADR 0158); ordered safety-frontier spill (`--frontier-memory-bytes`) and separately budgeted predecessor spill implemented for safety; temporal checks refuse spilled or reclaimed storage by name; seen/exact-store and graph spill still planned (ADR 0024, ADR 0031) | Charter rule 4; `crates/dive-api/tests/core_default.rs`, `crates/dive-api/tests/core_liveness.rs` (both stores equal at 1 and 4 workers); small-tier harness runs with `--impl-option store=exact`; D-007 |
| Structured results (`--output json`) | Human and `-tool` message streams; traces via `-dumpTrace json` only | Human-readable output | Versioned JSON: verdicts, stats, counterexamples as data, diagnostics, `options` block | preserving | implemented for parse/analyze/check and trace output; actual final temporal reports use schema 2; ordinary safety remains schema 1 | Charter ergonomics; D-008 |
| In-process library API (`dive-api`) | JVM process only | n/a | Embed checking, simulation and trace validation in Rust and `cargo test` | preserving | implemented for front-end, safety, final temporal and independent trace APIs; finite pure safety simulation added with independent validation; broader modes remain partial | Charter ergonomics |
| Native source work (`--source-coverage`) | `-coverage` reports a logical action-context cost tree | Off, also its TLC-equivalent setting | Native expression/generator counters at exact source locations, independently of verdict/witness proof | preserving | not implemented on Core: `--source-coverage` is a named refusal; the reference-engine implementation was removed with that engine (#791); frozen until after v0.1 (ADR 0153, #910) | ADR 0146; API/CLI source-coverage controls; D-008 |
| Final logical costs (`--coverage 0`, TLC `-coverage 0`) | Final action-context invocation/enumeration costs | Off, equivalent to TLC without `-coverage` | Separate original-source calling trees for Init/Next action-context costs | preserving | not implemented on Core: `--coverage 0` is a named refusal; the reference-engine slice was removed with that engine (#791); frozen until after v0.1 (ADR 0153, #910) | ADR 0150; `tlc-coverage-primary`; complete pinned cost fixtures; D-008 |
| Canonical `CHOOSE` order (`--choose-order=canonical`; TLC mode `=tlc`) | TLC picks the first satisfying element in its internal value order. Strings are ordered by the *interning order* of their tokens in the module text, so an unused definition `Unused == "alpha"` above `Init` changes `CHOOSE e \in {"zeta","alpha","mid"} : TRUE` from `"zeta"` to `"alpha"`. Non-empty sets order by size first, tuples by length first | **Canonical order** (owner-approved default, 2026-10-08): text-independent, restoring metamorphic invariance under reordering definitions | `--choose-order=tlc`, part of `--compat=tlc`: TLC's order exactly, including interning order, proven against TLC | changing | implemented in `dive-value` (`ValueOrder::Canonical` default, `ValueOrder::Tlc`; ADR 0012); the interner is fed in SANY/TLC's encounter order by `dive-model` (ADR 0012, "For the front end") | `docs/internal/survey/competitive.md` probes P2 and P7, in `conformance/corpus/probes/` (`P2Choose_str` is a registered expected divergence of the default profile); `crates/dive-value/tests/fixtures/specs/ChooseIntern*`, `PrintChoose`, `PrintSets`; D-006 |
| Output ordering (follows `--choose-order`) | Sets, record fields and function domains print in TLC's value order, so names (strings, model values, field names) print in intern order: `[qfb \|-> 1, qfa \|-> {1}]` when `qfb` is lexed first, `{"qsc", "qsb", "qsa"}` in order of first appearance | Canonical order (the `CHOOSE` order) | `--choose-order=tlc`, part of `--compat=tlc`: TLC's printing order exactly | changing (output text only, never a verdict) | implemented in `dive-value` (ADR 0012) | `crates/dive-value/tests/fixtures/specs/Print*`; D-006 |
| Pure JSON object ordering (`Json!ToJson`, follows `--choose-order`) | Object strings can depend on construction and normalization history | Canonical object order under the existing value-order policy | TLC value order refuses objects with two or more keys and compound keys containing multi-field records rather than inventing history | changing (JSON strings can affect predicates) | bounded native conversion with focused controls; Array/Object and host I/O remain refused; newly created TLC-profile trace names retain #124 refusal | consumed Json fixture, `crates/dive-eval/tests/json.rs` and `crates/dive-trace/tests/runtime_names.rs`; D-006 |
| TLA+ comparisons (`--comparisons=tla`; TLC mode `=tlc`) — **default** (Owner decision 2026-10-08; D-009) | TLC refuses some comparisons that TLA+ answers: `[a \|-> 1, b \|-> 1] = <<1>>` is an error while `<<1>> = [a \|-> 1, b \|-> 1]` is `FALSE`; `<<1>> \in [a : {1}]` is an error; whether `{<<1>>, [a \|-> 1, b \|-> 1]}` is an error depends on the order of its elements; `1 \in {1, "a"}` is `TRUE` but `"a" \in {1, 2}` is an error; a record literal refuses `= <<1>>` while the same function built with `:>`/`@@` gives `FALSE` | **TLA+ comparisons** and canonical finite-set construction (ADR 0153; D-028) | `--comparisons=tla`: answer `=` and `\in` between functions, records, tuples and sets as TLA+ defines them, symmetric and order-independent; keep the error only where TLA+ leaves the answer unspecified (`1 = "a"`) | changing | implemented default in `dive-eval` as `Comparisons::Tla`; TLC comparisons built and tested under `--compat=tlc` | `crates/dive-value/tests/fixtures/specs/Asym*`, `ErrMem*`, `PrintMember`, `RvFcnRcd*`, `RvCanonAsym`; `crates/dive-eval/tests/options.rs`; D-009 |
| Exploration order (`--explore-order=value`; TLC mode `=tlc`) — **canonical by default** (Owner decision 2026-10-08; D-010) | Sets are explored (`\E`, `\A`, filters, maps, function constructors, `x' \in S`) in TLC's value order, so the module text decides which of two violations at the same depth is reported | **Canonical order** (owner-approved default) | `--explore-order=value`: explore in the run's value order, canonical in the default profile, making that profile independent of the module text | changing (which violation is reported, and the counts at the stop) | default (owner decision 2026-10-08) in `dive-eval` (`ExplorationOrder::Value`); `dive check` notes at every default-profile stop that the result may differ under `--compat=tlc`; detecting whether the order mattered is planned | Probe `StringOrderViolation` (TLC reports `InvZeta` after 2 states); `crates/dive-eval/tests/options.rs`; Examples `MissionariesAndCannibals` (226 generated vs TLC's 225) and `acp/ACP_NB_WRONG_TLC` (12,645 distinct vs 11,852), plus pinned PlusCal `SBB` (1,581 distinct / 3,042 generated vs 1,617 / 3,126), same violation, property and depth, registered in `conformance/expected-divergences.toml`; D-010 |
| Independent counterexample validation (`dive validate-trace`; every `dive check` counterexample) | TLC prints its counterexamples unchecked, and `-dumpTrace json` cannot be read back: sets and sequences are both arrays, strings and model values both strings | Every counterexample is replayed against the spec by a separate validator that shares no search code with the checker; a failure is an internal error, never a verdict | `dive validate-trace` checks any trace, including TLC's own `-dumpTrace json` and a Rust program's implementation traces (`dive_api::trace`) | preserving (never changes a verdict) | implemented in `dive-trace`, `dive-api`, `dive validate-trace` and the harness (ADR 0016 amendments 1–2); bounded independent deadlock/fairness enabledness; `dive check` wiring with WP-D2 | `dive-conformance trace-controls`: 12 of 15 TLC safety counterexamples valid, 3 refused by name, every mutant rejected; `crates/dive-api/tests/trace_controls.rs`; D-012 |
| Logical worker count (`TLCGet("config").worker`) | TLC reports its physical worker count, so specs asserting `worker = 1` (e.g. `MCDieHardest`) refuse to run in parallel | **1 (logical) at any `--workers`** (owner decision 2026-10-08) | `--compat=tlc`: physical count, as TLC reports it | changing (only for specs that read it) | implemented (`dive-check` `run_config`) | D-011; harness worker-invariance report |
| Action composition (`--action-composition=tla`; TLC modes `=off`, `=tlc`) — **default** (D-022) | `A \cdot B` is an error unless the JVM runs with `-Dtlc2.tool.impl.Tool.cdot=true`. With it, the intermediate states count as generated, so a state whose `A` steps all lead to states where `B` is disabled is not a deadlock, and conjuncts after the composition are also applied to the intermediate state | **TLA+ meaning**: composed successors only, deadlocks reported; compositions TLC's algorithm distorts (conjuncts after it, variables assigned before it) refused by name | Core refuses disabled composition and TLC intermediate accounting by name; retained Reference supports `--action-composition=off` (part of `--compat=tlc`) and `--action-composition=tlc` (also `-Dtlc2.tool.impl.Tool.cdot=true`) | changing | pure bounded mathematical composition in Core; the retained `dive-eval` implementation also supports TLC accounting; harness passes `tlc` for the Examples `action composition` feature | `crates/dive-eval/tests/cdot.rs` and `tests/fixtures/cdot/` (`CdDeadlock`: TLC success with 2 generated, dive deadlock; `CdConj`: TLC deadlock); D-022 |
| Run budgets (`--time-limit`, `--max-memory`, `--max-states`, `--max-depth`; ADR 0107) | No wall-clock limit (only the developer property `-Dtlc2.TLC.stopAfter`); memory bounded by the JVM heap, exhaustion is `SYSTEM_OUT_OF_MEMORY` (exit 1/255) with no structured result | no default `--time-limit` in either profile; `--max-memory auto` (half of physical memory) in both | without a user-set `--time-limit` a run has no time limit; any exhausted budget reports `verdict: stopped` with resource, limit, observed value, phase and progress, exit 5 | changing (a run that exceeds a set limit or the default memory ceiling stops without a verdict) | implemented for `dive check` (safety, capture, temporal analysis; inline and parallel engines); explicit state/depth ceilings are opt-in and also bound temporal capture on Core (ADR 0158); finite DFID and simulation share the same time/memory policies, with explicit after-loading execution scope | `crates/dive-api/tests/budget.rs`, `crates/dive-liveness/tests/budget.rs`, `crates/dive-check/src/budget.rs` tests; harness budgets `crates/dive-conformance/src/pipeline.rs`; D-026 |

The opt-in [native distributed safety route](user/distributed-safety.md) extends
D-001's preserving ordered-commit contract to explicitly trusted authenticated
processes (ADR 0151). Its TLC-equivalent setting is distribution off. Current
pure safety/refusal/private-admission bounds and complete-unit retry are explicit;
focused both-profile process/word controls do not claim broad distributed-family
or performance qualification. It introduces no result-changing default.

## D-028: Canonical finite-set construction

Core finite-set literals and mapped sets use canonical order and extensional
identity, independent of TLC's mutable normalization history (ADR 0153).
This is an owner-approved changing default extending D-009. Mixed structured
sets can therefore produce a validated answer where TLC raises a construction
error. Direct scalar comparison errors remain unchanged. Symbolic domains stay
lazy; canonical construction does not enumerate every member of a powerset or
function set merely to test membership.

Bag union, difference, addition and mapping also construct their resulting finite
function domains in canonical order. They can retain concrete record keys with
different payload shapes. Nominal TLC comparisons keep checked normalization.
The `BagCounts` and `BagFailure` controls exercise counts and trace replay.

Identity-demanding operations require complete materialized children, including
singleton set elements, function keys and values, and compound states. An
unproved symbolic identity is `unsupported`, never structural inequality or a
specification evaluation error. Lazy operator membership and cardinality remain
available without this identity claim. Independent replay applies the same
boundary and cannot certify a violation or error word using an implementation
refusal.

The TLC-equivalent setting would retain TLC's constructor-object history. Core
intentionally does not implement that setting: nominal `--compat=tlc` uses
canonical producers with its existing checked-comparison refusals. The gap is
listed in [tlc-differences.md](tlc-differences.md#d-028-canonical-finite-sets).
The existing pinned `NxInLazyCup` control generates 10 states in TLC and 7
in default Core, with 3 distinct states, depth 2 and no queued states in both:
canonical enumeration removes the repeated `[a |-> 1]` from the lazy union.
`core_rules` asserts that exact generated-count difference without an oracle edit.

The same difference applies to initial membership. With
`Init == x \in ([a : {0}] \cup [a : {0,1}])` and `Next == UNCHANGED x`,
TLC emits three initial witnesses and five states overall. Default Core emits
two initial witnesses and four states overall. Binding the union to `S` in
`LET S == ... IN S` gives the same results. Nominal compat refuses the direct
form and reports the canonical counts for the `LET` form. Every completed
run succeeds with two distinct states, depth one, exit zero and an empty
queue.

Tests: `core_receiver_consumers` covers both MixedView literal orders,
MappedMultipleSubsets, independent valid/invalid words and large symbolic domains.

The shared evaluator uses the same mathematical default for Reference checks
and independent replay. Reference under `--compat=tlc` retains its original
normalization-dependent results. Exact additional default controls are:

| Corpus / model identifiers | Default result |
|---|---|
| `graph-demand/DemandedEdges` | `Inv` violation, 1 distinct / 1 generated / depth 1, exit 12; the word validates independently and its changed-state mutant fails |
| `graph-demand/SimpleEmpty` | Success, 3 distinct / 3 generated / depth 3, exit 0 |
| `graph-empty-product/LazyDemandedProduct`, `LazyProductHistory` | Success, 3/3/depth 3, exit 0 |
| `product-operations/DemandedRawProductSize` | Success, 3/3/depth 3, exit 0 |
| `reference-finiteness-demand/AliasSizeAfterFiniteness` | Success, 3/3/depth 3, exit 0 |
| `set-conversion-demand/AliasSizeAfterProjection`, `NonemptyMixedProjection`, `OtherConvertedConsumers` | Success, 3/3/depth 3, exit 0 |
| `crates/dive-api/tests/fixtures/comparison-receiver-unclassified/InitialOverride.tla` | Success, 3/3/depth 3, exit 0 |

Real argument errors remain errors: `graph-demand/FieldPriority` reaches the
edge-field type error, `set-conversion-demand/AntiFunctionRawEmpty` rejects a
set where a function is required, and `set-diagnostic-demand/PrefixReverse`
reports the nonempty-set requirement. Core reaches these real argument errors
without receiver-history admission. The other empty-prefix controls
`PrefixNestedReverse`, `PrefixNestedUnknown`, `PrefixUnknownAfterEmpty` and
`PrefixInfiniteAfterEmpty` also report that requirement; `PrefixUnknownFirst`
and `UnknownFirstProjection` retain the demanded non-set operand error. Each
reached initial error has 1 distinct / 1 generated / depth 1, exit 75; independent
named-error words validate, while changed error/state claims fail. The
`ConvertedEmptyProducts` positive/opposite fixture matches TLC: success 3/3/3
or initial `Inv` violation, exit 12, with a validated violation word. Core reports
1/1/1 for the initial violation; TLC omits those statistics.
Context-only TLC observations remain
unclassified; no deciding oracle class is inferred for registry entries.

## Oracle facts behind the registry

- **Reflexive model-value membership (D-009).** The exact
  `construction-set-history/ModelBadPrefix` model checks `a \in {1,a}`.
  Its default run succeeds with 2 distinct/generated states and depth 2;
  pinned TLC reaches the earlier typed-model prefix comparison and stops with
  fingerprint exception2147, exit75, at 1/1/depth1. The registry recognizes
  only this model's default success, exact exit75→0 and generated1→2 pairs,
  and its differing verdict/class/distinct/depth fields. It supplies no trace
  permission or strict-profile waiver and never counts as agreement. This does
  not specify the answer to arbitrary mixed-type comparisons.

- **Receiver comparison controls (D-009).** Eleven exact models in
  `comparison-receiver-primary` preserve the existing extensional function
  comparison policy. Default successes were reproduced before and after the
  receiver-provenance change, in both engines. The registry binds each exit
  and generated-count pair separately; only `NextLiteral` and
  `SubsequentInvariant` need the existing finite-error-word versus success
  length/ending distinction. No invalid-word or independent-validation field
  is permitted. These outcomes are expected divergences, never agreement.

- **Introspecting specs.** Specs can legitimately depend on `TLCGet("config").worker`. The owner-approved default reports logical 1; `--compat=tlc` reports the requested physical count, matching TLC. `MCDieHardest` therefore runs in the default profile at any width but fails its assumption under multi-worker compat mode. Physical threads remain visible in the result (D-011).

- **Finite temporal prefixes (D-008).** `QtEmpty` prints temporal violation 2116
  and a finite initial-state trace, but exits with safety status 12 rather than
  ordinary temporal status 13. The harness preserves both facts and verifies
  the specific complete stream shape. TLC-exact phase-L checker behavior remains
  pending; this parser correction does not enable a new checker default.

- **Action-composition generated counts (D-022).** The default-only harness
  registry recognizes the exact TLC/native pairs 1,350,001/900,001 for
  `FiniteMonotonic/MCCRDT.cfg` and 17,425/11,617 for `MCReplicatedLog.cfg`.
  Both entries permit no other differing field. The original full gate at
  `345a9bd1` exposed the missing MCCRDT entry after that model reached a
  completed result: success 2193, physical exit 0, 25,000 distinct states and
  depth 16 agreed; only the already documented 450,000 composition intermediates
  differed. The original failed gate remains a failure, and this classification
  is expected divergence rather than strict agreement. Issue #413 records the
  evidence. Goldens and checker behavior are unchanged.

- **Composition nonstuttering and fairness (D-022).** Independent bounded TLA
  replay tests the original-to-final edge. For `(x'=1) \cdot (x'=0)` at `x=0`,
  `ENABLED <<A>>_x` is false and both fairness kinds hold on the stuttering word.
  Pinned experimental TLC answers true and violates each fairness property.
  These exact truth fields are the additional D-022 scope, proved by finite
  relation enumeration and retained oracle facts in
  `crates/dive-trace/tests/fixtures/composition/`. No unrelated field is waived.

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

The checking-only TLC adapter uses D-008's existing human/JSON
reports and D-013's bounded short-lasso selection: `dive tlc -lncheck final`
selects that backend directly. Tool framing and other TLC mode protocols remain
named refusals. Filename normalization belongs to the adapter; native file APIs
are unchanged. The adapter reads and parses the configuration first, matching TLC's
input-error priority. See the
[checking guide](user/checking.md#tlc-checking-entrypoints).

| ID | Capability | Intended default | TLC counterpart | Status and evidence |
|---|---|---|---|---|
| D-013 | Liveness counterexample selection | `--lasso=short`: shortest prefix, deterministic short cycle | none: TLC's exact lasso is not a parity obligation (ADR 0082); `--compat=tlc` keeps the short validated lasso. `--lasso=tlc` was withdrawn on 2026-10-12 and its emulation removed | partial implementation: bounded final Rust API with WF/SF, finite constant-domain quantifiers, pure constant/state-argument operators and independent scoped ENABLED covers, original property claims and independent validation; explicit final/short CLI access with mixed-profile reports and eligible ordinary TLC-profile final access; bounded ordered parallel capture with 1/4/8/chaos controls; eligible ordinary dive-profile requests use bounded deterministic completed-level checks (D-014); remaining recovery/error and resource boundaries retain named refusals. ADRs 0017–0018, `LvNoFair`, `LvTwoLoopsRev`, fingerprint series |
| D-014 | Periodic liveness scheduling | Eligible ordinary dive requests: deterministic completed-level checks once the graph has at least 2^16 states by default (selectable with `--periodic-min-states`) and has doubled since the previous check (`options.lncheck: deterministic`, ADR 0018 §7) | Ordinary TLC profile and explicit `--lncheck final --lasso short`: final checking; no TLC wall-clock emulation | Bounded implementation; ordinary prefix checks search the first configured property, and opt-in `--lncheck periodic-all` searches every configured property in order in either profile; prefix words require fresh typed and serialized validation, actual partial counts retained; no corpus or performance claim |
| D-015 | Malformed symmetry declarations | Reject non-bijective functions | TLC acceptance under `--compat=tlc` | bounded checked/TLC closure and pure safety reduction implemented; default non-bijections refuse before search, compat reproduces SyNonBiject; staged startup warnings match source-backed probes; TLC quotient liveness, randomness and unverified declaration/value shapes still refused; supported whole-run unreduced temporal checking follows D-005 (ADRs 0019 and 0085) |
| D-016 | Checking declared symmetry | Audit only with `--check-symmetry` | No audit, as in TLC | planned; `SymAsym`, `SymAsymLate` |
| D-017 | Host access from a specification | No access without explicit category grants | Explicit grants also required under `--compat=tlc`; restrictions are reported | partial: bounded Reference startup IOEnv/TXT inputs, recorded assumptions and fresh CLI/API replay; runtime reads, writes, exec, recovery and unproved TLC name ordering refuse (ADR 0148, #757/#758); full corpus host models remain unqualified |

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
`crates/dive-api/tests/fixtures/checker/dfid/`.

**Refusals:** TLC rejects `-dfid` with several workers or with temporal
properties as general error 1000 (exit 255). `dive dfid` refuses both by name
(`DFID multiple workers`, `DFID temporal properties and fairness`) with exit 3
in both profiles; `crates/dive-cli/tests/mode_refusals.rs`.

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
generated-operator collisions in both profiles and admits an artifact only
after complete-module syntax/name/level analysis. Scoped name rewriting,
multiline substitutions and branch labels now follow the pinned translator;
successful generation does not imply an available Full algorithm checksum. No alternate
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
[its full selection](pluscal-translation.md) retains branch/name refusals. No
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
pure state-IF admission in explicit final TLC mode (#168);
ordinary periodic scheduling and upstream-refused temporal-argument IF forwarding remain refused.
This is a native limitation repair, not family certification.

The [single-process increment](pluscal-translation.md) matches unfair constant-ID
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

The [bounded process-fairness increment](pluscal-translation.md) adds weak/strong
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

The [multiple finite-process increment](pluscal-translation.md) proves
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


The bounded [native macro translator](pluscal-translation.md) retains all invalid
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

### D-013: ordinary TLC-profile final route

Eligible ordinary TLC-profile requests now reuse the existing pure bounded final
analyzer, actual configured-role dispatch and fresh independent word validation.
The selected schedule is reported as final, with deterministic short witnesses;
periodic scheduling, finite-prefix stopping and general temporal error timing are
not implemented by this route. Actual completed counters remain observable in
schema 2; safety-only roles keep the complete ordinary schema-1 result and startup
host. Ordinary default-profile requests use D-014's deterministic completed-level
schedule instead. Excluded options retain their
existing named refusals. See [the temporal guide](user/temporal.md#ordinary-checking-in-the-tlc-profile)
for precise eligibility. This changes no resource bounds or periodic defaults.


D-013 acceptance resource policy: file-based eligible ordinary/final checking
uses compact acceptance-buffer bytes (8,000,000) and independent materialization
work (32,000,000) in both profiles. Existing public custom-limit APIs retain
legacy cells; additive APIs select the new policy explicitly. TLC has no fixed
counterpart, so exhaustion is a resource refusal, never a verdict. Covered
allocations and excluded owners are listed in [the temporal guide](temporal-api.md).
This does not bound process memory or change vertex/edge/domain/transition limits.


### D-013 historical increment: ordered temporal execution

The earlier staged source records this scope; the later explicit final CLI
contract is described in the current user guide.

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

### D-013 historical library increment: pinned final-only SCC and witness selection (removed 2026-10-12)

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

See final-only Rust API guide for examples, resolved schedule,
actual evidence and refusal boundaries. The API admits startup sources before
binding creates a host; effectful ASSUMEs, unused top-level constants, config
replacement candidates and hooks return named refusal with zero used workers.
Standard-module classification does not bypass this admission. Explicit sequential
requests/fallback execution remain refused until their observer path is certified.


**Explicit final/TLC integration update (withdrawn 2026-10-12: the `--lasso tlc` mode, the `dive_api::tlc_temporal` API and the `dive-final-tlc` harness mode were removed with the rest of the TLC-internal liveness emulation):** `dive check --compat=tlc --lncheck final
--lasso tlc` now exposes the independently certified bounded final-only path.
Actual counts/worker/stage metadata and original property identities are retained,
including TLC's stuttering marker. This does not enable periodic checking or lift
exact-store, accepting-sink, recovery, effect or resource refusals. The separate
`dive-final-tlc` harness mode requires a final-schedule golden and fresh independent
replay; ordinary schema-1 results and earlier studies are unchanged. Configuration
alias targets are admitted through actual module contexts before any startup
binding effect. See the user guide.


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
are preserved. See the safety-only user contract
and `conformance/probes/tlc-final-safety` for clean paired evidence.

D-013 conditional-admission progress (#168): pure state IF atoms are admitted
through the corrected shared planner and independent final-word replay. This
removes the older duplicate syntax refusal without changing conversion/error
order, resource defaults or ordinary periodic scheduling. All original refusal
and failed-capture histories remain available. Effects, opaque bindings and
unverified errors remain explicit; no whole-family certification is claimed.
See conditional user guide, ADR 0036 and
`conformance/probes/tlc-state-if/`.

Temporal-kind IF remains a named bounded final-mode refusal (#170). Its
shared-guard normalization/numbered-particle repair is now implemented as
diagnostic language evidence, with actual final execution
and independent witnesses still pending. D-013 and temporal families stay partial.

The proven direct temporal-IF subset now runs in explicit final/TLC mode after
shared-object tag repair, exact particle/edge checks and fresh independent
in-memory/JSON witness replay. The unchanged full selected controls retain
upstream 2213/2214 errors and source-prefix 2116/exit-12 cases as named refusals,
without matching-error credit. Compound guards and INSTANCE claim/role identity
are separately checked. Existing arena/work ceilings and whole-arena forwarding
admission remain; no aggregate memory bound or periodic-family claim is added.
See the user guide and
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
options. The [fair procedure guide](pluscal-translation.md) describes exact
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
[the runnable recovery guide](user/storage-recovery.md)
and separate preserved study.

## D-020: fair-algorithm checksum collision (candidate)

The pinned PlusCal translator computes `7c28162a` for every tested `--fair`
algorithm, because its ternary checksum expression hashes only `"fair"` and
omits the normalized AST. Changing `x=0` to `x=9` without changing the translation
leaves `Validator` reporting `NO_DIVERGENCE`. C/P primary bodies still have different
TLA CRCs. No native corrected fair CRC default is adopted. Exact
procedure-free AST serialization retains the reference CRC, including this
weakness. That CRC is not source identity or proof. Unavailable algorithm CRCs
use the disabled-value spelling while retaining the real generated-body CRC;
`--checksums full` and `--checksums tla` select the same best-effort policy. See D-020 in the
[user log](tlc-differences.md), D-008 for separate artifact admission, and
`crates/dive-pluscal/rewrite-acceptance/fair-checksum/` for all six primary/four
source-edit controls.

Bounded final scope evidence (#134) now distinguishes configured names from
local/formal/INSTANCE derived roles; see the scoped final guide.
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

**Historical checksum-era scope (superseded by issue #241):** D-008 checksum continuation retains strict generated-operator CST comparison,
including an initially missing empty-Else TRUE conjunct now reproduced exactly.
The checksum-only producer did not lift multiline-generation refusal. The later
bounded non-junction sequential generator is documented in `docs/pluscal-translation.md`;
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
See the identity guide.

Callback-free captured concrete values now have a bounded matching admission
in explicit final mode; this resolves a native limitation, not a new divergence.
Stored representations, context/name authority, fixed recursive work/depth and
deferred/opaque refusals are documented in the guide.
No ordinary scheduling or whole-corpus certification is implied.


**Historical checksum-era scope (superseded by issue #241):** D-008 finite-process Full checksum continuation: declared-fairness records and
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
parity is unclaimed; see the operator guide.

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
[option use](pluscal-translation.md), and all clean configured,
unconfigured and original ignored-option native captures in
`crates/dive-pluscal/process-option-checksum-acceptance/validator-context/`.

**Historical checksum-era scope (superseded by issue #241):** D-008 process checksum options update: supported explicit fairness and inserted
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

D-009 implementation boundary (#619): the core now applies its existing
comparison-shape proof to compound set normalization in both profiles. Unproved
nested comparisons return unsupported rather than default success; exact
checked sorting remains pending. Size and earlier-field short circuits can
cause conservative refusals even where TLC succeeds. This preserves C2,
without adding an approved divergence. Pinned consumed controls are in
`crates/dive-api/tests/fixtures/core-normalization/`.

D-009 implementation boundary (#188): pure VIEW now deep-normalizes every
concrete variable before projection. The pinned primary exits255/EC1000 for
an ignored reverse-ordered tuple/record set; dive explicitly refuses the
unproved normalization/error-class case instead of a four-state violation
with unprintable values. This is an honest implementation refusal, not a new
result-changing default. See `conformance/probes/view-concrete-normalization/`.

The subsequent bounded native operator actual slice
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

## Stored successor admission (existing final-mode limitation)

The bounded final TLC path uses explicit stored Enum/Raw set elements and stored
function entries before fingerprints/comparison, including normalized Raw sets.
This is a native representation admission change, not a changed TLC semantic
default. Cached provider/deferred sets remain refused. Existing8m/depth256
Session bounds and separate50k/128 context bounds are unchanged. Pinned
`LiveCheck.addNextState`/`SetOfStates` concrete-value behavior is retained; see
stored-state guide and
paired source/evidence.
Ordinary schedules and whole-family parity remain partial.

Bounded final native forwarding additionally admits `Append` with exactly two
constant/state expression operands. Tail/SubSeq runtime-name admission remains
refused, not a default change; see `docs/native-sequence-operators.md` and the
complete source-bound controls. No new D-ID or family certification applies.

## Bounded partial temporal SCC diagnostics (#185)

Native ordered source commits now have a separate bounded partial-check API.
It skips unfinished records, retains original root/EA/OOS/PEM order and candidate
identities, and charges projection/SCC/witness work against the existing shared
graph quota. Successful partial checks never mean temporal `Holds`; ordinary
strict checking remains refused until real clock/quiescence/error/stop-count
contracts are implemented. The complete32 explicit-prefix projections and8
repeated forced original checks are diagnostics, not ordinary schedule runs.
See the user/API guide and
complete controls.
No new result-changing default or family certification is claimed.

### D-008 macro checksum stage qualification

Bounded nonrecursive macro Full serialization retains expanded reference AST
bytes separately from module admission. The original 240-row primary selection
has 28 translated/SANY-invalid artifacts; none earns artifact or proof credit.
Embedded ASCII-star decoration is now selected without source mutation, while
two C/P arbitrary trailing-prose native syntax
disagreements remain archived, with exact parser-consumed embedded end ownership
now matching the paired controls. Full procedural/scoped layout and multiline/junction actuals
remain explicit refusals. See [macro checksum scope](pluscal-translation.md).
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
macro checksum study.
Artifact admission and CRC equality still never establish model correctness.

D-008 bounded DEFINE progress: [DEFINE helpers](pluscal-translation.md) can be
serialized and emitted with reference declaration ordering. Raw AST/CRC equality
is separate from enclosing-module SANY/native admission and finite-model
checking. Invalid primary artifacts stay uncredited; recursive, symbolic,
action/temporal and unproved scope shapes remain named refusals. Neither profile
inlines helpers or silently repairs the reference output.

## Supplied pure periodic expressions (bounded diagnostic limitation)

ADR0054 adds a pure supplied-expression API at native dispatch pauses. Ordinary configured `_PERIODIC` binding and checkpoints stay refused. TRUE never means Holds; false/evaluation stops preserve original errors and no EOF credit. This changes no default/TLC semantic option and earns no family certification. See the guide and complete59 controls.

D-008 bounded LOCAL/function DEFINE progress: [the executable guide](pluscal-translation.md)
preserves local export visibility, pre-state helper lookup and function domains.
Domains and bodies share the action/temporal/cycle guard. Exact raw CRC equality
is distinct from public enclosing-module admission and model proof; invalid
reference outputs remain uncredited. Both profiles keep these same semantics.
Configured periodic diagnostic scope (#197): the pure snapshot factory is additive; ordinary `_PERIODIC` checking remains refused. Source/startup/effect and Java-exception restrictions are explicit limitations, not new result-changing defaults. See the current user contract.
D-009 trace transport boundary (#190): trace format version 1 retains the
known mixed-set shape failures. The opt-in stored-representation version 2
was withdrawn on 2026-10-12.
trusted-storage tuple bit loss (#195) is repaired by explicitly versioned structural storage format 2; older ambiguous archives refuse.


Producer request limitation (#201): the bounded Rust diagnostic stops actual new expansion claims and joins admitted work, but current-wave outputs remain uncommitted until the existing ordered source path. These receipts do not certify ordinary Java queue suspension, schedule/count parity or a model verdict. Checkpoint and ordinary CLI refusals stay in place; no new default or family credit.

### Actual committed request-prefix limitation (#204)

The Rust request diagnostic publishes only an actually returned contiguous source prefix before its periodic check, using the shared safety/output/count path and retaining later buffers. Graph/source publication can precede physical frontier append; deciding callbacks preserve that boundary. Complete24 actual pinned method observations include returned-but-unwritten Next output, later graph writes,2179 worker errors,2116 candidates,2104 hook stops and EOF. Hook exceptions exit2 without probe cleanup; early nonexception returns can leave workers alive. These measured boundaries earn no ordinary native scheduling/count/verdict credit. The original8192-binding candidate retains its independent-validator work refusal. No new semantic default or D-ID applies; ordinary scheduling/checkpoint/model proof remain unfinished.
D-007 storage increment (ADR 0063): additive `dive_store::seen::DiskSeen`
provides actual fallible disk fingerprint/exact membership with bounded
workspaces and explicit owned cleanup. ADR0063 qualified the standalone library only. ADR0068 now also admits
explicit INIT/NEXT safety through the ordinary disk route described below;
parallel read views and a total seen-memory budget remain pending. Neither
increment certifies the full storage family. See [the library guide](user/disk-seen-library.md)
and [the current CLI/API route](user/disk-seen-safety.md).

D-014 also retains the controlled zero-duration NaN updater/controller observation
and a failed native diagnostic resource experiment in [the known-issues log](internal/tlc-differences-notes.md#zero-duration-ratio-edge-case-controlled-component-evidence).
A finite-ratio improvement remains under consideration; no ratio default changes.


**Historical checksum-era scope (superseded by issue #241):** D-008 disjoint sequential procedure Full checksum continuation (ADR 0069)
represents target/current frame owners separately from the last declaration's
shared main-call vector. The original 64 C/P rows retain 62 clean reference
translations and two grammar errors; two corrected declarations and 32 declared
ownership/call-graph/option controls remain distinct. Named unproved call-graph,
process/scoped and modifier boundaries emit no replacement or CRC. Both profiles
retain the pinned header behavior, unchanged global quotas and D-020 weakness.
Invalid `-fair` commands are command-stage uncredited controls, not language
rejection or exit-equivalence credit. Artifact and strict saved-frame replay
remain separately qualified. See [the runnable guide](pluscal-translation.md).
## Supplied pure periodic expressions (bounded diagnostic limitation)

ADR0054 adds a pure supplied-expression API at native dispatch pauses. Ordinary configured `_PERIODIC` binding and checkpoints stay refused. TRUE never means Holds; false/evaluation stops preserve original errors and no EOF credit. This changes no default/TLC semantic option and earns no family certification. See the guide and complete59 controls.

Configured periodic diagnostic scope (#197): the pure snapshot factory is additive; ordinary `_PERIODIC` checking remains refused. Source/startup/effect and Java-exception restrictions are explicit limitations, not new result-changing defaults. See the current user contract.

Producer request limitation (#201): the bounded Rust diagnostic stops actual new expansion claims and joins admitted work, but current-wave outputs remain uncommitted until the existing ordered source path. These receipts do not certify ordinary Java queue suspension, schedule/count parity or a model verdict. Checkpoint and ordinary CLI refusals stay in place; no new default or family credit.

### Actual committed request-prefix limitation (#204)

The Rust request diagnostic publishes only an actually returned contiguous source prefix before its periodic check, using the shared safety/output/count path and retaining later buffers. Graph/source publication can precede physical frontier append; deciding callbacks preserve that boundary. Complete24 actual pinned method observations include returned-but-unwritten Next output, later graph writes,2179 worker errors,2116 candidates,2104 hook stops and EOF. Hook exceptions exit2 without probe cleanup; early nonexception returns can leave workers alive. These measured boundaries earn no ordinary native scheduling/count/verdict credit. The original8192-binding candidate retains its independent-validator work refusal. No new semantic default or D-ID applies; ordinary scheduling/checkpoint/model proof remain unfinished.

D-007 ordinary disk-seen increment (ADR0068): `--seen-disk-entries N` now admits
bounded inline safety membership for full fingerprint/exact keys. The resident
reference remains default in both profiles. Typed limits/cleanup and actual
inline workers are explicit; graphs, recovery and parallel read views remain
unproved. This is result-preserving completed behavior with explicit limits,
not a new semantic default or total-memory promise. See the
[user and Rust guide](user/disk-seen-safety.md).


D-007 storage implementation note (#217, ADR0072): the existing opt-in inline
INIT/NEXT disk engine now has a positioned fallible invariant-error replay path.
Clean ReplayNew reports 2 generated/4 distinct/2 queued while the generated
counter remains frozen. First query/insertion/bound/cleanup failures retain the
original error and ordered prefix and refuse completion. This adds no new
result-changing default, disk parallel/capture/recovery support or family credit.
Original guarded and incorrect-count epochs remain separately linked in
the replay study.

ADR0072's D-007 disk continuation retains an explicit actual-Print/PrintT
error-reconstruction refusal. Clean `ReplayPrint` has three output events,
pre-review native one; equal counts do not grant class/output or certificate
credit. Pure key/count replay, original errors and positioned failure evidence
remain separately qualified. This adds no result-changing default.


**Historical checksum-era scope (superseded by issue #241):** D-008 acyclic sequential procedure Full checksum continuation (ADR0073) separates
actual body edges from the reference LAST-header main-call alias. The clean
130-row C/P selection retains the original98 and adds32 acyclic shapes;
original-header/CRC, exploded frames, public artifact and replay evidence remain
separate. Real recursive/process/scoped graphs and other unproved checksum
shapes remain named refusals with no Full replacement. Shared quotas and
D-020 fair-header weakness are unchanged. See
[the executable guide](pluscal-translation.md); F-034/F-035 remain
partial and strict saved-frame replay stays qualified under #124.


## Historical D-008 recursive sequential procedure checksum continuation

The retired bounded Full editor serialized source-finite self/mutual call
vectors without unrolling execution. Pinned ordinary/tail ownership and LAST
header alias remain distinct in both profiles; quotas and D-020 CRC weakness do
not change. The 162-case selection keeps all 130 predecessors and two new
primary-generated/SANY-invalid zero-parameter self-tail artifacts uncredited.
Process/scoped checksum and strict #124 saved-frame replay remain qualified;
CRC equality is neither artifact admission nor a model proof. See
[the executable guide](pluscal-translation.md).

D-007 role continuation (#221, ADR0075): opt-in disk checking binds PROPERTY and
SPECIFICATION roles through the shared binder and gives the resident route's
results. Temporal checking with disk keys remains unsupported; requested
widths still disclose actual inline execution.
[Executable CLI/Rust guide](user/disk-seen-safety.md).

**Historical checksum-era scope (superseded by issue #241):** D-008's process-owned procedure Full-checksum continuation (#225/ADR0079)
separates original stored header ownership from process-indexed runtime slots.
One unfair finite process/one disjoint labelled procedure has bounded native
serialization; general process/fairness/scoped checksum contexts remain named
refusals. The clean C/P With binder `a` shadowing procedure parameter `a`
generates SANY-invalid duplicate operators; no successful artifact or CRC is
credited from those invalid references. Strict #124 frame replay is unchanged.
See [the current PlusCal contract](pluscal-translation.md).

## D-023: Truthful default numeric-exit accounting

**Current CLI status (ADR 0097):** ordinary `dive check` now uses TLC physical
statuses while retaining its native evaluator defaults. `--exit-codes native`
and historical `CheckReport::exit_code()` keep the earlier native contract.
The following captures describe the measurement correction before this new
explicit runtime default decision. New agreement requires actual matching
exits and every existing count/property/class/witness obligation; no corpus
projection below is new measured credit.


The original correction recorded the then-stable native exit contract documented
in D-008; it changed conformance accounting while preserving CLI/API behavior.
That historical default used physical exit 10 for property violations and 1
for rejected input. It now requires `--exit-codes native`; ordinary checking
uses TLC's deciding status in both semantic profiles. Numeric equality
is required for Agree in both profiles; documented unequal codes are explicit
ExpectedDivergence only on the exact registered model paths.

The measured exit-only correction covers these 14 previously false Agree rows:

| Model paths under `specifications/` | Pinned TLC → native default |
| --- | --- |
| `CheckpointCoordination/MCCheckpointCoordinationFailure.cfg` | 12 → 10 |
| `DieHard/DieHard.cfg`, `DieHard/MCDieHarder.cfg`, `DieHard/MCDieHardest.cfg` | 12 → 10 |
| `N-Queens/Queens.toolbox/FourQueens/MC.cfg` | 12 → 10 |
| `SlidingPuzzles/SlidingPuzzles.cfg`, `SlidingPuzzles/SlidingPuzzles_anim.cfg` | 12 → 10 |
| `spanning/MC_spanning.cfg`, `tower_of_hanoi/Hanoi_anim.cfg` | 12 → 10 |
| `P5TLCGetSet/P5TLCGetSet.cfg`, `P8ParallelDet/P8ParallelDet.cfg`, `P9Random/P9Random.cfg` | 12 → 10 |
| `M1Syntax/M1Syntax.cfg`, `M2Undefined/M2Undefined.cfg` | 150 → 1 |

The historical two D-010 safety entries also required 12 → 10; their ordinary
CLI statuses now match12 in direct controls, while their unchanged count
exceptions remain. P1Overflow
requires 255 → 0 (D-004), and P2Choose_str requires 12 → 0 (D-006), alongside
only their previous allowed fields. No wildcard registers future models.
The optional typed pair in `conformance/expected-divergences.toml` requires the
exact two integer codes and expected native verdict. Null, signals, unrelated
codes, booleans, floats and out-of-range values never match. Class, property,
count, depth and witness differences remain failures unless already explicitly
allowed; failed or unconfirmed independent trace validation is never excused.

Historical default90/4/60 is qualified as an exit-only estimate of 76 Agree /
18 ExpectedDivergence / 60 Unsupported, not a fresh conformance certificate or
an approved lower floor. Original reports and floors remain visible; full
remeasurement and review are pending. See [ADR 0096](decisions/0096-truthful-default-numeric-exit-conformance.md)
and the current measurements in [the scoreboard](scoreboard.md#corpus-scoreboard).

D-008 original PlusCal AST artifact continuation: explicit `translate --ast` and
public original-AST APIs expose bounded earlier-stage bytes as data in both
profiles. `artifact_available` does not mean a generated enclosing module was
accepted, no header CRC is claimed at that stage, and no file is written. Actual
`-writeAST` remains a named filesystem refusal; its retained successful primary
writes return null from translation and exit255. Procedure-free `-debug` header CRCs match
the retained primary while preserving TLA body bytes. Modern original procedure/
macro and compound ownership now have genuine pre-generation controls. Generated
debug labelled compound procedure-free headers retain exact CRC/body controls.
The historical flat procedural header CRC captures describe retired alias
emulation; current direct exact procedure CRCs are unavailable. Broader flags and
historical later-stage fields remain unfinished. SANY semantic/linting success with warnings
is separate from original checksum validation. This adds no semantic
default or completed flag/corpus milestone. See [the guide](pluscal-translation.md).


D-008 PlusCal checksum/API continuation: schema2 translation reports contain real
artifact/replacement acceptance, whole-module analysis and requested/effective
settings without proof-only fields or duplicate option-context dumps. The one
best-effort checksum policy retains exact procedure-free CRCs and the real TLA
body CRC; unavailable algorithm CRCs use `chksum(pcal) \in STRING`, including in
compatibility mode. This is an explicit header difference, not byte equality to
an unlike unmodified primary translation. A predeclared primary input with that
value disabled preserves it during actual retranslation; original primary facts
remain separate. Literal clause omission disables body-change warning detection
and is not emitted. Unknown CRCs never gate generation/rewrite/debug or claim a
model result. Default native modified-body protection keeps `--replace-modified` as the
explicit counterpart; the TLC profile resolves ordinary pinned pcal's Noop
overwrite behavior automatically. Algorithm changes
alone regenerate without a grant. `--checksums full` and `--checksums tla`
select the one policy; direct exact procedure CRCs remain unsupported. Full flags/filesystem/
backends/149 and ordinary model-checking milestones remain open. See the current
[PlusCal guide](pluscal-translation.md) and [ADR0084](decisions/0084-pluscal-translation.md).

Current PlusCal overwrite policy: the default API/CLI protects modified or
unverified existing bodies, with `--replace-modified` as its explicit overwrite
counterpart. `--compat=tlc` resolves that protection to ordinary primary Noop
overwrite automatically. Pure low-level `rewrite_document` retains its caller's
explicit policy. `requested_editing` records input settings, `editing` records
effective settings, and old body CRC/match data remains observable. Marker,
source-option, generation, module-analysis and quota refusals are unchanged.
The fresh exact modified-body primary control supports its own complete output
comparison; missing/disabled header variants do not inherit header byte-identity
credit from that control.

The historical checksum-era scope notes above preserve earlier primary facts and
retired admission boundaries. They do not describe current translation support:
checksum availability no longer gates generation, rewriting or debug, and exact
procedure-header alias serialization has been removed. Procedure-free exact CRC
and original-AST operations remain real features. See the current
[PlusCal contract](pluscal-translation.md); historical input gaps and model-parity
obligations remain open.

## D-024: Pinned local Bits exhaustive checker scope

Historical guard; stable ID/link retained, not reused or converted to an
ExpectedDivergence entry.

Three retained directTLC4 Hanoi runs report243distinct/907generated/depth33;
historical native4 and the primary/golden1 report233/768/depth32, with the same
NotSolved/2110/physical12 and valid32state native words. All raw observations and
negative comparisons remain historical and are not regraded. The former local-
class one-worker restriction fenced that stopping-prefix discrepancy. The
existing comparator uses the one-worker reference for native implementations
at every width; only TLC oracle self-checks have multiworker leniency.

The proposed revised checker follows the maintained native contract: every
supported native width compares to the pinned one-worker reference. Required
verdict/property/class/physicalexit, counts/depth and independent validation of
the actual concrete native word remain unchanged. This removes only the local
class width guard; exact class admission and checkpoint class-source identity
stay binding. General store/effect/capture, simulation and DFID scopes retain
existing restrictions. No comparator, golden, ExpectedDivergence, queue-order or
timing exemption changes.

Acceptance requires latest-parent original-MC1 qualification, a separate
complete-search primary/native1 diagnostic, supported native4 pinned-reference
and independently validated actual-word controls, fresh review/fullgate/all4/CI.
Issue #272 remains open; this source proposal is not runtime acceptance.


D-013 query-language continuation: evaluator ENABLED uses canonical-name partial
successors for root and hidden INSTANCE variables in both profiles, retaining
actual lazy branch cutoffs. This is TLC language behavior, not a new divergence.
Independent INSTANCE query/word and general callable/temporal integration remain
unfinished. D-022 composition still uses root-only intermediates; assigned hidden
query names refuse explicitly rather than being projected away. See
[query execution and boundaries](user/temporal.md#instance-query-namespace-controls)
and ADR 0099.

## D-026: Run budgets stop a run with a structured result

ADR 0107. `dive check --time-limit DURATION|none` and `--max-memory
BYTES|auto|none` bound a whole check. Neither profile has a default time limit, as TLC has none
(its `-Dtlc2.TLC.stopAfter` is a developer property); both profiles default to
half of physical memory. An exhausted budget reports
`verdict: "stopped"`, a `stopped` block and exit status 5 in both exit
conventions; it never reports a verdict about the model. TLC's counterpart of
an exhausted memory budget is `SYSTEM_OUT_OF_MEMORY` (1001–1003, exit 1 or
255), which carries no structured result. The memory ceiling is a
host-resource exception to `--compat=tlc`, like the host-access grants of
D-017: it applies in compat mode too (owner, 2026-10-20). Its TLC counterpart
is `-Xmx`, which `dive tlc` accepts (`-Xmx8g`, or `--max-memory SIZE|auto|none`),
so an explicit ceiling does not label the profile. An explicit `--time-limit`
under `--compat=tlc` labels it `tlc+overrides`, since TLC has no time limit. Count limits are unchanged by this
entry. D-025 is reserved for PR #334's proposal, which this policy supersedes.

Finite `dive simulate` and `dive dfid` accept the same time/memory settings.
Their reports use `result: "stopped"`, actual mode phase and prefix progress,
with exit 5. These clocks begin after model loading. Execution, retained-state
serialization checkpoints and fresh independent finite-word replay share the
same owner; exhausted runs skip unprocessed retained traces. Initial loading
and individual value encoding/decoding remain uninterruptible. One watchdog
samples resources again before publication. See the mode user guides and
`crates/dive-api/tests/mode_budgets.rs`.

The opt-in discovery budgets `--max-states N|none` and `--max-depth N|none`
extend this stopped outcome to state identities and BFS depth. Both default
to `none`, their TLC-equivalent setting. An explicit finite setting under
`--compat=tlc` labels the profile `tlc+overrides`. TLC's simulation `-depth`
does not provide this exhaustive-search setting. A first over-budget new
identity stops before admission, with resource `states` or `depth`; no fixed
liveness analysis count changes. With temporal properties a ceiling stops
graph capture (phase `graph-capture`, `capture_completed: false`) without a
verdict; violations already proven at a completed level are kept (ADR 0158).
See `docs/user/checking.md` for exact-bound, parallel-publication and recovery
semantics.


### Symmetry with liveness

D-005 disables symmetry reduction for the entire supported temporal run by
default, after actual property-role binding. Safety-only roles retain existing
symmetry behavior. `refuse` remains explicit, and `tlc` is the TLC-equivalent
requested setting but its quotient temporal implementation still refuses.
Explicit unreduced compat evaluation reports `tlc+overrides`.

The single default-profile P4 registry entry binds all three exact counts
(distinct/generated/depth: reduced TLC 1/2/1, full reference 2/3/2) and requires
the strict native reader's actual bound whole-run unreduced mode. Missing,
neighboring or reversed facts and any other observable difference are not
excused. It is an expected divergence, never `agree`, and cannot apply to TLC,
override or oracle/adapter profiles. D-022 generated-count pairs are unchanged.
The no-SYMMETRY [primary references](conformance-corpora.md#liveness-quotient-primary-controls)
are separate from established populations. VIEW remains refused; its order-sensitive
primary result and fairness reconstruction gap are tracked in #428.

## Native debugger execution boundaries

`dive debug` and `debug::Session` opt into real one-worker Core safety observation
in both profiles. `CheckRequest.debugger = None` is the default and TLC's
debugging-off setting. This native DAP route changes no model semantics and adds
no result-changing default or new D-ID. TLC `-debugger` remains unsupported.
Unsupported temporal, checkpoint, remote and inspection combinations refuse;
termination of incomplete checking cannot report success. The ordinary report
and independent word validator keep their authority. See the
[debugging guide](user/debugging.md) and
[ADR 0152](decisions/0152-compiled-source-maps-and-native-debugging.md).

## D-029: Predicate-witness counts

The owner approved evaluating recursive Boolean action predicates as predicates,
without expanding every logical witness into an identical successor (ADR 0153,
2026-11-06; #813). Admission requires a complete compiled plan without effects
or unknown call targets, conservatively covering indirect compiled calls.
Effectful or unproved plans refuse by name instead of changing register, random
or output behavior. Counts are actual emissions, including Init.

The two count controls preserve verdict, distinct states, depth, violated
property, error class, physical exit and independent word validation.

The exact default-profile registrations allow only generated counts:

| Model | TLC generated | Core generated | Distinct | Depth |
|---|---:|---:|---:|---:|
| `predicate-witness-counts/specifications/ActionCount/ActionCount.cfg` | 3 | 2 | 1 | 1 |
| `examples/specifications/byzpaxos/VoteProof.cfg` | 318877 | 27727 | 6962 | 14 |

Both succeed with an empty queue, no violated property or counterexample and
exit 0. These entries use exact `generated_count_pair` values and no field
wildcards; unrelated count or result changes remain disagreements. The opposite
invariant control fails initially and its actual word validates independently;
a forged property claim and an invalid successor are rejected. A false recursive
predicate also produces the pinned deadlock (1 generated, 1 distinct, depth 1,
exit 11), with an independently validated deadlock word.

The pinned `specifications/ActionCount/OrError.cfg` separately proves the Boolean
short-circuit case: a recursive predicate is true before a later `1 \div 0`
alternative. Core succeeds (2193, exit 0); TLC's action generator demands the
unnecessary alternative and reports `TLC_MODULE_DIVISION_BY_ZERO` (2179, exit
255), with an error prefix. Both have 1 distinct / 2 generated / depth 1 and an
empty queue. Its exact model/default registration declares only verdict,
class, error-prefix presence and exit pair 255→0. It does not authorize count,
property or independent word-validation differences.

Core keeps Boolean evaluation under nominal `--compat=tlc`; strict comparison
reports these count and short-circuit differences as findings, without registry
credit. Calls, including argument evaluation, poll run control and refuse beyond
64 active calls before unbounded recursion exhausts the host stack. The pinned
RegisterWitness regression, which TLC violates after three generated/distinct
states, refuses in Core because of its compiled effects. No lexical-capture,
prime, escape or nested-recursion guard is removed.
See [the user explanation](tlc-differences.md#d-029-predicate-witness-counts).

## D-047: Forced Boolean formals

When an operator's argument is a prime-free expression, TLC binds the formal
to a lazy value. If an earlier conjunct has already evaluated the formal, a
later use of it as an action conjunct reads the cached Boolean. Otherwise TLC
expands the argument as an action, so a disjunction emits one successor per
true disjunct. Which of the two happens depends on demand order. Core always
expands the argument in action position, independent of what an earlier
conjunct evaluated (#1166). Reproducing TLC would mean tracking at run time
whether each thunk was forced earlier in the same expansion, and keeping that
consistent with Core's guard memoization; that is demand-order emulation, which
the standing direction rules out.

Only the generated count changes. Verdict, distinct states, depth, queue,
property, error class, exit and independent trace validation are unchanged:

| Model | TLC generated | Core generated | Distinct | Depth |
|---|---:|---:|---:|---:|
| `forced-boolean-formal/specifications/ForcedFormal/ForcedFormal.cfg` | 2 | 3 | 1 | 1 |
| `forced-boolean-formal/specifications/ForcedFormal/UnforcedFormal.cfg` | 3 | 3 | 1 | 1 |

`UnforcedFormal` drops the forcing conjunct and agrees exactly. Both profiles
behave the same; the `ForcedFormal` count pair is registered for the default
profile and as a nominal compat gap. If a later disjunct of a forced formal
raises an evaluation error, TLC succeeds with the cached value while Core's
expansion reaches the error; independent validation does not reproduce that
error, so Core refuses by name instead of reporting it.
See [the user explanation](tlc-differences.md#d-047-forced-boolean-formals).

<a id="d-030-canonical-bag-fold-order"></a>

## D-031. Canonical bag fold order

With value exploration order, the Core evaluator and independent trace
evaluator apply `BagsExt!FoldBag` in normalized key order. Order-sensitive
callbacks can produce different values, effects, verdicts, counts, diagnostics
or traces. With TLC exploration order, both evaluators require a known order
for the input. [Example and profile behavior](tlc-differences.md#d-031-bag-fold-order).
