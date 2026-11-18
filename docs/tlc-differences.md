# Where dive differs from TLC, and why

dive is a Rust implementation of the TLA+ tools that is checked continuously against the canonical Java tools (SANY and TLC). This page lists every place where we think TLC does something wrong, or less well than it could, and dive does something different.

`--compat=tlc` (or `DIVE_COMPAT=tlc`) selects the TLC-profile settings. Exact
TLC fidelity is not a release requirement: the harness measures observable
results against the pinned oracle, and a profile name alone gives no
conformance credit. Unsupported features refuse by name, and each default
difference has a stable D-entry below.

## Compat gaps

`dive check Spec.tla --compat=tlc` runs on Core, the only engine. These are the
known places where `--compat=tlc` still does not reproduce TLC:

- TLC's construction and normalization history is outside Core's compatibility
  contract: construction errors, `RvMemNorm` replay and Next-side lazy-function
  fingerprint wrappers.
- Function-set diagnostics can name incomparable operands in a different order
  after canonical function construction, for example on an enumerated domain.
- Random operators require an admitted draw placement and a replayable stream;
  general TLC draw-order matching is not claimed, including with VIEW.
- Seeded simulation under `--compat=tlc` does not reproduce TLC's choice
  stream: for a given seed and aril, Core samples different traces and reports
  a different generated count than the pinned TLC (for example 31 rather than
  29 for `SimChoice` at seed 0, aril 0). Every sampled trace is still
  independently validated.
- Temporal checking uses the admitted native policy and independently validated
  words; full `-lncheck off/final` fidelity is not implemented.
- PlusCal rejects a call followed by a goto to an unknown label, which pinned
  pcal accepts (`CallGoto`).
- Deterministic native multi-worker results use the pinned one-worker baseline;
  matching TLC's multi-worker queue order is outside parity (Hanoi with `Bits`
  counts and depth).
- These are named Core refusals: bounded final temporal checking
  (`--lncheck final --lasso short`, and `dive tlc -lncheck final`); multi-worker
  liveness whose violated names are known only at run time; finite simulation
  and DFID that reach random or register runtime effects Core does not replay;
  source and TLC logical coverage (`--source-coverage`, `--coverage 0`,
  `dive tlc -coverage 0`); external inputs (`--allow-io`); and recovery of a
  checkpoint archive in an older pre-release format. There is no alternative
  engine for them.
- Boolean recursive predicate guards keep their
  [D-029](#d-029-predicate-witness-counts) successor counts under
  `--compat=tlc` on Core. `byzpaxos/VoteProof` succeeds with TLC's 6962
  distinct states and depth 14 but generates 27727 states instead of TLC's
  318877. The harness registers this exact pair as a TLC-profile compat gap
  (`d029-voteproof-tlc` in `conformance/expected-divergences.toml`) and reports
  it as `expected-divergence`, never `agree`; any other difference stays a
  finding.

Each gap keeps its actual refusal or measured difference. Internal errors remain
defects. None permits a false verdict or weakens counterexample validation.
The entries below describe their implemented scope; any exact-compatibility
claim in them is subject to this list.

**Status values:**
- **not implemented:** decided, not built; the option is refused.
- **implemented:** built and tested in both modes.
- **default:** built, and on by default.

**Kinds:**
- **Same answer, better path:** dive reaches TLC's answer in a better way, for example faster, in parallel, or deterministically.
- **Different answer:** dive's default answer can differ from TLC's, for reasons given in the entry.

---

## D-001: Parallel checking gives the same answer at any worker count

| | |
|---|---|
| **Kind** | Same answer, better path |
| **Default** | Deterministic parallel search; workers on by default |
| **TLC behaviour** | `--workers 1` reproduces TLC's exact breadth-first results |
| **Status** | implemented (`dive check --workers N`, M5 WP-D3): the deterministic parallel engine of [deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md). Every result is identical at every worker count and under randomized scheduling, and equals TLC's `-workers 1` golden under `--compat=tlc`. Specs that depend on TLC's sequential side effects run in *sequential mode* and say so (see below). |

Explicit [native distributed safety](user/distributed-safety.md) extends this
ordered-commit contract to authenticated trusted worker processes ([authenticated native safety workers design](decisions/0151-authenticated-native-safety-workers.md)).
Distribution stays off by default and under the TLC-equivalent local setting.
The initial pure Core slice preserves logical results through complete-source
retry; broader distributed recovery, temporal/storage/effect combinations and
performance qualification remain unfinished.

**What TLC does.** With more than one worker, TLC's breadth-first search is only approximately breadth-first (`tlc2/tool/TLCTrace.java`; tlaplus#883). Three things change from run to run and with the worker count:

- the number of states found before a violation;
- the reported search depth;
- the counterexample, which may not be the shortest.

Measured on the pinned TLC, on one spec with a safety violation:

| Workers | States explored when the violation was found | Counterexample length |
|---|---|---|
| 1 | 63,251 | 46 |
| 8 (three runs) | 64,416; 67,497; 65,409 | 46 |

On Examples `DieHard` with 4 workers, TLC reports depth 9 for a 7-state counterexample. The pinned TLC also has an upstream multi-worker race (tlaplus#1445, fix pending) that can produce bogus states. As a result, the only TLC results you can fully trust are single-worker ones, which are slow.

**What dive does.** Parallel search is designed to be deterministic. Any worker count gives the same verdict, the same state counts, the same depth, and the same shortest counterexample as TLC's single-worker run. You get the speed of parallel checking without giving up exactness.

How it works, in short: dive explores one breadth-first level at a time. Workers expand states in parallel, but every successor carries its *position* in TLC's single-worker order, and when two workers find the same new state the one with the earlier position wins. The first violation, deadlock or error in that order stops the search, and the counts are computed for exactly that point. Which worker happened to do what never enters the result. `Print` output is buffered and released in TLC's order too.

**Sequential mode.** A few specs observe TLC's evaluation order directly, so no parallel search can reproduce them: they write TLC registers (`TLCSet`), read live counters (`TLCGet("distinct")` and friends), draw random values (TLC draws from one stream per expanded state), perform I/O, or have side effects inside lazily evaluated sets or functions. dive detects these statically and checks them with one worker, in TLC's order. The output says so: the header reads `4 worker(s) requested, 1 used` followed by the reason, and the JSON `options.workers` block has `"sequential": true` and the reason. `--sequential` forces this mode for any spec; results never change.

**Measured.** On the Examples small tier (`--workers 1,3,4,8`):
- **Default profile.** Every checked model gives the same results at every worker count.
- **Byte-identical JSON.** For the 60 agreeing models that run in under a minute, the JSON report (timing and thread fields aside) is byte-identical at 1, 3, 4 and 8 workers and under chaos scheduling, in both profiles.
- **`--compat=tlc`.** 70 models agree with TLC's single-worker golden at every worker count. The one exception is by design: `DieHard/MCDieHardest` asserts `TLCGet("config").worker = 1`. Under `--compat=tlc` dive reports the physical worker count as TLC does ([D-011](#d-011-tlcgetconfigworker-reports-the-logical-worker-count)), so at `--workers 4` the assumption fails exactly as it does in TLC at `-workers 4`. In the default profile it reports 1 and the model agrees at every worker count.

Scaling figures are in [deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md) ("Implementation and measurements (WP-D3)").

**Evidence.** Probe P8 in `docs/internal/survey/competitive.md`, [worker count and depth semantics design](decisions/0004-worker-count-and-depth-semantics.md), [tlc compatible defaults dive enhancements design](decisions/0010-tlc-compatible-defaults-dive-enhancements.md), [deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md), Core's worker-invariance tests (`crates/dive-api/tests/core_parallel.rs`: every Core fixture at 2, 3 and 8 workers against 1 worker), and the harness worker-invariance reports in `docs/conformance.md`.

---

## D-002: Randomness is always replayable

| | |
|---|---|
| **Kind** | Same answer, better path (reporting); different answer only when you fix a seed |
| **Default** | Fresh seed per run, as in TLC, but **the seed is always reported**; `--seed S` alone replays a run exactly, at any worker count |
| **TLC behaviour** | TLC's seeding and TLC's draws, bit for bit, under `--compat=tlc` (given TLC's `-seed S` and `-fp N`) |
| **Status** | implemented and wired into checking, in both modes, and verified against TLC. `dive check` accepts `--seed`, always reports the seed (and the fingerprint polynomial), and draws `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` from the run's streams: the start-up stream for constant definitions, `ASSUME`s and `Init`, and a stream per expanded state. On admitted untransformed routes under `--compat=tlc`, draws match TLC; this is not a claim of exact trace printing at every random stopping boundary (fixtures `TmRandom` and `TmRandomStartup` in `crates/dive-api/tests/fixtures/checker/tlc-module/`). Like TLC, dive rebuilds a counterexample from a fresh start-up stream; when draws before `Init` make that impossible, dive reports the violation without a trace, as TLC does, and says why. `RandomSubsetSet` is still refused by name. The independent trace validator refuses every random draw made by a check: the run's seed and polynomial do not establish its stream position independently of successor generation ([trace validator independence design](decisions/0016-trace-validator-independence.md), amendment 3). Its TLC JSON interpretation heuristic scores only pure initial conjuncts, so it never consumes draws or writes registers before replay. |

**What TLC does.** `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` draw from an unseeded random source during model checking unless you pass `-seed`. A spec that uses them can produce a different state space on every run.

| Spec | What we measured on the pinned TLC |
|---|---|
| Examples `SpanningTree/SpanTreeRandom` | 1,878 / 40,460 / 65,396 / 106,664 / 240,806 distinct states across runs with identical flags |
| `RandomElement(1..1000000)` in `Init` (probe P9) | 599,378, then 344,064, then 43,074. With `-seed 7`: 730,700 three times |

The seed is not shown in the normal results, so a surprising run cannot easily be reproduced. Even `-seed` is not always enough:

- **`-seed` alone does not replay draws made in `Next`.** TLC draws in `Next` from a generator seeded with *the state's fingerprint* XOR the seed, and TLC picks the fingerprint polynomial at random unless you also pass `-fp`. With an action that draws `RandomElement(1..1000000)` from the state `x = 0`, three runs with `-seed 7` drew 560,481 (polynomial 71), 618,858 (55) and 37,068 (1). With `-seed 7 -fp 0` the draw is always 772,259.
- **A draw before `Init` can lose the counterexample.** TLC draws constant definitions, `ASSUME`s and `Init` from one generator. To print a counterexample it evaluates `Init` again with a reset generator, so an `ASSUME` that drew first shifts every draw of `Init`. When the violation comes from an initial state that the second evaluation no longer produces, TLC reports the violation and exits with code 12, but prints no trace at all:

  ```tla
  ASSUME PrintT(RandomElement(1..1000000))
  VARIABLES x, y
  Init == x \in {RandomElement(1..1000000), RandomElement(1..1000000)} /\ y = 0
  Next == y' = y + 1 /\ x' = x
  Inv == y < 3
  ```

  With `-seed 7` the trace is missing; with `-seed 0` it is printed. It depends on which initial state reaches the violation first.
- **A `LET` does not fix a random value.** In `Init` and in actions, TLC draws a `LET`-bound `RandomElement` again in each conjunct that uses it, so the value printed is not the value assigned. With `Twice(y) == <<y, y>>`, this `Init`:

  ```tla
  Init == LET d == RandomElement(1..1000000) IN
          /\ PrintT(<<"init", d, Twice(d)>>)
          /\ x = d
          /\ PrintT(<<"init-x", x, d>>)
  ```

  printed `<<"init", 749170, <<348310, 348310>>>>` and then `<<"init-x", 897278, 708178>>` (with `-seed 7`, after an `ASSUME` had drawn once). Within one constant-level expression, such as an `ASSUME`, the value is drawn once.

**What dive does.** dive also draws a fresh seed by default, and always reports it in both human and JSON output. `--seed S` reproduces the run exactly, at any worker count:
- each random stream depends only on the seed, a key, and how many values have been drawn from it. The key is the expanded state's fingerprint for draws in `Next`, a fixed key for constants, `ASSUME`s and `Init`, and the trace number in simulation;
- dive's state fingerprint is fixed, so the seed alone is enough;
- counterexamples use ordered replay. A startup draw before Init can still make that replay lose its trace, matching the pinned TLC limitation; dive never invents a replacement witness.

The operators choose with TLC's algorithms (which element, which subset); only the source of randomness differs. A dive run that cannot be reproduced from its reported inputs is a bug.

Under `--compat=tlc`, supported untransformed checking reproduces TLC's choices for a fixed `-seed` and `-fp`: Java's `java.util.Random`, TLC's per-state seeding and each operator's algorithm. Next-state draws use TLC's variable order ([evaluator architecture design](decisions/0014-evaluator-architecture.md)), and LET expressions are evaluated where TLC evaluates them. Checking-path randomness runs inline with its actual width reported.

**Randomness with VIEW or SYMMETRY.** These combinations are explicitly
unsupported before binding, output or search, including unused source calls and
configured native aliases. The rescue removes transformed-stream admission
proofs rather than certifying a selected stored-value shape. Ordinary pure VIEW
and untransformed per-state RNG remain available. Earlier pinned transformed
stream measurements remain historical facts; they are not current support.
Graph, temporal and checkpoint/recovery randomness remain separate limitations. The internal native
`RandomSubsetSet` helper still reports its named evaluator limitation; configured
replacement selection does not make that helper implemented.

**Start-up-fixed randomness with temporal properties.** A zero-arity
constant-level definition such as SpanTreeRandom's
`Edges == UNION {{{n, m} : m \in RandomElement(SUBSET (Nodes \ {n}))} : n \in Nodes}`
is evaluated once at start-up, from the reported seed, and its value is kept for
the whole run, as TLC keeps it. Checking never draws again, so both engines check
temporal properties of such a model on the fixed graph. A draw while checking,
including through an operator with parameters, still refuses. So does a draw in
a definition that a configured `Op <- D` or `C <- D` targets: TLC evaluates
`D`'s body at every use of `Op` or `C`, so `D` is not fixed. Nor is a
definition whose start-up value keeps a lazy set, such as
`S == {n \in Nat : n = RandomElement(0..1)}`: each membership test evaluates
the filter's predicate again, so it draws while checking and refuses. Under
`--compat=tlc` the draws are TLC's: `SpanningTree/SpanTreeRandom` at `-seed 0`
gives TLC's 79,550 distinct and 1,357,252 generated states. The default profile
draws from dive's own streams, so its counts differ (22,040 distinct and 293,361
generated at seed 0, a registered expected divergence). A temporal violation of
such a model still refuses: the independent validator does not replay start-up
draws, and an unvalidated candidate is not reported as a violation.

**Configured random calls.** The minimal model below selects a native
operator through its configuration rather than a source call:

```tla
CONSTANT C(_)
VARIABLES n, x
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == x # 80
```

With `CONSTANT C <- RandomElement`, clean pinned TLC at `-seed 7 -fp 0 -workers 1`
reports an invariant violation after two states. The old native route drew 74
instead of 80 and missed that violation; `Inv == x # 74` instead produced a false
native violation where TLC completed four states. The immediate admission repair
and those exact false-answer controls remain historical evidence.
The ordinary `RandomElement` stream now follows the actual bound callee, with
startup and per-predecessor streams and truthful inline worker reporting.
Non-VIEW configured native Randomization targets use that same bound-callee
classification and per-state stream as direct calls. The focused tests compare
these routes; this is not a claim that every configured random family is
covered. Transformed, graph and recovery random paths remain
explicitly unsupported.

Clean TLC also exposes a replay limitation on that `x # 80` model: exit 12,
generated/distinct/depth 2/2/2, but only the initial `x=0` state is printed. Its
initial-predecessor fast path reuses the original state object;
`TLCStateRandom.isInitializedFor` uses object identity and retains the consumed
stream. With `Inv == n<3`, TLC prints all four states `x=0,80,94,100`. Both dive profiles
currently refuse the first-successor random replay boundary, retaining only
observed prefix statistics and no counterexample, rather than claiming that a
truncated trace is valid or that a different full trace matches TLC. Both direct
and configured clean calls are captured separately in the
bound-stream study.

**Evidence.**
- Harness review finding M5, probe P9, [tlc compatible defaults dive enhancements design](decisions/0010-tlc-compatible-defaults-dive-enhancements.md)
- TLC runs captured with the harness at seeds 0, 7, −3, 1234567890123, 2^63−1 and −2^63 (`crates/dive-stdlib/tests/fixtures/random/`). dive reproduces all 728 values TLC printed, and the text of all 10 error cases (`tests/tlc_random.rs`). The fixtures `RndTraceAssume` and `RndLetRedraw` are the two examples above.
- `java.util.Random` and TLC's `RandomGenerator` match bit for bit (`tests/java_random.rs`).
- The first six draws of `-seed 0` reproduce the Examples golden of `SpanTreeRandom`: 79,550 distinct states and 1,357,252 generated (`tests/examples_random.rs`).
- dive's streams replay identically at 1 to 8 threads and in any processing order (`tests/dive_mode.rs`).

---

## D-003: Simulation is reproducible

| | |
|---|---|
| **Kind** | Different answer only when you fix a seed |
| **Default** | Seed reported; `--seed S` replays supported finite sampling; extra workers are refused |
| **TLC behaviour** | Under `--compat=tlc` |
| **Status** | the random streams are implemented in `dive-stdlib`: TLC's worker seeding, and dive's per-trace streams. Finite pure single-worker safety simulation is implemented (`dive-sim`, [finite seeded simulation design](decisions/0020-finite-seeded-simulation.md)); other modes, spec-level randomness/effects and additional workers are named refusals. |

**What TLC does.** Even with `-seed 0` and one worker, the number of states TLC generates in simulation varies between runs. On Examples `EWD840_anim` it ranged from about 267k to 271k.

From TLC's source, its simulation uses two kinds of generator, and neither is tied to the trace being generated:
- each worker chooses initial states, actions and successors with its own generator. That generator's seed is the next value of a generator seeded with `-seed` and advanced by `-aril`;
- each worker evaluates `RandomElement` and the `Randomization` operators with a generator seeded with `-seed` itself, and never reseeded, so every worker draws the same sequence of values. What a trace draws depends on which worker generated it and on what that worker generated before.

**What dive does.** The same seed gives the same supported finite simulation.
Each trace has one stream keyed by seed and trace number, driving state and
action selection. Adding more traces leaves the earlier ones unchanged, making
this usable in tests and CI. The architecture supports trace identities that
will remain stable across workers; additional workers and spec-level random
operator draws are not implemented yet and refuse by name.

**Evidence.** The pinned oracle reproduces all 30 published traces (172 states
after finite stutter removal) and 304 generated states across five seeds and
two arils in `crates/dive-sim/tests/fixtures/choice-oracle.json`. Twelve additional
clean probes compare error classes, exits and available counts. A thirteenth
probe exposes the pre-constraint action-property validation boundary: TLC
reports 2112, while the scoped simulation API explicitly refuses that
combination by name. The API's
independent validator accepts the real violations, rejects a deliberately
corrupted sampled edge, and refuses a deadlock whose absence of successors it
cannot decide. The default profile preserves earlier trace documents when the
requested trace count grows.

TLC's `SimulationWorker.getTrace` removes finite stuttering. In the seed-zero,
aril-zero probe, its third published trace has five states although six were
chosen. Dive retains the uncompressed concrete trace for validation and
provides `tlc_states` separately for TLC's display convention. A finite sample
always reports `exhaustive: false` and `model_proved: false`; a zero exit never
claims the model was proved.

The intended worker-independent default is not yet a parallel implementation:
requests for additional workers refuse explicitly. Liveness/fairness simulation,
random operators in the spec, runtime effects and the other limits listed in
`crates/dive-sim/CONTRACT.md` remain unsupported. Existing Java RNG and stream
controls live in `crates/dive-stdlib/tests/java_random.rs` and `dive_mode.rs`.

---

## D-004: Integers are unbounded

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Arbitrary-precision integers |
| **TLC behaviour** | `--int=tlc` (included in `--compat=tlc`): 32-bit integers with TLC's overflow error |
| **Status** | implemented in the value layer (`dive-value`, [value representation design](decisions/0012-value-representation.md)), in both modes. The CLI switch and the default flip follow once both modes pass the conformance harness. |

**What TLC does.** TLA+ defines integers as the mathematical integers, but TLC stores them in 32 bits. Anything outside −2,147,483,648 … 2,147,483,647 stops the run:

```tla
VARIABLE x
Init == x = 2147483646
Next == x' = x + 1
```

Measured on the pinned TLC:

| Expression | TLC's report |
|---|---|
| `2147483647 + 1` | `Overflow when computing 2147483647+1` (`EC.TLC_MODULE_OVERFLOW`; exit 255 during model checking) |
| `65536 * 32768`, `2 ^ 31`, `-(-2147483647 - 1)` | `Overflow when computing 65536*32768`, `… 2^31`, `… --2147483648` |
| `(-2147483647 - 1) \div (-1)` | `Overflow when computing -2147483648 \div -1` |
| `2147483648` | `TLC can't handle a number this big.` (`EC.TLC_INTEGER_TOO_BIG`, exit 255) |
| `-2147483648` | also rejected, although the value fits, because the literal under the minus does not |
| `Cardinality(SUBSET (1..31))` | `Overflow when computing the number of elements in: SUBSET (1..31)` |
| `Cardinality(-2147483647..2147483647)` | `Size of interval value exceeds the maximum representable size (32bits)` |

The fixed `ExplicitOuterPowersetCardinality` control computes
`Cardinality(SUBSET (SUBSET (1..5)))`. Pinned TLC fails during Init with GENERAL
(1000) and physical exit 75, wrapping the size overflow; the default profile
computes 4,294,967,296 and completes with one distinct state, two generated
states and depth one. Its expected [D-004](#d-004-integers-are-unbounded) difference is registered with exact
exit and generated-count pairs. TLC mode retains the overflow and replay exit.


**What dive does.** dive computes with unbounded integers, as the language intends. Small numbers still use an inline 64-bit fast path. With `--int=tlc` every one of the reports above is reproduced word for word. When a run would have stopped with TLC's overflow error, dive's output says so, so the difference is never silent.

**Why it matters.** Four of the five other Rust or Go TLA+ checkers we measured widen integers silently, with no TLC mode and no indication in their output. dive does better by default and is TLC-exact on request.

**Evidence.** Probe P1, harness fixture `Overflow`, [tlc compatible defaults dive enhancements design](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) (maintainer decision of 2026-10-08), [value representation design](decisions/0012-value-representation.md). The fixtures `Ov*`, `Ar*` and `ErrCardSubset`/`ErrIntvSize` in `crates/dive-value/tests/fixtures/` hold TLC's transcripts, and dive's messages match them byte for byte.

---

## D-005: Symmetry with liveness uses the full state space

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Supported unreduced checking; disable symmetry for the whole genuine temporal run |
| **TLC behaviour** | `--symmetry-liveness=tlc` (included in `--compat=tlc`): warn and proceed |
| **Status** | unreduced/refuse implemented for supported temporal runs; TLC quotient remains refused |

**What TLC does.** Symmetry reduction lets TLC explore only one representative of each group of states that differ only by a permutation of interchangeable values, such as process ids. Combined with liveness checking ("something good eventually happens"), symmetry can make TLC miss real violations. TLC knows this. It prints a warning ("Declaring symmetry during liveness checking is dangerous. It might cause TLC to miss violations…", `EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), but then carries on and can report success.

Measured on the pinned TLC, it goes wrong in both directions:

- **A missed violation.** `x` moves between three interchangeable model values `a`, `b`, `c` at every step, under weak fairness. `PROPERTY []<>(x = a)` is false: `a, b, c, b, c, …` never returns to `a`. Without `SYMMETRY`, TLC finds that counterexample. With `SYMMETRY Permutations({a, b, c})`, TLC prints the warning and reports **no error**, after exploring 1 state.

  ```tla
  Next == x' \in S \ {x}
  Spec == Init /\ [][Next]_x /\ WF_x(Next)
  InfOftenA == []<>(x = a)
  ```

- **A false violation.** A token alternates between two interchangeable processes under weak fairness, and the property "every process holds the token infinitely often", `\A p \in S : []<>(tok = p)`, is true. The spec and the property are both symmetric. With `SYMMETRY`, TLC reports `Temporal property Fair was violated`, with a counterexample of one state followed by "Stuttering". The fairness condition forbids that behaviour.

**What dive does.** Supported default-profile temporal runs check the whole unreduced state space and report that symmetry reduction was disabled. Explicit `refuse` remains available. Safety-only roles keep their existing symmetry reduction; actual genuine temporal roles select the whole-run unreduced path.

`--symmetry-liveness=tlc`, including its selection by `--compat=tlc`, requests TLC quotient semantics but currently refuses. No missed/false TLC quotient verdict, quotient counterexample annotation or permutation-closing validation is claimed.

**Evidence.** [tlc compatible defaults dive enhancements design](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) (maintainer decision of 2026-10-08); [symmetry design](decisions/0019-symmetry.md); probes `SymLiveMissed`, `SymLiveMissedNone`, `SymLiveSymProp` and `SymLiveWarn` in `crates/dive-liveness/tests/fixtures/tlc-probes/`; survey probe P4 (another checker raised a false liveness alarm under symmetry).

---

**Update, 2026-10-09.** The default is `--symmetry-liveness=unreduced`: ignore `SYMMETRY` for the entire liveness run, including its safety checks, and report the unreduced counts. This replaces the earlier approved refusal default. `=refuse` remains available. `--compat=tlc` requests `=tlc`; that route remains refused rather than claiming reproduction of TLC's missed and false violations. The unreduced and explicit refusal modes now implement this policy for supported temporal checks. TLC quotient liveness remains a named refusal, including under `--compat=tlc`; it does not silently select the sound mode. Explicit `--compat=tlc --symmetry-liveness=unreduced` reports `tlc+overrides`. Compare the sound mode against TLC run without symmetry, rather than excusing arbitrary verdict differences. Original declared symmetry and the actual bound whole-run choice are reported separately. VIEW quotient checking and its concrete witness reconstruction remain unsupported.

## D-006: `CHOOSE` does not depend on unrelated text in the module

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | A canonical order, independent of the module text |
| **TLC behaviour** | `--choose-order=tlc` (included in `--compat=tlc`): TLC's order, including string interning order |
| **Status** | implemented in the value layer (`dive-value`, [value representation design](decisions/0012-value-representation.md)), in both orders. `--choose-order=tlc` also needs names recorded in TLC's encounter order: `dive-model` does that, and on 556 corpus models no name both tables hold is out of TLC's order ([value representation design](decisions/0012-value-representation.md), "For the front end"). |

**What TLC does.** `CHOOSE x \in S : P` picks the first element of `S` that satisfies `P`, in TLC's internal value order. For strings, that order is the order in which each string *first appears in the module text*, not alphabetical order. So adding an unused definition can change what `CHOOSE` returns, and can flip a verdict:

```tla
EXTENDS Integers
Unused == "alpha"   \* add or remove this line
VARIABLES s
Init == s = CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE
Next == UNCHANGED s
SLex == s = "alpha"   \* checked as an invariant
```

| | TLC picks | Invariant `SLex` |
|---|---|---|
| Without `Unused` | `"zeta"` | **fails** |
| With `Unused` | `"alpha"` | holds |

The same order decides how TLC *prints* values, in `Print`, error messages and counterexamples. Measured on the pinned TLC:

- **Names** (strings, model values and record field names) are ordered by when TLC first stored the name.
  - A few names are stored before parsing starts, such as `TRUE`, `x`, `S`, `a`, `b`, `c`, `y` and `z`.
  - The parser then stores identifiers in source order.
  - String contents are stored after that.
  - So `{"a", "b", "zebra", "Init", "c"}` prints in exactly that order, and `[qfb |-> 1, qfa |-> {1}]` prints `qfb` first when `qfb` appears first in the module.
- **Sets** order by size first, then element by element: `{{}, {1}, {2}, {0, 5}, {1, 2}}`.
- **Tuples and other functions** order by length (domain size) first, then by domain, then by values: `{<<>>, <<1>>, <<2>>, <<0, 9>>, <<1, 2>>}`.
- **Kinds:**
  - untyped model values come before every other value;
  - `FALSE` comes before `TRUE`;
  - comparing other kinds is an error ([D-009](#d-009-comparisons-that-tlc-refuses-although-tla-answers-them)).
- **Equal elements keep the first form written.** `{1..3, {1, 2, 3}}` prints as `{1..3}`, while `{{1, 2, 3}, 1..3}` prints as `{{1, 2, 3}}`.

**What dive does.** dive uses a canonical order that does not depend on the module text: names in the lexicographic order of their characters (Unicode code points), and everything else structured exactly as TLC orders it. Reordering or adding definitions never changes a result. This is one of the charter's metamorphic guarantees. With `--choose-order=tlc`, the order and the printing match TLC's.

**Evidence.**

- Probes P2 and P7 in `docs/internal/survey/competitive.md`.
- [tlc compatible defaults dive enhancements design](decisions/0010-tlc-compatible-defaults-dive-enhancements.md) (maintainer decision of 2026-10-08) and [value representation design](decisions/0012-value-representation.md).
- In `crates/dive-value/tests/fixtures/`:
  - `ChooseIntern` and `ChooseInternUnused` (this example);
  - `PrintChoose`, `PrintSets` and `PrintFcns` (122 printed values in all, matched byte for byte).

Pure `Json!ToJson` uses [D-006](#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)'s canonical default object/set order. TLC mode
refuses objects with two or more keys and compound keys containing multi-field
records, including nested tuple/set/function children, because normalized dive
functions do not retain TLC's construction history. Scalars, sequences, finite
sets and bounded keys without missing history have a native conversion path;
strings use Gson escaping without HTML escaping. Domain traversal obeys the
configured evaluation-depth guard before printing and refuses symbolic/deferred
set expansion. Array/Object entry points and JSON file operations remain refused.
The consumed Json fixtures preserve historical oracle observations; focused
native/pinned-oracle controls and independent review cover the supported path.
Independent replay accepts already-interned JSON strings. Newly created runtime
names in TLC-profile deciding words retain the existing named refusal,
so these words earn no agreement. See [the module guide](user/modules.md).

---

## D-007: Exhaustiveness claims can be unconditional

| | |
|---|---|
| **Kind** | Same answer, with a stronger guarantee |
| **Default** | Fingerprint mode, as in TLC, always reporting its collision bound |
| **Better option** | `--store exact`: full states, with an unconditional claim that every state was checked |
| **Status** | implemented: `dive check --store exact` runs the same search over canonical state encodings, for safety and temporal properties. It gives the same results as fingerprint mode (`exact_storage_runs_on_the_core` in `crates/dive-api/tests/core_default.rs`; temporal fixtures in `crates/dive-api/tests/core_liveness.rs`). Ordered safety-frontier spill is available with `--frontier-memory-bytes N` (zero forces disk), under both profiles/stores. It preserves concrete traces and ordered counts, and reports explicit I/O/cleanup failures. Visited stores, candidates, traces and graphs remain outside the frontier-only budget. See [frontier storage](user/storage-recovery.md). Separate predecessor spill is available for safety. Temporal checks refuse spilled or reclaimed storage by name; graph spill is not implemented. |

**What TLC does.** TLC stores 64-bit fingerprints of states, not the states themselves. A collision between two distinct states would silently skip part of the state space. TLC prints a probabilistic estimate of this at the end of a run, but the "no error found" verdict itself is probabilistic.

**What dive does.** dive offers an exact mode that stores full states and makes an unconditional exhaustiveness claim. Every result says which mode produced it.

**Temporal properties.** With `--store exact`, liveness checking builds its
behavior graph from full state identities: two states whose fingerprints
collide stay two vertices. TLC's liveness graph is keyed by fingerprint, so a
collision can merge states and change the answer. dive's human report states
the claim ("Exact full-state storage." or "Fingerprint storage (64-bit);
collision probability not computed."). A spilled store with temporal
properties is still refused by name.

```sh
dive check Spec.tla --store exact --output json
```

**Separate disk predecessor option.** Safety BFS accepts
`--predecessor-memory-bytes N` (zero goes straight to disk). This moves the
parent/ordinal records used to rebuild concrete counterexamples, with separate
capacity/write/cleanup evidence. It does not bound the seen set, frontier,
candidate or reconstructed-path memory. Omit the setting for the existing memory
counterpart in either profile. Storage and cleanup failures never certify
completion. Simulation, DFID and graph capture refuse this staged setting.
See [predecessor storage](user/storage-recovery.md).

**Evidence.** Charter, soundness contract rule 4; zero/tiny-budget ordered and
independent-witness controls in `crates/dive-api/tests/predecessor_spill.rs`.

---

**Checkpoint evidence (2026-10-10, staged).** The finite grid in
`conformance/probes/native-checkpoint/` finishes uninterrupted at 5,123,201
states generated and 2,563,201 distinct. Pinned TLC recovery resets generated
and initial counters to its saved distinct count; the captured resumed run reports
3,703,060 generated. Its developer timed stop also reports success while 1,511
states remain queued. Native checkpoint pauses explicitly report an
incomplete search. The bounded pure safety slice resumes the actual saved
frontier without Init regeneration; [native checkpoint documentation](user/storage-recovery.md)
lists its named exclusions and trusted-local, non-certificate contract. Recovery keeps the TLC counter-reset counterpart; retaining
original totals is the named, opt-in `--checkpoint-counters retain` behavior,
with no new default approved. Raw successful and failed recovery controls,
source hashes and the minimal grid precede implementation. Java/native byte
interoperability and broader runtime state remain deferred.

**Parallel recovery continuation.** Admitted pure safety checkpoints can resume
through the deterministic pool, at 1/4/8 workers or a different width from the
pause. Completed-level barriers retain the logical discovery/predecessor/frontier
order; native counter-limit stops match inline prefixes. Boundary-zero pauses
report actual 1 before the pool starts. This removes a dive limitation, not a
claim that TLC cannot recover parallel runs. Counter defaults, archive bounds
and trust qualifications are unchanged; runtime/temporal recovery remains
refused. See the separate parallel study.

**Configured-key recovery.** Pure supported VIEW and safety symmetry
now recover through the same reduced-key path as live search. The primary
minimal CpView/CpSymView/CpSymConcrete controls report 7/4/4, 5/3/3 and 3/3/3
(generated/distinct/depth) on pinned TLC; ordinary dive agrees. CpSymConcrete
keeps the value p2 in the actual witness even though the canonical key represents
p1. Recovery preserves that concrete state. Equivalent keys do not make a changed
Init or Next witness valid, and archives remain trusted local state, not proof.
Both profiles/stores retain the TLC counter-reset default; Retain is still
explicit.
Public Rust context aliases resolving constant replacements are admitted by their
actual definition IDs before binding. An effectful local alias refuses before
output or an archive; structurally valid arena metadata alone is not a purity
certificate. This is a native admission repair (), not a TLC defect. See raw controls.

[D-007](#d-007-exhaustiveness-claims-can-be-unconditional) native recovery additionally composes with the existing opt-in frontier
and predecessor budgets, independently or together. This removes a native
limitation; it is not a new TLC defect or result-changing default. Saved concrete
frontiers/parents are reconstructed into fresh transient stores in reference
order. Archive/discovery workspace is excluded from both budgets; no total RAM,
Java byte-interop or archive certificate claim. Counter reset/Retain and pure
VIEW/symmetry witnesses retain their existing semantics. See
[the runnable recovery guide](user/storage-recovery.md)
and separate preserved study.


**Native recovery admission update ([D-007](#d-007-exhaustiveness-claims-can-be-unconditional), 2026-10-11):** pure runtime string/name
construction is admitted with the complete saved ordered name table. For
`n' = n + 1 /\ s' = ToString(n + 1)`, the bounded recovery example reaches the
same 4-state invariant violation as uninterrupted checking and clean pinned TLC.
TLC encounter order uses a reported actual one-worker inline fallback; the
canonical profile may use the parallel engine. Runtime effects/register reads
and independent TLC witness replay of name history remain honest refusals. This
removes dive's own checkpoint limitation, not a reference implementation defect.
See `docs/native-checkpoint.md` and the source/raw control study at
`conformance/probes/native-checkpoint-runtime-names/`.


## D-008: Results are data, and diagnostics are built for people and tools

Coverage is not implemented on Core. `dive check --source-coverage` (native
source work) and `--coverage 0` (TLC `-coverage 0`, logical action-context
costs) are named refusals in both profiles; both default to off, the
TLC-equivalent setting. Only the removed reference engine collected them
. See [coverage](user/checking.md#coverage).

[D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents) PlusCal original AST data: `dive translate --ast` returns an earlier-stage
`AST.tla` representation in memory. Exit0 and `artifact_available: true` mean bytes
are available; `accepted` and `generated_module_accepted` stay false. Both profiles
use this explicit native operation. Explicit `translate --write` publishes an
admitted generated module, and pinned `-writeAST` writes the original AST and
exits 255; see the current PlusCal guide for their filesystem contracts.
Bounded simple debug/header shapes
match retained primary CRCs, including measured compounds and macro substitutions.
Modern original procedure/macro ownership is measured before generation; generated
debug labelled-compound and measured flat procedural headers now match exact CRC
pairs. Compound/process-owned procedures, renaming/ordering and historical fields
remain unfinished. The retained SANY semantic successes carry warnings and do not
prove clean original PlusCal checksum validation.
See [the current PlusCal guide](pluscal-translation.md).

**Current PlusCal translation.** Scoped renaming, multiline macro substitution,
branch labels, symbolic process domains and procedure frames follow pinned
`pcal.trans`. Existing translation blocks are replaced in their original slot.
Native expansion limits and generated-operator collisions are explicit
refusals. Public artifact acceptance still requires enclosing-module syntax,
name and level analysis: pcal can write a `PrintT` reference without `EXTENDS
TLC`, but dive rejects that module. Optional algorithm CRC serialization keeps a narrower procedure-free
scope without gating rewriting. See [the PlusCal guide](pluscal-translation.md).

| | |
|---|---|
| **Kind** | Structured answers preserved; checker exit conventions are separately selectable ([D-023](#d-023-default-process-exits-are-distinct-conformance-observables)) |
| **Default** | Human-readable output; `--output json` gives a versioned schema |
| **TLC behaviour** | TLC-style output conventions under `--compat=tlc` |
| **Status** | implemented: every command offers `--output json` with a versioned schema |

**What TLC does.** Results come as human-oriented text, or as the `-tool` message stream meant for the Toolbox. Counterexamples are available as JSON only through `-dumpTrace json`. SANY's error messages give a location, but no stable error codes and no suggested fix. SANY's command line exits 0 on semantic errors unless you pass `-error-codes`.

The `dive tlc`/`tlc` checking adapter selects TLC evaluation and one worker
by default, normalizes module/configuration suffixes and emits existing human or
JSON check reports. `-lncheck final` directly selects the bounded deterministic
short-lasso backend ([D-013](#d-013-a-deterministic-short-liveness-counterexample)), with its existing limits and independent word
validation. Finite `-simulate num=N` and `-generate num=N` select independently
validated non-exhaustive sampling; their unbounded/file protocols still refuse.
Tool framing and other TLC mode protocols refuse by name. The adapter reads and parses the configuration before the module, matching TLC's
input-error priority when both files are invalid.
See the [entrypoint guide](user/checking.md#tlc-checking-entrypoints) for exact supported
flags, path resolution and statuses.

**`dive check` exit codes.** TLC physical statuses are now the CLI default in
both evaluator profiles ([check exit conventions design](decisions/0097-check-exit-conventions.md)). `--exit-codes native` retains0 success,
1 rejected input,10 property violation,11 deadlock,12 assumption and13 evaluation
error. Both conventions retain2 usage,3 unsupported/resource and4 internal error.
`--compat=tlc` also selects TLC evaluation; an explicit native exit override is
reported as `tlc+overrides`. Historical Rust `CheckReport::exit_code()` remains
profile-based. JSON discloses the semantic profile, selected convention and
physical status. See [the checker guide](user/checking.md#process-statuses).


**Finite temporal-prefix exit codes.** With `S = {}`, this property is false
already in the initial state:

```tla
CONSTANT S
VARIABLE x
Init == x = 0
Next == x' = x
P == \E i \in S : []<>(x = i)
```

The pinned TLC reports `Temporal property P was violated` (2116), prints one
finite initial state, and exits **12**, normally the safety status. Its counts
are 2 generated, 1 distinct, depth 1. `LiveCheck.printErrorTrace` deliberately
stores invariant code 2110 after printing temporal code 2116. A consumer that
classifies solely by exit status mislabels the violation.

The harness preserves the temporal class and actual exit status after checking
this specific finite stream shape. Ordinary cyclic temporal failures still
require exit 13. The raw evidence lives in
`crates/dive-conformance/tests/fixtures/temporal-prefix/`. This is an oracle
classification fix, not a new result-changing default. Reproducing TLC's
finite-prefix stopping point in the checker remains phase-L work; the bounded
final Rust API returns an independently validated periodic witness.

**What dive does.** Every diagnostic has:

- a stable code (for example `DIVE-S0010`);
- a precise source span;
- a message that names the construct;
- a likely fix, where we know one.

`--output json` gives verdicts, statistics and counterexamples as structured state sequences, along with diagnostics and an `options` block that records the active profile, options and seed. Scripts, tools and Rust callers can act on this programmatically. The library API (`dive-api`) gives the same results in-process, with no JVM.

`dive analyze` also gives every semantic diagnostic the SANY error code it corresponds
to (`sany_code`), so tools written against SANY's codes keep working.

**Evidence.** [syntax tree and parser design](decisions/0009-syntax-tree-and-parser.md), [semantic analysis design](decisions/0011-semantic-analysis.md), `dive parse --output json`, `dive analyze --output json`, and probes M1 and M2 (diagnostic comparison in `docs/internal/survey/competitive.md`).

---

**Bounded PlusCal multiline reports.** Sequential non-junction multiline
expressions now generate with reference lexical changed-variable substitution.
LF/CRLF, tabs, blank/comment lines and UTF-8 comments have separate retained
controls. Junction indentation, binders and macro/process/procedure expression
contexts still yield named unsupported reports. Full/body CRCs, whole operator
CST/SANY admission and finite counts/independent witness checks are separate;
neither a checksum match nor a translation report claims a checked model.
The input file is unchanged. See `docs/pluscal-multiline.md` and its complete
primary/refusal history. [D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits)'s fair-only source-detection weakness is preserved,
not silently corrected.

---

## D-009: Comparisons that TLC refuses although TLA+ answers them

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | **TLA+ answers** for `=` and `\in` between functions, records, tuples and sets (`--comparisons=tla`; maintainer decision 2026-10-08). The error is kept only where TLA+ leaves the answer unspecified, such as `1 = "a"`. Core finite-set construction is canonical ([core is the product design](decisions/0153-core-is-the-product.md); [D-028](#d-028-canonical-finite-sets)); direct scalar type errors stay unchanged. |
| **TLC behaviour** | `--comparisons=tlc`, part of `--compat=tlc` |
| **Status** | default in `dive-eval` (`Comparisons::Tla`). TLC's behaviour is implemented and matched in `dive-value` and `dive-eval` under `--compat=tlc`. |

**What TLC does.** TLC refuses to compare values of different kinds, which catches real mistakes such as `1 = "a"`. But it also refuses comparisons whose answer TLA+ defines, and its refusals are asymmetric and depend on the order in which elements are written. Measured on the pinned TLC:

| Expression | TLC |
|---|---|
| `<<1>> = [a \|-> 1, b \|-> 1]` | `FALSE` |
| `[a \|-> 1, b \|-> 1] = <<1>>` | error: `Attempted to check equality of record: … with non-record <<1>>` |
| `{[a \|-> 1, b \|-> 1], <<1>>}` | a set of two elements |
| `{<<1>>, [a \|-> 1, b \|-> 1]}` | error: `Attempted to compare record: … with non-record <<1>>` |
| `<<1>> \in [a : {1}]` | error: `Attempted to check if non-record <<1>> is in the set of records` |
| `[a \|-> 1] \in {1} \X {2}` | error: `Attempted to check if non-tuple …` |
| `1 \in {1, "a"}` | `TRUE`, because TLC sorts a written set only when it must |
| `"a" \in {1, 2}` | error |

The refusal even depends on *how* the function was built, not only on its value:

| Expression | TLC |
|---|---|
| `[qa \|-> 1, qb \|-> 2] = <<1>>` | error: a record literal refuses non-records |
| `("qa" :> 1 @@ "qb" :> 2) = <<1>>` | `FALSE`: the same function built with `:>` and `@@` compares sizes first |
| `[x \in {"qa", "qb"} \|-> 1] = <<1>>` | `FALSE` |

Whether TLC rejects a set also depends on which pairs its insertion sort compares, and so on the string order. `{[fx |-> "qzb", fy |-> 0], <<1>>, [fx |-> "qza", fy |-> 0]}` is a set in TLC (probe `RvCanonAsym`), but the same elements in another order are an error.

In TLA+, tuples and records are functions. Two functions with different domains are unequal, and a function is not a member of a set of functions with another domain. So each error above has a defined answer, `FALSE`. And `=` should not depend on which side a value is written on, or on how the function was built.

**What dive does today.**

- **Under `--compat=tlc`.** dive reproduces all of the above exactly, including the asymmetry and the dependence on how the function was built.
- **In the default profile, sets.** dive decides whether building a set is an error exactly as TLC does, by running TLC's insertion sort in TLC's name order, whatever order the set is then kept in.
- **In the default profile, comparisons and membership.** The approved
  `--comparisons=tla` setting uses extensional equality for functions and sets,
  and structural membership for function/record/sequence sets. It answers the
  defined mixed-structure cases symmetrically. Where TLA+ leaves the answer
  unspecified, it retains TLC's answer or error.
- **Mixed overrides.** Choosing TLC comparisons while retaining canonical
  value order can still change which incomparable pair a membership search
  meets. Such a run is mixed. The global `--compat=tlc` switch restores both
  comparison behavior and ordering together.

The maintainers approved TLA+ comparisons as the default on 2026-10-08. This changes
results for defined comparisons that TLC stops with an evaluation error; the
TLC counterpart remains implemented and tested. Set construction still follows
TLC's normalization rules, so some heterogeneous set literals remain refusals.

In the `ModelBadPrefix` corpus control, the configured model value `a` is a
member of `{1,a}` by reflexivity. The default answers `TRUE` and finishes with
two states; TLC encounters a typed-model comparison error before reaching that
element and exits75 after one state. The harness records this exact default
case as `expected-divergence`, never `agree`. TLC mode retains its actual
reference error or native refusal. No trace difference is excused, and this
case grants no answer for other mixed-type queries.

The `comparison-receiver-primary` corpus retains eleven existing default
comparison results as model-specific expected divergences. Its tuple/record
domain controls and nested tuple-value control complete by default; TLC stops
at construction-dependent comparison errors. Each registration binds its exact
exit and generated-count pair and only the fields that actually differ. The
Next division-by-zero control remains ordinary agreement. These registrations
change no evaluator behavior, grant no TLC-mode or trace-validation exception,
and add no strict parity credit.

**Evidence.**

- [value representation design](decisions/0012-value-representation.md).
- The fixtures in `crates/dive-value/tests/fixtures/` hold TLC's transcripts:
  - `AsymSetRecFirst`, `AsymSetTupleFirst`, `AsymEqTupleRec` and `AsymEqRecTuple`;
  - `ErrMemRcdTuple`, `ErrMemProductRec`, `ErrMemEnumStr` and `PrintMember`;
  - `RvFcnRcd1`–`RvFcnRcd8` and `RvCanonAsym`.

---

**VIEW and mixed structured sets.** Default-profile Core now checks both
`{<<1>>, [a |-> 1, b |-> 1]}` and the reverse literal order, including a VIEW
that projects the set away. Its emitted mixed-set word round-trips and validates
independently; altered states and a wrong invariant claim fail validation.
Earlier normalization and decoder refusals are superseded by [D-028](#d-028-canonical-finite-sets). The original
TLC behavior remains recorded in the pinned fixtures and git history.

The separate homogeneous recovery matrix retains 504 executions, 216 valid
original/recovered witnesses and 216 semantically rejected altered witnesses.
Source-qualified before and current evidence
retain the old wrong result separately. Memory VIEW also preserves TLC’s
nonforcing normalization of evaluator-owned lazy filters: `x = {i \in Nat :
Print(i, TRUE)}`, `Next == UNCHANGED x`, and `VIEW 0` complete with
2 generated / 1 distinct / depth 1 and no Print output on the pinned primary.
The checker visits the filter’s stored bound set without running its predicate.
Unknown providers and lazy set-operation cache normalization remain refused;
callback-bearing filters remain inadmissible to spill/checkpoint archives.
Full VIEW/random and deferred-value family support remain outside this repair.

## D-010: The order in which sets are explored

| | |
|---|---|
| **Kind** | Different answer, only at a violation |
| **Default** | **Canonical order** (`--explore-order=value`; maintainer decision 2026-10-08): sets are explored in the run's value order, which is canonical in the default profile ([D-006](#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)). Runs where the two orders would have stopped differently are flagged. |
| **TLC behaviour** | `--explore-order=tlc`, part of `--compat=tlc` |
| **Status** | default in `dive-eval` (`ExplorationOrder::Value`). `dive check` in the default profile notes, at every stop at a violation, deadlock or error, that the result may differ under `--compat=tlc`. dive does not detect whether the order actually mattered in a run. |

**What TLC does.** TLC explores the elements of a set in its internal value order: the elements of `\E x \in S`, `\A x \in S`, `{x \in S : P}`, `{e : x \in S}`, `[x \in S |-> e]` and `x' \in S`. For strings and model values, that order is the order in which names first appear in the module ([D-006](#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)). So the module text decides which successor TLC generates first, and with it which of two violations at the same depth TLC reports:

```tla
VARIABLE y
Init == y = "init"
First == "zeta"
Next == \E v \in {"alpha", First} : y' = v
InvAlpha == y # "alpha"
InvZeta == y # "zeta"
```

With `INVARIANT InvAlpha InvZeta`, the pinned TLC reports `InvZeta` after 2 states generated, because `"zeta"` appears in the text before `"alpha"`. Exploring in the canonical order reports `InvAlpha`, also after 2 states.

**What dive does.** In the default profile, dive explores sets in the canonical order, as it does for `CHOOSE` ([D-006](#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)), so the whole default profile is independent of the module text. Complete searches give the same states, totals and depth as TLC. Only *which* of several violations at the same depth is reported first, and the counts at that stop, can differ. dive flags runs where that happened. `--explore-order=tlc` (part of `--compat=tlc`) reproduces TLC's order exactly.

**Evidence.** Probe `StringOrderViolation` in `crates/dive-api/tests/fixtures/checker/tlc-probes/`; `crates/dive-eval/tests/options.rs`. Two Examples models show it in the default profile, with the same violated property, depth and counterexample length as TLC, and only the counts at the stop differing:

| Model | TLC (distinct / generated / queued) | dive default profile |
|---|---|---|
| `MissionariesAndCannibals` (`Solution`) | 61 / 225 / 11 | 61 / 226 / 11 |
| `acp/ACP_NB_WRONG_TLC` (`AC1`) | 11,852 / 59,286 / 1,248 | 12,645 / 64,357 / 970 |

Both are registered as expected divergences in `conformance/expected-divergences.toml`, narrowed to the fields that differ. Under `--compat=tlc` both agree with TLC exactly (the CM1 conformance gate).


The pinned PlusCal `SBB` model also differs only in distinct/generated counts:
canonical exploration reaches `Immutability` at depth 15 after **1,581 / 3,042**
states, versus **1,617 / 3,126** under TLC. Changing only the evaluator's
`Options::exploration` to `ExplorationOrder::Tlc` reproduces TLC's counts while
all other default semantics remain unchanged. Both PlusCal label variants were
checked; all six actual 15-state words validate independently. The SBB registry
entry applies only to this model and the default profile, with an exact generated
count pair. Property, depth, class, exit and word validity remain strict. Raw
queue counts remain evidence, without a queue-field exemption. The
[source-bound control record](../conformance/reports/core/2026-11-02-sbb-order.json)
retains input hashes and the diagnostic-only option change.

---

## D-011: `TLCGet("config").worker` reports the logical worker count

| | |
|---|---|
| **Kind** | Different answer (only for specs that read it) |
| **Default** | `1`, the *logical* worker count, at every `--workers` setting (maintainer decision 2026-10-08) |
| **TLC behaviour** | Under `--compat=tlc`, the physical worker count, as TLC reports it. `--compat=tlc` also defaults to 1 worker, as TLC does |
| **Status** | implemented: `TLCGet("config").worker` is 1 by default and the requested `--workers` under `--compat=tlc` |

**What TLC does.** `TLCGet("config").worker` returns the number of worker threads. Specs read it to protect themselves from TLC's approximate multi-worker breadth-first search ([D-001](#d-001-parallel-checking-gives-the-same-answer-at-any-worker-count)). For example, Examples `DieHard/MCDieHardest` declares:

```tla
ASSUME /\ TLCGet("config").mode = "bfs"
       /\ TLCGet("config").worker = 1
```

so that TLC is guaranteed to find a shortest counterexample. Run with `-workers 4`, TLC fails that assumption, and the spec refuses to run.

**What dive does.** dive's parallel search is, by construction, TLC's single-worker search executed in parallel ([D-001](#d-001-parallel-checking-gives-the-same-answer-at-any-worker-count), [deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md)). So by default dive reports the logical worker count, 1, at any `--workers` setting. Specs like `MCDieHardest` therefore run at full parallel speed and get exactly the guarantee they ask for. dive's own output still reports the physical thread count.

**Conformance.** Under `--compat=tlc` such a spec can behave differently at different worker counts, exactly as TLC does. The conformance harness therefore compares a spec that reads `TLCGet("config")`, run under `--compat=tlc` with more than one worker, with TLC's own result at the same worker count, on the verdict and the error class ([worker count and depth semantics design](decisions/0004-worker-count-and-depth-semantics.md), amendment of 2026-10-09). In the default profile the single-worker result applies at every worker count.

**Evidence.** Harness worker-invariance report (`MCDieHardest`), [evaluator architecture design](decisions/0014-evaluator-architecture.md), [deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md). `MCDieHardest` at 1, 3, 4 and 8 workers agrees with TLC in both profiles: under `--compat=tlc`, at 3, 4 and 8 workers both TLC and dive fail the assumption (`TLC_ASSUMPTION_FALSE`).

---

## D-012: Counterexamples are checked by an independent validator

| | |
|---|---|
| **Kind** | Same answer, with a stronger guarantee |
| **Default** | Every counterexample `dive check` reports is first replayed against the specification by a separate validator. `dive validate-trace` checks any trace on demand |
| **TLC behaviour** | Nothing to switch off: validation never changes a verdict. Under `--compat=tlc` the validator evaluates with TLC's semantics |
| **Status** | implemented: the validator (`dive-trace`), the library API (`dive_api::trace`), `dive validate-trace` and the conformance harness, which validates every `dive check` counterexample. `dive check` emits the document (`counterexample.trace`) but does not yet validate it itself before reporting (M5) |

**What TLC does.** TLC prints the counterexample its own search found, and nothing checks it independently. A bug in TLC's successor generation, or in how it rebuilds the trace, would print a trace that the specification does not allow. TLC's `-dumpTrace json` output cannot be read back reliably, even by TLC: a set and a sequence are both JSON arrays, and a string and a model value are both JSON strings. For example, for `x = {<<1, 2>>}` and `x = <<<<1, 2>>>>`, TLC writes the same `[[1, 2]]`.

**What dive does.** A separate validator checks the trace *as data*. It evaluates `Init` on the first state, and the next-state relation as a predicate on each pair of states, without ever generating successors: the code that searches is not even compiled into it. It also checks that the claimed invariant or property really fails at the end, that nothing failed earlier, and that every state the search must have explored respects the `CONSTRAINT`s. A counterexample that fails is an internal error, never a verdict.

```sh
dive validate-trace DieHard.tla --trace trace.json --claim invariant:NotSolved --compat=tlc
valid: the 7-state trace validates
```

`dive validate-trace` reads dive's own trace format, whose values are exact, and TLC's `-dumpTrace json`, which it reads by trying the interpretations of the ambiguous parts (see [trace validator independence design](decisions/0016-trace-validator-independence.md)). From Rust, `dive_api::trace::SpecModel` checks a program's own recorded states against a specification.

**Random values.** A trace whose values come from `RandomElement` or the `Randomization` operators is checked against the run's seed: give `dive validate-trace` the `--seed` (and `--fp`) that `dive check` reported. The validator draws the same values while it evaluates `Init` and each step, so a changed value is rejected. Without the seed, it refuses by name.

**Limits.** The validator confirms deadlocks when an independent finite-cover procedure proves absence of successors, and rejects claims when it finds a witness. Missing covers remain refused by name; only those cases use TLC's own ending state for harness confirmation. Pure concrete liveness words now validate when the trace supplies the complete, nonempty `properties` list. The validator checks the closing edge, all behavior temporal conjuncts, and that exactly the claimed temporal properties are false. Fairness and direct `ENABLED` now use independent finite covers; missing covers, effects, VIEW/symmetry closing evidence and unnamed legacy liveness words remain refused. Error traces are checked step by step, but re-raising their final error remains unsupported. Random values drawn outside `Init` and the next-state relation (in an invariant, say), under a `VIEW` or `SYMMETRY`, or in an order only the successor generator knows (one action's draws before another's, when the trace took the second) are refused by name too. See *Where dive is currently more limited than TLC*.

**Periodic-word evidence.** Ten pinned liveness traces validate, including recurring fair actions, intermittent enabledness (`LvWFIntermittent`) and a fair disabled-action stutter (`LvStutterEnd`). A non-stuttering occurrence on the repeated cycle proves both weak and strong fairness; occurrences only in the prefix, stutters and empty subscripts do not. Without that certificate, weak fairness needs a disabled cycle position and strong fairness needs all cycle positions disabled. These are checked independently; incomplete covers remain refused. `LvStutterActionConstraint` demonstrates that TLC adds implicit stuttering even under `ACTION_CONSTRAINT x' # x`: the pinned oracle reports a one-state stuttering violation at `x = FALSE` (exit 13, 3 generated / 2 distinct / depth 2). dive permits that implicit stutter while checking constraints on generated transitions. Closing-edge and property-claim mutations are rejected (`crates/dive-trace/tests/lasso_claims.rs`; [liveness architecture design](decisions/0018-liveness-architecture.md)). This validator does not yet enable liveness model checking.

**Evidence.** [trace validator independence design](decisions/0016-trace-validator-independence.md) and its amendment 1. `dive-conformance trace-controls` runs TLC's own counterexamples through the validator: 12 of TLC's 15 safety counterexamples in the Examples goldens validate, and 3 are refused by name (two on evaluator operators not implemented yet, one whose `ALIAS` hides the variables). Every deliberately broken copy of them (a state dropped, two states swapped, a value changed, the trace cut short, the wrong invariant claimed) is rejected.

---

## D-013: A deterministic short liveness counterexample

**Status:** partial, approved default. `--lasso=short` chooses a shortest prefix and a deterministic short cycle. It does not promise the globally shortest lasso. Under `--compat=tlc` dive also reports a short lasso that the independent validator accepts. TLC's exact choice is not a parity obligation ([liveness defaults and architecture design](decisions/0082-liveness-defaults-and-architecture.md), [rescue reset design](decisions/0086-rescue-reset.md)), so the proposed `--lasso=tlc` mode was dropped on 2026-10-12 and its emulation code removed; `--lasso tlc` is now a usage error. Under `--compat=tlc`, `dive_liveness::tlc_plan::classify` reproduces TLC's two liveness formula refusals (errors 2213 and 2214).

**Library progress.** The bounded final-only coordinator now handles pure
predicates and WF/SF via scoped enabledness and non-stuttering actions. Ten
pinned violations match TLC counts in both stores and yield independently valid
candidates; three success controls also match, including enabled successors
outside a constrained graph. Every false configured property is attributed
and independently checked; omitted or extra claims are rejected. Capture now
uses ordered parallel source waves with reported pool width and a separate
scratch admission bound. TLC-exact planning/selection, phase-L error
accounting and periodic capture remain pending. Ordinary
`dive check` checks temporal properties on Core.

The [Rust API](temporal-api.md) returns a
periodic violation only after both its in-memory word and lossless trace
validate against a separately bound model. Unsupported validation stays a
refusal. Safety stops retain their statistics and finite counterexamples.
This API always selects short lassos, including with TLC value semantics;
it does not claim TLC-exact selection or periodic/error timing.
Finite constant-domain quantifiers and tuple bindings are now supported, with
original configured names retained even when the binder splits roles. Five
additional pinned probes match verdict truth and counts in both profiles/stores.
The empty-domain probe uses TLC's finite-prefix exit 12; exact class/trace timing
and the oracle parser correction remain tracked in.


**Example and measurement.** `LvNoFair` and `LvTwoLoopsRev` in the liveness probe corpus have several violating loops. Changing only TLC's fingerprint polynomial changed the counterexample on 6 of 21 measured models; `MCDistributedReplicatedLog` gave five choices, of 4, 5 or 6 states and with different named properties. Those implementation-dependent choices make diagnosis harder. Every dive lasso will be independently checked, including fairness and its claimed properties. Evidence: [tlc liveness semantics design](decisions/0017-tlc-liveness-semantics.md) and [liveness architecture design](decisions/0018-liveness-architecture.md)–§6.

**Planning progress, 2026-10-10.** A separate bounded diagnostic planner now
reproduces TLC's negation, simplification, DNF and pre-tableau grouping order.
For example, `TpMultiple` combines weak fairness with three properties; pinned
TLC produces two order-of-solution groups and three possible-error models.
The new projection reproduces their predicate-bin order and index arrays,
while retaining the original property names for independent checking.
The twelve real-model controls retain eleven completed transformation/grouping
projections and one SANY rejection. TLC's separate configured/action name
`Live` and derived temporal name `Identity` are recorded field by field; dive
keeps the configured table separately and matches the derived role vector.
Future action attribution remains tracked separately. A further 84 supplied-formula controls include four named
shape refusals and TLC's integer-overflow bound on DNF expansion.

This supports the future TLC-compatible path; it does not change which
counterexample ordinary `dive check` returns. Strict integrated liveness,
strict execution, timing and selection remain refused. The existing
final/short path and its reported restrictions are unchanged. Evidence:
`conformance/probes/tlc-planning/README.md`.

[D-013](#d-013-a-deterministic-short-liveness-counterexample) diagnostic planning now also projects bounded TLC particle/tableau
numbering and promise identities. Actual pinned graph fields and
independent periodic/closing-witness controls cover this artifact; it never
reports a temporal verdict. Empty/nonpositive direct structures, actions,
unchecked group/tag/bin metadata, opaque/effectful scopes and resource limits
remain named refusals. Strict integrated TLC liveness, phase-L timing, SCC and
witness selection remain unsupported by this stage. Original configured/Action
claims and derived role names stay separate; no new default is adopted.

Eligible ordinary TLC-profile checking selects the existing bounded final/short
backend without extra policy flags. Its report states the actual final schedule;
this does not implement TLC's periodic trigger. Actual startup-bound safety roles
retain ordinary safety behavior. Real temporal properties use the supported pure
subset and independent periodic-word replay. Ordinary default-profile requests
use the deterministic completed-level schedule ([D-014](#d-014-periodic-liveness-checking-has-a-replayable-schedule)). Excluded options retain their
previous refusals. See [ordinary temporal checking](user/temporal.md#ordinary-checking-in-the-tlc-profile)
for exact eligibility, schema, counts and exit conventions. General TLC parity
remains incomplete; limits and external timeouts do not prove a model holds.

## D-014: Periodic liveness checking has a replayable schedule

Eligible ordinary dive-profile temporal checking uses deterministic completed
BFS-level boundaries. A check is due once the graph has at least 2^16 distinct
states and at least twice as many as at the previous check ([liveness architecture design](decisions/0018-liveness-architecture.md)), so
all prefix checks together cost about one final check, and smaller models are
checked once, at the end. Workers join and the level commits first. Exhausted frontiers
receive only the final check. Ordinary TLC-profile requests keep final checking;
Core refuses explicit `--lncheck final --lasso short` by name. The implementation does not
provide TLC's wall-clock controller or a `--lncheck default` timing emulation.

A prefix never proves `Holds`. A deciding periodic word must pass fresh
independent typed and serialized validation. Results retain actual partial
counts, depth and nonempty frontier with `capture_completed: false`; they never
borrow final counts. Reports name the actual schedule and growth/check/boundary/
exhaustion facts. Capture limits are cumulative; analysis bounds keep their
per-property/per-check scope, so repeated checks add work without a total-run
work/byte cap or speedup promise. A prefix check searches only the first
configured property, so a prefix violation names the same property that the
final check would name first; later properties are decided by the final check.
A violation found on a prefix stops with fewer states than TLC's
`-lncheck final` golden, which is a [D-014](#d-014-periodic-liveness-checking-has-a-replayable-schedule) count divergence. As with TLC's own
periodic checking, it can also preempt an invariant violation or evaluation
error that lies beyond the checked prefix and that a final-only run would
report instead. `--compat=tlc` restores final checking. See [the temporal guide](user/temporal.md).

## D-015: Symmetry functions must actually be permutations

**Status:** bounded checked/TLC groups, supported pure startup warnings and pure safety key reduction are implemented. Unverified startup exceptions, declaration shapes, lazy/deferred permutations, random seeding and TLC quotient liveness remain named refusals. Supported whole-run unreduced temporal checking follows [D-005](#d-005-symmetry-with-liveness-uses-the-full-state-space). A declaration containing `[a |-> b, b |-> b]` is not bijective. TLC accepts it and closes the set under composition, permitting an unsound reduction. The approved model default is to name and reject the offending function; `--compat=tlc` retains TLC's acceptance. The minimal missed-invariant control below supplies numerical evidence for that policy.

**Foundation update, 2026-10-09.** The bounded symmetry-closure kernel now
has a checked setting that rejects non-bijective maps, including collisions
with implicitly fixed model values. Its TLC setting preserves malformed-map
closure and exact insertion order. All 1,329 direct pinned-kernel inputs match;
valid cases match in both settings. Pure declaration preparation now preserves
configuration substitutions and pre-constant lookup; its four early errors
match normal pinned TLC transcripts. Unverified evaluation/normalization
startup errors refuse by name. For example, `Sym == 1 \div 0` is outer
GENERAL (1000), exit 255, with a nested 2179 message; `Sym == {1, TRUE}` wraps
its normalization error in GENERAL. The raw SyDivZero and SyMixedElements
captures preserve those contexts. These exceptions still refuse by name. The
subsequent pure safety increment below activates supported startup preparation
and key reduction. Evidence and reproduction:
`crates/dive-model/tests/fixtures/symmetry-group/README.md`.

**Safety update, 2026-10-10.** `dive check` now applies pure symmetry
reduction in both stores and at parallel worker widths. Its counterexamples
show concrete explored states and validate independently. The SyNonBiject
control below now runs: the default refuses the non-bijection before search;
`--compat=tlc` reproduces TLC's success and exact 3 / 1 / depth 1 counts.
Source-backed warning controls also match TLC's too-small/unused-set warnings.
That original safety increment did not enable symmetry with liveness or the opt-in audit. Supported whole-run unreduced temporal checking is now implemented separately under [D-005](#d-005-symmetry-with-liveness-uses-the-full-state-space); the audit is not implemented.

**Minimal measured missed violation.**

```tla
CONSTANT a, b
VARIABLE x
Init == x = a
Next == x' \in {a, b}
Bad == {[p \in {a, b} |-> a], [p \in {a, b} |-> p]}
Sym == Bad
Inv == x = a
```

Bind `a` and `b` to model values and check `Inv`. With `SYMMETRY Sym`, the
pinned TLC reports success: 3 generated, 1 distinct, depth 1. Without symmetry,
it finds `x = b` violating `Inv`: 3 generated, 2 distinct, depth 2, exit 12.
The collapsing map is not a bijection; it merges the violating state into the
initial state's key. The normal-CLI SyNonBiject/SyNonBijectNone transcripts
are in the symmetry-group fixture directory. Current dive default model checking
rejects this map before search; compat mode reproduces the reduced result.

## D-016: The declared symmetry can be audited

**Status:** not implemented; it would be opt-in. The `SymAsym` and `SymAsymLate` probes declare symmetry that the transition relation does not respect. TLC trusts the declaration, so the reduced search can hide behaviors. `--check-symmetry` will check initial states, successors, invariants, constraints and VIEW under the declared generators and report the first counterexample to the declaration. Ordinary runs, including `--compat=tlc`, omit this expensive audit. Evidence: [symmetry design](decisions/0019-symmetry.md) and the probe transcripts; implementation measurements are pending.

## D-017: Host access requires explicit grants

**Status:** partial. Ordinary Reference safety checking supports bounded
source-binding `IOEnv` and successful TXT/UTF-8 `Deserialize` reads, with explicit
recorded assumptions and independent offline source replay. Auto selects
Reference before effects. Runtime reads, writes, execution, checkpoint/recovery,
empty environment records and unproved TLC environment-name ordering refuse.
For example, Examples `TLC/TestGraphs` reads `IOEnv` at startup and
`ewd998/EWD998ChanTrace` reads it in its next-state relation; this slice does not
claim either full corpus model is qualified.

The interface is `--allow-io=env,files,exec`, with no categories granted by default
in either profile. Grants and actual input records appear in the optional
`external_inputs` report context. `validate-trace --inputs RUN.json` consumes the
same context without specification I/O. External values remain explicit
assumptions, not independently authenticated host truth. Native trace versions
are unchanged. Input bounds and exact supported forms are documented in
[host inputs](user/modules.md#host-inputs-and-explicit-grants) and [recorded startup host inputs design](decisions/0148-recorded-startup-host-inputs.md).
Three pinned controls cover admitted counts/words, TXT demand order, and the
otherwise verdict-changing IOEnv construction-order boundary.

---

## Under consideration

### Empty symmetry sets crash TLC

```tla
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == {}
```

With `INIT Init`, `NEXT Next` and `SYMMETRY Sym`, the pinned TLC exits 255
with GENERAL and an `IllegalArgumentException`, rather than explaining that
there are no permutations. SANY accepts the module. TLC's closure kernel
allocates a set with capacity `size - 1`; zero and one supplied function throw
when called directly. Normal binding often disables singleton cases first
with warning 2300, so these two stages must be tested separately.

The raw minimal SyEmpty capture and direct-kernel inputs are in
`crates/dive-model/tests/fixtures/symmetry-group/`. The new Rust kernel reports
`TooFewGenerators` instead of panicking. Model binding/reduction are still
refused; no new result-changing default for empty declarations is adopted.


These are recorded candidates. They are not decided, and nothing in dive behaves differently for them yet.

- **Which liveness counterexample is reported.** TLC's liveness counterexample depends on the order of its internal hash tables, so it depends on the fingerprint polynomial (`-fp`), which TLC picks at random unless you set it. On the Examples spec `MCDistributedReplicatedLog`, six polynomials gave five different counterexamples, of 4, 5 or 6 states, naming either `InSync` or `AllExtending and InSync` as violated. TLC's choice is also not the shortest available: for `[]<>(x = 0)` on a counter `0, 1, 2` with no fairness, TLC shows three states and then stuttering, where stuttering after two states already violates the property. dive's default (`--lasso=short`) reports a counterexample with the shortest possible prefix, the same one at any polynomial and any worker count; see [D-013](#d-013-a-deterministic-short-liveness-counterexample). The verdict and the state counts are the same either way. Evidence: the [tlc liveness semantics design](decisions/0017-tlc-liveness-semantics.md), the [liveness architecture design](decisions/0018-liveness-architecture.md) and the probes in `crates/dive-liveness/tests/fixtures/tlc-probes/`.
- **Liveness under a state constraint.** TLC checks liveness on the graph that `CONSTRAINT` leaves, while `ENABLED` (and so fairness) still sees the successors the constraint removed. A behaviour that leaves the model is not considered at all. For `x' = x + 1` under weak fairness with `CONSTRAINT x < 3`, TLC reports that `<>(x = 5)` holds, although no state inside the model has `x = 5` (probe `LvConstraint`). With `ACTION_CONSTRAINT` the cut graph can also produce a counterexample that the unconstrained spec does not have (probe `LvActionConstraint`). dive reproduces TLC exactly. A future warning could point out when a liveness verdict depends on a constraint.

- **SANY parser artefacts.** SANY rejects some inputs that look like valid TLA+, apparently because of how its JavaCC parser is implemented:
  - `{a, b \in S}`;
  - `{f(a) \in b : a \in S}`, which is rejected with `\in` but accepted with `∈`;
  - a keyword as a record field anywhere except the first position;
  - `3 <4> 5`, where `<4>` lexes as a proof step;
  - `F(+)`, where `(+)` lexes as `⊕`.

  dive matches SANY exactly. A future opt-in `--syntax=lenient` could accept these. The cases are in `crates/dive-syntax/tests/sany-cases/` and `docs/divergences.md`.
- **SANY semantic-analysis artefacts.** `dive analyze` accepts and rejects exactly what
  SANY does. A few of SANY's verdicts look like artefacts of its implementation:
  - **Valid specifications SANY rejects:**
    - `1 \in ℕ` with `EXTENDS Naturals` ("Unknown operator: `ℕ`");
    - an `INSTANCE` of the file's own inner module inside a `LET` or a proof step
      ("Multiply-defined module");
    - a label inside an unnamed `THEOREM ASSUME ... PROVE`.
  - **Suspicious specifications SANY accepts with only a warning or silently:**
    - a variable or constant declared twice;
    - two theorems or proof steps with the same name;
    - a definition silently replaced by an `INSTANCE` that also defines it;
    - `[]` applied to an action hidden inside a `LET`;
    - an action and a temporal formula mixed inside `IF`.

  A future opt-in lenient or strict mode could differ deliberately. The cases and SANY's
  verdicts are in `crates/dive-sema/tests/sany-cases/` and `docs/divergences.md`.
- **Line breaking of printed values.** TLC wraps long values at 80 columns by re-parsing their printed form. It gives up, and prints the whole value on one line, when the value contains a model value with `_` in its name, a negative interval such as `-1..1000`, or a string with an escaped quote (fixture `PrintWide` in `crates/dive-value/tests/fixtures/`). dive's printer matches TLC exactly. A future option could wrap these too. It would change only human-readable output, never a result.
- **`\div` on negative numbers in a spec that extends only `Naturals`.** TLC then uses `Naturals`' own Java division, which rounds toward zero when the quotient truncates to 0: `(0 - 1) \div 2` and `1 \div (0 - 2)` are `0`, although `(0 - 1) % 2` is `1` and `(0 - 7) \div 2` is `-4`. With `Integers` loaded, `(0 - 1) \div 2` is `-1`, as `a = b * (a \div b) + (a % b)` requires. `-2147483648 \div -1` also wraps under `Naturals` instead of overflowing. With TLC's 32-bit integers (`--int=tlc`, part of `--compat=tlc`) dive matches TLC exactly; with dive's default arbitrary-precision integers, `\div` is floored division under both modules (fixture `RvNatDiv` in `crates/dive-value/tests/fixtures/`). A spec that relies on negative numbers should extend `Integers`.
- **"States generated" counts that depend on how a set is represented.** TLC counts every successor its enumerator produces, duplicates included ([deterministic parallel search and state storage design](decisions/0015-deterministic-parallel-search-and-state-storage.md)). For `x' \in S` it walks `S` without normalizing it when `S` is a lazily represented set, so an element in both halves of a lazy `\cup` is generated twice:

  ```tla
  VARIABLE r
  Init == r = [a |-> 0]
  Next == r' \in [a : {1}] \cup [a : {1, 2}]
  ```

  The pinned TLC reports 10 states generated and 3 distinct: each of the 3 states generates `[a |-> 1]` twice and `[a |-> 2]` once. Writing the same set as `[a : {1, 2}]`, or as a set enumeration, gives 7. Worse, the count also depends on history: once TLC has compared, sized or fingerprinted the same set object, it caches a normalized copy and enumerates that instead. dive matches TLC wherever the two enumerations agree, and refuses the others (see *Where dive is currently more limited than TLC*). A future option could count distinct successors only. Fixture `NxInLazyCup` in `crates/dive-eval/tests/fixtures/next/`.
- **`-nowarning` changes the number of states generated.** Once an action has assigned every variable, TLC evaluates the rest of it as a plain boolean instead of enumerating it, but only while warnings are on (`TLCGlobals.warn`, the default). With `-nowarning`, the same action enumerates the rest, so `x' = 1 /\ (TRUE \/ TRUE)` generates two successors instead of one. On fixture `NxFastPath` the pinned TLC reports 7 states generated by default and 37 with `-nowarning`, for the same 4 distinct states. Initial states always take the shortcut. dive reproduces both (`Options::warnings`); a warning switch that changes a count is a candidate for decoupling.
- **The order in which `\A` explores its bindings in an action.** For `\A i \in {1, 2, 3} : A(i)` in a next-state relation, TLC enumerates `A(1)` first and then `A(3)` before `A(2)`: it conses the remaining bindings onto its continuation list, last first (fixture `NxForallOrder`). This decides which successor comes first when the `A(i)` assign variables. dive reproduces it.
- **Java overrides that disagree with the CommunityModules' TLA+ definitions.** TLC runs the Java override, never the definition, and dive does the same. Measured on the pinned TLC and CommunityModules `202610040242` (fixtures in `crates/dive-eval/tests/fixtures/specs/`):
  - `Bitwise!shiftR(n, pos)` is Java's `n >>> pos`, which uses only the low five bits of `pos`: `shiftR(16, 32)` is `16` and `shiftR(16, 33)` is `8`, where the definition gives `0` (fixture `CmBitwise`). On negative numbers, outside the definitions' domain, `x ^^ y`, `Not` and `shiftR` follow Java's 32-bit two's complement (`(0 - 5) ^^ 3` is `-8`, `shiftR(-16, 2)` is `1073741820`).
  - `SequencesExt!LongestCommonPrefix` on strings assumes the first string of the normalized set is the shortest. TLC orders strings by when it first met them, not by length, so `LongestCommonPrefix({"q7q", "q7"})` fails with `String index out of range: 2` instead of returning `"q7"` (fixture `ErCmLcpStringOrder`).
  - `BagsExt!FoldBag` folds in the order TLC happens to hold the bag in, which for a bag built with `:>` and `@@` is the order it was written, not a normalized one. dive keeps functions normalized and refuses `FoldBag` on bags of two or more elements rather than guess.
  - `SetToBag` of a non-set reports its error as `BagToSet`'s (fixture `ErCmSetToBagNonSet`).

  dive reproduces all of these under `--compat=tlc` and in the default profile. A future option could follow the definitions instead.
- **Errors inside `TLCGet`, `TLCEval` and other evaluating overrides are wrapped.** An error raised while TLC evaluates `TLCGet`, `TLCEval`, `ReplaceFirstSubSeq` or `ReplaceAllSubSeqs` (including in their arguments, and in the TLA+ definition TLC falls back to) is reported as `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` with the Java method's signature, around the actual error: `TLCGet(7)` with no register 7 is reported that way rather than as `TLCGet(7) was undefined.` (fixtures `ErCmTlcGetUndefined`, `ErCmTlcGetArgError`, `ErCmTlcEvalError`, `ErCmReplaceFallbackError`). dive reproduces the wrapping. A future option could report the inner error alone.
- **String length in UTF-16 units.** TLC's `Len`, `SubSeq` and `Tail` on strings count UTF-16 code units, not characters (tlaplus#1076). See `docs/internal/survey/oracle-history.md`.
- **Quirks of TLC's `.cfg` reader.** TLC's model-configuration reader accepts some files that it will later fail on, and it reads some values in ways you might not expect. Each case below was measured on the pinned TLC:
  - `CONSTANT N = -1` is not a negative number. TLC reads `-` as a model value, and then fails on the next line with "expecting = or <-" (`CFG_EXPECTED_SYMBOL`, exit 151). A cfg file has no way to write a negative integer.
  - `N = 2147483648`, or `N = \h1F`, stops TLC with an unexpected `NumberFormatException` (`GENERAL`, exit 255) instead of a configuration error.
  - `INVARIANT Inv1, Inv2` is accepted, with the comma read as an invariant name. TLC then fails with "The invariant , … is not defined".
  - `INIT NEXT` (a forgotten name) makes `NEXT` the initial predicate.
  - `{1,}}` is read as a set containing the model value `}`.
  - String values keep their escapes verbatim, so `"a\nb"` in a cfg is four characters, unlike the same literal in a module.

  dive matches TLC exactly, and adds warnings (`DIVE-C0100` to `DIVE-C0102`) where TLC accepts input it can never resolve. A future opt-in `--cfg=strict` could reject these files. The cases and TLC's recorded runs are in `crates/dive-config/tests/fixtures/` and `crates/dive-config/acceptance/README.md`. Earlier TLC versions did worse: a lexical error silently truncated the file, so TLC could report success while ignoring an `INVARIANT`. The pinned TLC rejects such files, and so does dive.

## Where dive is currently more limited than TLC

dive refuses rather than approximates (soundness contract rule 3). These are honest refusals, and we track them here so that nobody mistakes them for features.

- **Extremely deep nesting** (`DIVE-S0040`). dive refuses expressions nested more than 256 levels deep, and syntax trees more than 2,048 levels high (for example a chain of thousands of `+`). SANY accepts these. No module in the corpus comes close. Removing the limit is a tracked follow-up.
- **Very deeply nested values** (`DIVE-V0030`). dive refuses to print, to convert to JSON, or to compare, test for equality or test for membership with TLC's rules, a value nested more than 500 levels deep, such as `<<<<<<...>>>>>>`. Its own equality, ordering, hashing and fingerprints handle any depth, and so does dropping a value. TLC works recursively up to the limit of its JVM stack (`-Xss`; the harness uses 8 MiB). The limit keeps dive safe on a 2 MiB worker stack even in debug builds. No spec in the corpus is known to come close.
- **Very large sets.** dive enumerates at most 2^31−1 elements of a set (`DIVE-V0020`). TLC's own limit is the same, from Java's `int` sizes. Larger sets that TLC handles without enumerating, such as `0..2147483646` in a state, are handled the same way in dive.
- **Very deeply nested cfg sets** (`DIVE-C0030`). dive refuses a constant value in a `.cfg` file whose sets are nested more than 1,024 deep. TLC reads them recursively, and stops with a stack overflow at a depth that depends on its `-Xss` setting. No cfg file in the corpus comes close.
- **PlusCal re-translation during analysis.** When a module contains both a PlusCal
  algorithm and its translation markers, SANY re-translates the algorithm to check it
  against the translation.
  - Two outcomes of that check reject the module: an unterminated string in the algorithm,
    and `END TRANSLATION` without `BEGIN TRANSLATION`. `dive analyze` matches both.
  - SANY would also reject a module whose re-translation throws an unexpected exception.
    `dive analyze` does not re-translate (that is `dive-pluscal`'s job), so it could accept
    such a module. No corpus module or mutant has shown this.
- **Evaluation refusals** (`dive-eval`). The evaluator refuses, naming the feature, rather than guess:
  - a Java-overridden operator dive does not implement yet: `JavaTime`, Json file operations and representation-dependent Array/Object entry points, most of `TLCExt` (`AssertError`, `TLCCache`, `TLCFP`, ...), `Randomization!RandomSubsetSet`, and the CommunityModules operators `FiniteSetsExt!kSubset`, `Combinatorics`, `Statistics!ChiSquare`, `VectorClocks!CausalOrder`, `GraphViz`, SVG layout and element serialization. `SVG!PointOnLine` has the bounded literal stored-record path described in [module loading](user/modules.md); other SVG argument representations remain refused. TLC never uses these operators' TLA+ definitions, so dive does not either;
  - cases of implemented overrides whose TLC result depends on something dive does not track: `BagsExt!FoldBag` on a bag of two or more elements, `Bags!BagOfAll` with an operator that has side effects, and the error a few `Bags`, `Functions` and `IsInjective`/`AntiFunction` cases raise when values TLC cannot compare meet in an order dive does not know;
  - `TLCGet` keys that read TLC's internal description of the run (`"spec"`, `"action"`, `"revision"`) or its JVM (`"-D..."`), the live counters (`"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"stats"`, `"duration"`) while states are being checked (they are provided while assumptions are checked), and `TLCSet("pause", TRUE)`;
  - the random operators when the evaluator runs without a model checker providing a random stream (for example `ASSUME`s evaluated on their own);
  - a function that TLC keeps unevaluated (`[x \in S |-> e]` over an infinite domain, or a recursive function) stored inside another value, when computing it could fail or has side effects;
  - enumerating `S \X T` or `[a : S]` with an infinite component, a set too large to count when it is enumerated lazily, and `IsFiniteSet` of a lazily evaluated set;
  - applying a function of 32 or more arguments to a value TLC cannot compare with its domain.

  An evaluation nested deeper than the depth limit is a resource limit, as TLC's stack overflow is.
- **Successor-generation refusals** (`dive-eval`'s successor generator). It refuses, naming the case, rather than guess the successors TLC would generate:
  - `x' \in S` where `S` is a lazily represented set (for example `[a : S] \cup [a : T]`, or a set filter or `UNION` built from one) whose TLC enumeration lists an element twice, or in an order other than the normalized one. TLC's answer depends on whether it has already compared, sized or fingerprinted that same set object (see *Under consideration*);
  - applying, as an action, a lazily evaluated function that has pending `EXCEPT`s or an argument outside its domain;
  - a built-in operator passed as an operator argument and applied as an action (`Op(\/)`);
  - `x' \subseteq S` where `S` is not a set.
- **`dive check` refusals.** Core checks invariants, deadlock, temporal properties with fairness, and `SYMMETRY`. With a genuine temporal property, `SYMMETRY` is checked on the unreduced state space in the default profile ([D-005](#d-005-symmetry-with-liveness-uses-the-full-state-space)). TLC's symmetry-quotient liveness (`--symmetry-liveness=tlc`) is refused, and it is the `--compat=tlc` default, so under `--compat=tlc` a temporal property with `SYMMETRY` is refused unless `--symmetry-liveness=unreduced` is passed. Core refuses, naming the feature, rather than give an answer it cannot vouch for:
  - `POSTCONDITION`;
  - `--chaos`, `--action-composition=tlc`, and action composition (`\cdot`) with conjuncts after it or variables assigned before it in the default mode ([D-022](#d-022-action-composition-works-by-default-with-its-tla-meaning));
  - with temporal properties: the explicit bounded final mode (`--lncheck final --lasso short`), and `--max-states` or `--max-depth`;
  - anything the binding stage or the evaluator refuses (for example `IOUtils`, Json file operations and the CommunityModules operators listed above);
  - evaluation errors whose exact TLC error class depends on value-source metadata dive does not retain. dive reports their messages but marks the deciding class as unverified, and the conformance harness refuses agreement.

  The features Core refuses since the Reference engine was retired, with the tests that will cover them once implemented, are listed in . Also: `ALIAS` is not yet applied to printed counterexamples or simulation traces (dive prints the states themselves, and says so). `dive simulate` runs on Core with one worker; parallel simulation workers are refused.
- **Trace validation** (`dive-trace`, `dive validate-trace`). The validator refuses, naming the reason, rather than trust the checker:
  - deadlock endings whose successor absence cannot be decided by the independent finite-cover check. A concrete successor refutes the ending, complete finite-cover exhaustion validates it, and missing covers remain unsupported;
  - liveness claims that lack a complete named-property list or need fairness or `ENABLED` facts the shared evaluator cannot decide on the trace's states, effects, or VIEW/symmetry closing evidence. Pure concrete periodic words are checked through their closing edge and complete property set. `ENABLED` is decided by the core evaluator that `dive-trace` shares ([validator enabled via core evaluator design](decisions/0103-validator-enabled-via-core-evaluator.md));
  - the final claim of an assertion failure or evaluation error (re-raising it needs the successor generator). The steps are checked;
  - a TLC `-dumpTrace json` document written under an `ALIAS` whose records do not contain the variables, and one too ambiguous to search exhaustively when no interpretation tried validates;
  - traces whose states leave variables unassigned (partial implementation traces), which need a search;
  - public Rust typed states with names whose text and token do not match the model's interner (`DIVE-V0041`), opaque deferred values whose names cannot be inspected without running caller code (`DIVE-V0042`), or sets, records and functions sorted in the other profile's value order (`DIVE-V0043`). Build values with the model's interner and value order, or decode a trace document against that model;
  - whatever the evaluator refuses, and reads of the live counters, which depend on the search rather than on the trace. `TLCGet("level")` is the level of the state being checked (a primed `TLCGet("level")`, which TLC evaluates at the successor's level, refuses validation by name), and `TLCGet("config")` is provided when the run's `-seed` and `-fp` are known (the conformance harness passes the goldens'). Integer/named register reads and writes, including `TLCGet("all")` and `TLCGet("all:named")`, are refused during evaluation because their values can depend on checker evaluations outside the trace. Untaken calls remain supported and validation leaves caller registers unchanged;
  - every random draw made while checking the trace, even with the run's `-seed` and `-fp`: only the successor generator determines its stream position, which independent replay does not assume ([trace validator independence design](decisions/0016-trace-validator-independence.md), amendment 3).

  A step label that names an operator with parameters is noted and not checked.
- **Simulation with a liveness `PROPERTY`** (not in this release). `dive simulate` refuses a configuration that names a temporal `PROPERTY` (`simulation temporal properties: <names>`), including the fairness conjunct of a specification named as a `PROPERTY`, rather than checking it on the simulated traces as TLC does; dive does not yet check liveness on simulated traces with independently validated words. Use `dive check` for the property, or remove it from the simulation configuration. Examples: Examples `ewd687a/EWD687a_anim` (property `DT2`) and `ewd998/SmokeEWD998` (the `WF_vars(DetectTermination)` conjunct of `PROPERTY TDSpec`).
- **`VIEW` combined with liveness** (not in this release). A model that declares a `VIEW` and checks a temporal property is refused by name (`graph capture with VIEW requires quotient edge and closing-state evidence`). In these models the `VIEW` is what makes the state space finite, so the search runs on the quotient graph. A liveness verdict on the quotient is sound only with an argument that the `VIEW` preserves the property, plus concrete reconstruction of the counterexample lasso; neither exists. Safety checking with a `VIEW` is unaffected. Examples: `FiniteMonotonic/MCDistributedReplicatedLog`, `ewd998/EWD998ChanID`.
- **TLC's exact liveness counterexample.** TLC picks its lasso through hash-table and SCC visitation order, which depends on the fingerprint polynomial. dive reports a deterministic short lasso that the independent validator accepts, with the same verdict and required state-space facts. Each report names the complete property set independently proved false on its own word; different valid lassos can name different sets ([word relative liveness attribution design](decisions/0126-word-relative-liveness-attribution.md)). `--lasso tlc` is refused as a usage error ([D-013](#d-013-a-deterministic-short-liveness-counterexample)).
- **Configurations embedded in a `.tla` file.** TLC can read a configuration from a `---- CONFIG Name ----` section of a `.tla` file. `dive parse --config X.tla` refuses this as `unsupported`.
- **Binding refusals** (`dive-model`). Binding a specification to its configuration refuses, naming the feature:
  - a constant operator given values in the `.cfg` (`F(1) = 2`), which TLC holds as a table of values (`OpRcdValue`);
  - unknown or changed Java classes next to a module: TLC loads them as module
    overrides, while dive cannot run arbitrary Java. The exact pinned Hanoi
    `Bits.class` has a native signed-AND port under the native one-worker-reference
    contract ([D-024](#d-024-pinned-local-bits-checking-requires-one-worker));
  - TLC's undocumented `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE` sections. A `_PERIODIC` entry is bound first, with TLC's errors for an undefined name (2229) or an operator with parameters (2228), and then refused: dive does not run the hook on TLC's wall-clock timer ([D-014](#d-014-periodic-liveness-checking-has-a-replayable-schedule));
  - a definition TLC evaluates at start-up that dive's evaluator refuses, when the result decides how the model is bound (it is named in the configuration, it is a conjunct of the behavior specification, or it prints or draws), or when the refused construct splits the next-state relation;
  - the few messages whose text depends on Java objects dive does not have (a Java override used as a configuration name, a state variable used as a conjunct of `SPECIFICATION`).

---

Independent replay refuses every random draw encountered while checking a
supplied trace ([trace validator independence design](decisions/0016-trace-validator-independence.md) amendment 3). A matching seed does not establish which
search branches consumed draws before those states. Seed and fingerprint
settings still describe run-dependent model binding; they do not authorize
random replay or a successful validation result.

## Temporal checking limitations

An expanded final/short study selects 53 pinned Examples
models: 26 match final truth/counts, 22 refuse features or resource limits, and
five time out without a verdict. Ten refusals involve finite lazy binding
domains. There are no observed disagreements or pending validations, but the
timeouts are findings and the study exits nonzero. This is explicit final/short
mixed-profile evidence; ordinary strict temporal checking remains refused.
TLC-exact temporal classes, phase-L timing and lasso selection, wider scoped
language support, symmetry/VIEW evidence and capture scaling remain open.


**[D-013](#d-013-a-deterministic-short-liveness-counterexample) finite lazy-domain update:** temporal formulas can now quantify over
bounded function sets, record sets, Cartesian products and subsets. For example:

```tla
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Funcs == [{1, 2} -> BOOLEAN]
Live == \A f \in Funcs : []<>(x = f[1])
```

The pinned oracle holds `Live` with 3 generated states, 2 distinct states and
depth 2. Replacing `Next` with `UNCHANGED x` gives a liveness violation (exit 13),
with 2 generated states, 1 distinct state and depth 1. The same paired controls
exist for record sets, Cartesian products and subsets. All eight match dive's
explicit final/short path in both profiles and stores, at requested widths
1/4/8; actual graph capture is still one worker. Every violation validates
independently, and alternating/stuttering word controls distinguish true from
false properties.

The binding limit bounds domain entries, intermediate materialization work
(component entries and generated value slots), and Cartesian bindings separately.
No resource default was increased. Preflight uses raw cardinality upper bounds
without changing TLC's normalization state or running deferred predicates.
Infinite/opaque deferred domains and oversized/work-heavy constructors still fail by
name; a small cardinality alone does not justify allocating a huge function.
Both profiles use this support. Strict TLC phase-L scheduling and exact lasso
selection remain pending; this is not strict temporal conformance.

Evidence: `crates/dive-api/tests/fixtures/lazy-domains/` contains the eight
minimal specs and clean pinned oracle captures; `tests/lazy_domains.rs` checks
truth, counts and independent validation. `dive-eval` binder tests verify order
against ordinary enumeration, empty-domain functions, comparison-error order,
raw normalization state and giant-domain preallocation refusals.


**[D-013](#d-013-a-deterministic-short-liveness-counterexample) captured-filter update:** final/short temporal formulas can quantify
over closed pure filters of bounded function, record, product or subset sets,
and map over those filtered bindings. For example:

```tla
Domain == {f \in [1..2 -> BOOLEAN] : f[1] = FALSE}
Live == \A f \in Domain : []<>(x = f[1])
```

With the toggling `Spec` above, pinned TLC holds with 3 generated states, 2
distinct states and depth 2. With `Init == x = TRUE` and `Next == UNCHANGED x`,
it reports a liveness violation (exit 13), with 2 generated states, 1 distinct
state and depth 1. Six clean paired filter, map and state-dependent-domain
controls match final truth/counts in both profiles and stores at requested
widths 1/4/8; final graph capture remains one worker. Fresh independent
property evaluators accept an alternating word and reject a stuttering mutant.
Those word controls test property truth; complete violating reports separately
validate Init, Next and fairness against the spec.

A filter captures the state in which its value was created. Re-evaluating the
filter expression at a new state creates a new capture; primed predicates use
the successor state. These semantics are preserved through finite preflight and
periodic closing edges. The input's structural upper bound is charged before
predicate evaluation, even for an already materialized filter or one that would
select nothing. No resource defaults or effect permissions changed.

This support is shared by both profiles on the explicit final/short path.
Effectful/unresolved predicates, infinite/opaque inputs, non-filter deferred set
operations and local recursive-function scope analysis still refuse by name.
Strict TLC phase-L timing, exact lasso choice and full domain/scoping coverage
remain pending. Evidence is in `conformance/probes/finite-deferred-audit/`,
`crates/dive-api/tests/deferred_domains.rs` and the bounded evaluator controls.
The prior ten-model report is unchanged: passing these minimal guards is not
credit for its nine refusals/bounds and one timeout.


Captured operators passed through the Rust scope API remain an explicit limit:
static IR call-site effects do not prove them pure. Bounded captured filters
refuse such contexts before invoking the predicate, including pure supplied
operators until a scoped proof exists. Captured thunks/functions also require
closed effects. The native `rust-operator-capture-before.*` repro in the same
probe directory records the avoided PrintT purity bypass; it is not a TLC
behaviour difference or checking result.

Unknown or missing capture metadata also refuses before predicate execution.
A Rust caller can construct an unchecked expression ID; the captured-filter
proof rejects it without a panic, rather than treating the absence of metadata
as proof of purity. General caller-created IR validation is a separate API gap.

**[D-013](#d-013-a-deterministic-short-liveness-counterexample) retained temporal argument update:** pure expression arguments such as
`Identity([]<>x)` now keep their caller context, including a captured `LET`
value or a nested substituted module body. The pinned direct/local/body pairs
hold or violate with 3 generated states, 2 distinct states and depth 2. On the
explicit final/short path both profiles reproduce truth/counts; every returned
violation is checked independently, including Init, Next, fairness and its
closing edge. This is not exact TLC lasso/tableau or phase-L conformance.

An exported INSTANCE parameter is still a limit. The minimal paired examples
in `conformance/probes/temporal-forwarding/specs/TfInstanceHolds/` and
`TfInstanceFails/` pass SANY but TLC reports 2213/exit 255 at the inner formal:

```tla
\* Inside the instantiated Leaf:
Identity(p) == p
Claim(p) == Identity(p)
\* At the root, after two INSTANCE substitutions:
P == I!Claim([]<>x)
```

dive's temporal planning and independent periodic validation refuse temporal
arguments inside retained INSTANCE contexts. The accepted body-forwarding
example uses a zero-arity `Claim == []<>v` in Leaf and `Identity(I!Claim)` at the
root. No broader acceptance option or new default is adopted from this finding.
Opaque captured operators, recursive local-function domain proofs and more
complex ENABLED scope covers remain unsupported. The existing recursive paired
probes are retained; their self-binding and branch-sensitive finite preflight
must be proved before admission.


Unchecked Rust expression references and retained lazy expression IDs now
produce typed refusals at the planner/periodic endpoints, including missing
scoped definitions. Missing references have no invented source span. The raw
native `native-unchecked-context-before.*` repro in the probe directory records
the prior panics; it is not an oracle result. Constructing an evaluator with an
invalid redirect target can still panic before these endpoints run.
General caller-created IR/context validation remains a separate partial API
obligation; this increment does not redesign the evaluator constructor.

### Runtime names in TLC order

A model such as `s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)`
observes the order in which runtime strings are created. Speculative evaluation
can change that order even with one requested worker. TLC additionally creates
its configuration strings before assumptions: with `x \in 0..15` and the
invariant in `crates/dive-api/tests/fixtures/checker/runtime-names/DynamicStringOrder.tla`,
seed 7 stops at 23 generated/distinct states; seeds 0 and 14 stop at 30.
The raw pinned captures preserve this evidence.

In TLC value or exploration order, reachable string constructors now force
inline checking, with the reason and one actual worker reported. The default
canonical order remains parallel. This is a temporary admission restriction,
not a different answer: an ordered parallel runtime interner is still needed
for those TLC-mode models to scale. The conservative analysis includes tuple
uses of operators that can also construct strings, record-name constructors,
and wrappers or native overrides. Explicit final/short graph capture admits pure runtime-name models through the
same reported inline fallback. Genuine live-counter, random and I/O effects
remain refused. Strict temporal timing and lasso selection remain unsupported.

### Independent runtime-name trace validation

TLC's encounter order can depend on strings created while exploring states
outside a counterexample. Reading the counterexample itself creates string
tokens, so that order cannot be inferred from a witness prefix.

In `crates/dive-trace/tests/fixtures/runtime-names/ReplayNames.tla`, the clean
pinned oracle with seed 0, fingerprint 0 and one worker completes safely with
3 generated states, 2 distinct states and depth 2. The forged word
`(x=6, s="unset") -> (x=6, s="7")` claims an invariant violation. The previous
validator accepted it because loading `"7"` before constructing `"6"` changed
what `CHOOSE` selected. The raw false positive is retained with the safe oracle
capture, source hashes and reproduction flags.

TLC-order trace validation follows one rule. Names interned when dive binds
the model (SANY's encounter order, constants and configuration, and the
checker's own start-up names) have the same order as in TLC. The validator
validates as usual. If decoding the trace or evaluating any check interned a
name after binding, it reports **unsupported: runtime string encounter order**,
whether the trace would otherwise be valid or invalid. A seed and fingerprint
do not supply missing namespace history. The rule applies to dive's documents,
TLC's JSON documents, the Rust API, `ENABLED` decisions and lasso formulas.

On `ReplayNames`, both the forged `"7"` word and the actual `"6"` behavior are
refused under `--compat=tlc`: each creates a string the specification never
mentions. Canonical default validation accepts the actual behavior and
rejects the forged step. Literal strings remain supported, and so do models
whose name-capable operators (`Tail`, `\o`, `SubSeq`, `ToString`) only build
values from start-up names on the trace's own steps: for example
`MCCheckpointCoordinationFailure`, whose `SubSeq` and `\o` apply only to
sequences of model values.

This is dive's own limitation, not a new language default. A validator that
shares the checker's model sees the checker's runtime names and refuses; the
harness validates on a freshly bound model.

Finite safety simulation also refuses TLC runtime-name models before sampling
until its simulator startup and independent namespace provenance are proved.
Default canonical sampling remains supported and independently validated. This
is an explicit limitation, not a claimed mode-specific match.
The same restriction applies to directly supplied lasso formulas and independent
`ENABLED` predicates, even if they are not configured as checked properties.
The configured model flag alone cannot describe an arbitrary Rust caller's
predicate. These APIs preflight the supplied context before computing a result.
Stable checker configuration strings remain supported: a separate forged-prefix
control verifies that the seed is interned before the fingerprint, independently
of the order in which a trace mentions them.

### Bounded parallel temporal capture

**[D-013](#d-013-a-deterministic-short-liveness-counterexample) parallel-capture update:** explicit final/short checking now honors the
requested pool width for supported pure models. It retains all accepted edges,
including duplicates and seen states, in the same order as the inline engine;
validated short-word reports are identical at 1/4/8 workers and chaos schedules.
Fixed waves expand no more than eight source states concurrently. A separately
named library scratch bound admits at most 65,536 successors of one source by
default; raising it is explicit. Hitting it returns a resource refusal with
ordered partial evidence, never a completed graph. This is a conservative
capture limit relative to TLC, not an enhancement to TLA+ semantics. Existing
logical graph limits are unchanged. See [the bound and controls](temporal-api.md#bounded-parallel-graph-retention).
VIEW, symmetry, randomness and stateful effects remain graph refusals. This
does not prove exact TLC lasso selection, periodic scheduling or phase-L error
precedence. Ordinary strict temporal checking still refuses.

## D-018: A DFID zero exit may mean only a depth-bounded check

| | |
|---|---|
| **Kind** | Reporting/exit disposition; unchanged specification mathematics |
| **Default** | Explicit incomplete termination at an exhausted DFID bound |
| **TLC behaviour** | `--compat=tlc` retains TLC's measured exit status, with the incomplete scope reported |
| **Status** | Native pure safety slice implemented; full mode and output acceptance are not |

```tla
VARIABLE x
Init == x = 0
Next == x' = x + 1
```

This model has infinitely many reachable states. TLC's `-dfid 2` exits zero
after 2 states generated/2 distinct, and `-dfid 4` exits zero after 7/4.
Neither emits the completed-check message 2193. The help calls the argument
an initial depth, but it is a maximum level in `DFIDModelChecker`'s loop.
`-dfid 0` and `-dfid 1` run only initialization and exit 255.

Dive's native DFID exposes the reached level, partial counts and
`termination: depth-bound`, with `exhaustive: false` and `model_proved: false`.
A zero compatibility exit will not become a proof claim. A complete finite
check will carry a separate visitation-completion certificate. Clean primary
transcripts are under `crates/dive-api/tests/fixtures/checker/dfid/`; no RNG or trace
instrumentation changes the oracle in these controls.

TLC rejects `-dfid` with more than one worker, or with temporal properties to
check, as a general error and exits 255. `dive dfid` refuses both by name,
`DFID multiple workers` and `DFID temporal properties and fairness`, with the
refusal exit 3, under `--compat=tlc` as in the default profile.

**[D-002](#d-002-randomness-is-always-replayable) replay extension:** even with seed 7, aril 3 and fingerprint index 0,
`DfidBranch` produces both `0,1,3` and `0,2,4` counterexamples. Each
`DFIDWorker` seeds a new Java generator from its own fresh `nextLong()`;
the supplied CLI seed does not seed it. Native DFID reports every
worker seed and makes default runs replayable. An explicit replay override in
TLC mode will be labelled as an override. The pure safety implementation is partial; fresh-seed distribution and raw display
are not certified as complete TLC parity.

## D-019: DFID can truncate a counterexample after switching initial roots

```tla
EXTENDS Naturals
VARIABLE x
Init == x \in {0, 1}
Next == IF x = 0 THEN x' = 0 ELSE x' = 2
Inv == x < 2
```

**What TLC does.** The clean pinned `DfidRootsMixed` runs with `-dfid 6`,
`-workers 1 -fp 0 -seed 7 -aril 3` report invariant code 2110 and exit 12.
When root 1 is explored first, the printed witness is `1,2`, with 3 generated
and 3 distinct states. When harmless root 0 is explored first, TLC can instead
print only `1`, with 4 generated and 3 distinct states. State 1 satisfies
`Inv`, so the truncated word is insufficient evidence for the reported violation.

**Why.** `DFIDWorker` checks the next root before resetting its traversal depth.
Its invariant printer therefore walks the previous root's stack depth. This is
a concrete witness bug, separate from DFID's unseeded worker choices ([D-002](#d-002-randomness-is-always-replayable)).

**Dive.** Native DFID keeps the actual concrete path, returns `1,2`, and accepts
the violation only after independent validation. A printed-witness restoration
mode is explicitly unsupported in this slice; `--compat=tlc` retains the
verified search/counting contract but does not label a malformed word valid.
Reports state this witness policy. No TLC-matching trace claim is made here.

**Status and evidence.** Source and primary raw observations are in
`crates/dive-api/tests/fixtures/checker/dfid/`; `oracle-controls.json` records the clean
selection. Separately labelled seeded-worker reference controls reproduce both
root choices. This correction follows the soundness-first contract; a broken
witness never certifies a violation.

## PlusCal limitations

Native `dive-pluscal` now parses current C/P source into typed, source-mapped
ASTs. This is the first translation stage, not a translated specification:
a separate bounded sequential path now normalizes explicit labels/control flow
and emits validated enclosing modules. Macro expansion, processes/procedures,
branch-internal labelled expansion, unproved name disambiguation, multiline/binding
expression substitution and historical translator options/checksums remain named
refusals. Bounded sequential automatic labels, default model-value initialization
and `L:+` / `L:-` are now matched separately; see
[the normalization contract](pluscal-translation.md). See
[the translation contract](pluscal-translation.md).

The [single-process contract](pluscal-translation.md) now admits unfair literal-ID
and finite-membership process bodies. Locals are scalar for a constant-ID process
and indexed by `self` for a process set. Missing process labels always require
`--label`, unlike the pinned sequential default. Multiple blocks, process/label
fairness and unproved renaming remain explicit refusals. The minimal retained
`process P = 1 variables x=self` example translates with exit 0 but leaves `self`
unbound; SANY rejects that output. Native refuses this path in both profiles and
gives it no artifact credit, rather than inventing a replacement binding.

The pinned grammar scanner accepts `(1]`, `[1)`, `<<1}` and `[1, end]` because
it counts delimiter nesting rather than parsing TLA expressions. Native source
ASTs preserve those slices and make no claim of expression validity. Later
translation must validate them. Source bytes, grouping, macro calls, fairness
and every ordered statement are retained for that later stage.

[The source grammar study](pluscal-parser.md) accounts for 222 inputs/markers:
200 complete field-level AST matches, 13 grammar rejection matches, four
macro-expansion stage differences, one reference timeout and four non-standalone
documentation markers. All remain visible; F-034 is partial and uncertified.


### Translator success does not ensure a valid TLA module

```tla
---- MODULE TranslatorName ----
(* --algorithm A
variable Next = 0;
begin L: Next := 1; E: skip; end algorithm; *)
====
```

The pinned translator exits **0** and prints successful translation. Its output
then declares variable `Next` and operator `Next`; pinned SANY rejects the module.
Changing `Next` to `vars` has the same outcome. The retained complete matrix also
contains three other SANY-rejected translator outputs: two `define` cases and a
shadowing `with` binder. All five remain invalid-artifact evidence, never parity
credit. Raw output and module/config hashes are in
`crates/dive-pluscal/translation-acceptance/fixtures/manifest.json`.

Native translation refuses generated-name collisions and unproved binder
shadowing before returning an artifact. The API also performs enclosing-module
syntax/name/level admission. This is a documented current limitation in both
profiles, not an approved alternative name-disambiguation algorithm. Precise
TLC-equivalent disambiguation and rewrite/checksum parity remain pending.

**[D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents) public Rust context update:** a caller-created invalid evaluator redirect
previously panicked after interning a literal, while malformed lazy expression
IDs could enter captured scopes. Constructor and scope admission now produce
stable `DIVE-EVAL-C001`–`C005` refusals before evaluation or interning. Independent
trace/predicate APIs also reject foreign model/evaluator metadata before replay
setup. Valid configured models keep TLC's existing startup diagnostics. The
checks apply equally to both profiles; no compatibility flag disables them.
Original native observations and source are retained in
`crates/dive-api/tests/fixtures/public-context/`. Bare numeric IDs do not carry provenance, and opaque deferred
Rust bindings refuse rather than call user code to establish their shape.
F-035 remains partial; this is not a full Rust API or IR certification.


Temporal predicate integration fixes a native limitation: TLC's liveness evaluator skips an action body on a subscript stutter, while ordinary action evaluation checks the body first. Native shared temporal atoms and independent angle/box/fairness-occurrence replay now follow the liveness order. The minimal division-by-zero example and all 36 direct pinned controls are in `conformance/probes/temporal-action-order/README.md`. Broader conversion error precedence and strict liveness integration remain pending; this is a TLC-matching fix, not a new default divergence.

## D-020: fair-algorithm checksums do not detect source edits

This minimal source has the same algorithm checksum after changing `x=0` to `x=9`:

```tla
---- MODULE FairCRC ----
EXTENDS Integers
(* --fair algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)
====
```

The pinned translator emits pcal checksum **`7c28162a`** for
initial values 0, 1 and 9 in both C and P syntax.
Their TLA-body CRCs are **`64f9cea5`, `7faa8ba4`, `a530a3ac`** respectively. Editing just the algorithm initializer in
the original translated file from 0 to 9 still gives **`NO_DIVERGENCE`** in all
four original/edited C/P validator controls. This is a missed source edit, not
an admitted algorithm-equivalence claim: `trans.performTranslation` and
`Validator` hash `"fair"` alone because of Java ternary-expression precedence.

**Status: measured upstream candidate; no corrected fair CRC adopted.** Native
rewriting retains the reference CRC for its supported procedure-free AST subset,
including this measured constant CRC. It does not certify source identity.
If an exact algorithm CRC is unavailable, rewriting disables that value and
retains the real body CRC. Both `--checksums full` and `--checksums tla` select
this best-effort policy in either profile. The native body overwrite grant and
ordinary primary overwrite behavior are described under [D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents). A future
corrected fair option still needs its own approved default decision.

All six primary sources/artifacts and four unchanged-body source-edit controls
remain in `crates/dive-pluscal/rewrite-acceptance/fair-checksum/`. Body CRC and
usable artifact admission are separate contracts ([D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents)). See
[block rewriting](pluscal-translation.md) for executable CLI/Rust usage and limits.

Bounded final scope evidence now distinguishes configured names from
local/formal/INSTANCE derived roles; see the scoped final guide.
Exported INSTANCE temporal-formal and opaque/effectful combinations remain named
refusals. The additive property-origin increment below exposes original
expanded Action names separately; its metadata is not a new result-changing
default or ordinary liveness certificate. Ordinary periodic scheduling remains
unsupported; the whole small-model study from the prior IF slice is unchanged.


**Historical checksum-era scope (superseded by):** **[D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents) checksum continuation:** nested `elsif` and multiple `with` binders now
have bounded reference AST serialization. Strict operator-tree comparisons
caught dive omitting the pinned empty-`else` TRUE conjunct; the original mismatch
and paired empty/explicit-Skip controls remain visible. Native now preserves
that conjunct. Raw multiline checksum matching remains separate from public
translation, which explicitly refuses unproved multiline layout and empty
`then` generation. CRC agreement never means the model was proved or that an
invalid generated artifact is admitted. See
[the scope and executable examples](pluscal-translation.md). [D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits)
continues to expose the fair-header-only checksum weakness without correcting
it by default.

### Native concrete capture limitation narrowed

TLC accepts a split action capturing `<<[tag |-> "ok", vals |-> {a,b}], <<1,2>>>>`
and configured string/model/set constants. The earlier native final checker
refused all32 selected runs at1/4 workers. A callback-free stored-value proof
now admits the paired final controls while preserving opaque/cached providers,
invalid namespaces, effects and existing50k/128 bounds as refusals. Public
callers must supply sets/records/general functions with the evaluator's stored
ordering profile; same-owned atoms alone do not preserve cached CHOOSE order.
Tuples have profile-independent integer domains, but nested values still match.
The pre-order-guard observations and interrupted gate remain blocked evidence.
This matches
the reference and does not change any approved default. The original failed
selection remains evidence; see the concrete capture guide.
Ordinary periodic checking and full family certification remain unfinished.


**Historical checksum-era scope (superseded by):** **[D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents) finite-process Full reports.** Declared-fairness finite process records
now have proved native post-translation AST/CRC derivation, separately checked
from admitted generated modules. Raw multiline process checksum matching does
not lift the generator's layout refusal. Supported explicit process fairness and
automatic labels have the separate [option contract](pluscal-translation.md);
source-option declarations, name rewrites and procedure/macro ASTs remain named Full refusals. Both profiles
report nonproof editing results. See [process checksum examples](pluscal-translation.md).
[D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits)'s fair-header-only source detection weakness remains explicit and unchanged.

The process checksum source study retains a native generator CST failure for a
long singleton unchanged variable. The pinned five-unit UTF-16 identity spelling
rule is now matched; this restores artifact fidelity in both profiles, without
a new divergence or proof claim. Opposite process modifiers and default-value
name conflicts have separate captured dispositions.

Pure operator-formal forwarding now has bounded explicit-final TLC admission;
this resolves a native refusal rather than adding a result-changing default.
Known user targets preserve caller scopes and are inspected even when unused;
opaque/native/recursive/higher-order and configured-origin-ambiguous targets
remain refused. Clean accepted INSTANCE operator-formal controls are separate
from older exported temporal-expression failures2213. Full periodic/corpus/family
parity is unclaimed; see the operator guide.

## D-021: PlusCal checksums need the generation options

```tla
---- MODULE OptionCRC ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variable g=0;
process P \in {1,2} begin L:g:=1; E:skip; end process;
end algorithm; *)
====
```

The clean pinned `pcal.trans -wf OptionCRC.tla` keeps only two hexadecimal CRCs
in the translation header; it does not persist `-wf`. In the retained minimal
P-syntax control the pcal CRC is `247b4187`. A fresh unconfigured Validator gives
`PCAL_DIVERGENCE_EXISTS` without any edit. The same Validator configured with
`-wf` gives `NO_DIVERGENCE`. The paired `-sf` and inserted/custom-label controls
show the same false change indication. An explicitly `fair+` process translated
with `-nof` instead validates both ways because the source AST retains declared
fairness even when the generated formula omits it.

This is a context problem, not a source-equivalence or model-proof result.
Pinned Validator reparses using its current options and source `PlusCal options
(...)` declarations. The four retained in-source declaration controls validate
unconfigured. Native's first option producer ignored those declarations: weak
and strong cases produced admitted replacements with the wrong fairness, while
missing-label cases were rejected. Those failures and exact primary outputs are
preserved before the native source-context guard.

**Status: measured upstream interoperability candidate; no corrected default.**
For supported procedure-free process shapes, dive derives exact CRCs from explicit
CLI/API options and reports `translation_options` on every replacement request.
The API has one fairness enum; the CLI rejects repeated/conflicting fairness
flags before loading. Native generation and exact derivation use the source fairness/label
subset documented in [source options](pluscal-translation.md); unknown,
malformed, conflicting and other unproved source contexts refuse by name. Store/replay explicit settings with the
artifact and configure reference validation consistently. An unconfigured
Validator result never proves the source was edited.

When available, the compatibility counterpart is the same pinned CRC/options
context under `--compat=tlc`. Both checksum flag spellings now select best-effort
CRC derivation; unavailable algorithm CRCs are disabled in either profile. The unchanged `--fair algorithm` weakness ([D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits))
can also hide changes, and its `NO_DIVERGENCE` result does not certify identity.
Raw AST/CRC agreement, usable artifact admission, finite checking and independent
witness validation stay separate ([D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents)).

See [runnable option examples and limits](pluscal-translation.md)
and `crates/dive-pluscal/process-option-checksum-acceptance/validator-context/`
for all primary/configured/unconfigured/native-before-guard evidence. Broader
source-option parsing, scoped serialization, exact formatting and file writes
remain pending rather than inferred from this bounded increment.

[D-021](#d-021-pluscal-checksums-need-the-generation-options) original-source control: a declaration inside a previous translation
block is still read by the pinned translator. The retained native producer
masked it first and admitted both Full and body-only output without requested
weak fairness. High-level native rewriting now refuses before masking; the
minimal source and both failed outputs are in
`conformance/probes/pluscal-source-option-masking/`. Source-option parsing and
precedence are not fully implemented; no stronger checksum default is adopted.

### Bounded pure native operator arguments

The explicit final TLC mode now accepts Len/Head/Cardinality operator actuals
within its bounded pure constant/state slice. TLC13 and dive13 agree on the
minimal `Use(F(_)) == []<>F(x); Live == Use(Head)` with `x=<<FALSE>>`.
Configured/opaque/effectful targets still refuse. Concrete normalized Enum/Raw
set states now have callback-free stored-successor admission with preserved
before/refusal evidence. Unknown deciding runtime error classes and ordinary
periodic scheduling remain limitations. See the guide and the complete
20-control probe selection; this is bounded support, not family certification.

for broader source-option parsing and precedence; the bounded proved subset
is documented in [source options](pluscal-translation.md). No stronger
checksum default is adopted.


### D-021 retained source-option errors and partial application

The pinned translator also continues after some source-option errors. In the
minimal C and P process controls, `PlusCal options (-wf, -sf)` prints an
`Unrecoverable error`, exits **0**, writes a translation, and keeps the first
weak-fairness setting. `PlusCal options (-unknown)` likewise exits 0 and writes
a translation after reporting an unknown option. An unterminated declaration
can accidentally consume a later `)` from the algorithm and apply a setting.
The initial missing-label study and the corrected explicit-label study are both
retained in `crates/dive-pluscal/source-option-semantics-acceptance/prestudy/`;
the latter has 16 clean primary runs and carries no native conformance credit.

The bounded native continuation refuses malformed, unknown or conflicting source
options before artifact/CRC generation instead of silently accepting partial
application. This is an explicit bounded unsupported disposition, **not** a
new corrected checksum or fairness default. A source declaration overriding
CLI fairness is intended reference behavior and is tested separately. Artifact
acceptance still requires enclosing-module analysis; zero from the translator
never proves model correctness or a usable artifact ([D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents)).

## D-022: Action composition works by default, with its TLA+ meaning

Core supports the bounded pure mathematical slice described in
[compatibility](user/compatibility.md): complete enumerable first stages and
conjunctive/`LET` tails assigning each variable once from unprimed expressions.
Its `Reduction` control has 3 distinct states, depth 2 and 7 final generated
states; pinned TLC with the switch has 13 generated states. Duplicate final
candidates remain counted. Other shapes retain named refusals, and Core's
nominal compatibility policy still refuses TLC's intermediate-accounting mode.
The historical Reference-engine measurements below are unchanged.

Ordinary TLC-profile bounded final checking accepts the existing
`--action-composition=tlc` option with otherwise default resident settings.
It preserves intermediate generated counts. Independent replay still refuses
reached composition, so a candidate requiring it is not published as a violation.
See [temporal checking](user/temporal.md) for the exact option scope.

| | |
|---|---|
| **Kind** | Different answer (only for specs that use `\cdot`) |
| **Default** | `--action-composition=tla`: `A \cdot B` is evaluated, in successor generation, in expressions and under `ENABLED`, as TLA+ defines it |
| **TLC behaviour** | `--action-composition=off`, part of `--compat=tlc`: every use is TLC's error. `--action-composition=tlc` (also spelled `-Dtlc2.tool.impl.Tool.cdot=true`) reproduces TLC with that switch exactly |
| **Status** | default in `dive-eval` (`ActionComposition::Tla`); the TLC modes are implemented and tested against the pinned TLC |

**What TLC does.** TLC refuses `A \cdot B` ("The current version of TLC does not support action composition") unless its JVM runs with `-Dtlc2.tool.impl.Tool.cdot=true`, which enables what TLC calls an incomplete implementation. With the switch, TLC enumerates the states `t` that `A` leads to, then the states `B` leads to from each `t`. Two things go wrong:

- **It counts the intermediate states as generated, and so hides deadlocks.** TLC decides that a state is deadlocked when expanding it generated nothing. The intermediate states count, so a state whose `A` steps all lead to states where `B` is disabled is not reported:

  ```tla
  VARIABLE x
  Init == x = 0
  Next == (x' = x + 1) \cdot (x < 1 /\ x' = x + 1)
  ```

  `A \cdot B` has no step from `x = 0`, so this spec deadlocks at once. The pinned TLC (with the switch, deadlock checking on) reports success: 2 states generated (the initial state and the intermediate `x = 1`), 1 distinct state.
- **Conjuncts after the composition are also applied to the intermediate state,** and a variable assigned before it is pre-set in the intermediate state to its current value:

  ```tla
  VARIABLE x
  Init == x = 0
  Next == ((x' = x + 1) \cdot (x' = x + 1)) /\ x' = 2
  ```

  From `x = 0` the formula allows the step to `x = 2`. TLC also checks `x' = 2` against the intermediate state, where `x = 1`, generates nothing and reports a deadlock (1 state generated).

Without the switch, `TLCGet("-Dtlc2.tool.impl.Tool.cdot")` returns its own key; with it, `"true"`. Examples `FiniteMonotonic/MCCRDT` and `MCReplicatedLog` assume the latter.

**What dive does.** By default, dive evaluates `A \cdot B` with its TLA+ meaning: `s -> u` is a step iff some state `t` has `A` from `s` to `t` and `B` from `t` to `u`. Only the composed successors count as generated, so the first spec above is reported as deadlocked. Where the composition sits inside a conjunction (conjuncts after it, or variables already assigned before it) and TLC's algorithm would give a different answer, dive refuses by name rather than approximate. `TLCGet("-Dtlc2.tool.impl.Tool.cdot")` is `"true"` whenever composition is enabled.

Under `--compat=tlc` composition is off, as in TLC. `--action-composition=tlc` reproduces TLC with the switch, including the counts and the hidden deadlock. dive runs it in sequential mode, because the intermediate states count as generated but are not successors. The conformance harness passes it for models with the Examples `action composition` feature, as it passes the switch to TLC. DFID and simulation count only emitted successors in both enabled modes, following their pinned TLC functors. For `CdDeadlock`, each reports deadlock (2114) with one generated state even in `tlc` composition mode; the pinned TLC exits 11. Composition being disabled retains the named refusal of an unverified DFID/simulation evaluation-error context. Ordinary temporal composition and its captured intermediate accounting remain unfinished.

**Independent bounded replay and fairness.** In `tla` mode, replay admits pure
finite complete intermediates with at most one complete tail successor, using
the shared evaluator and a fresh private cache domain. It applies a
nonstuttering continuation to the actual original-to-final edge. For
`A == (x'=1) \cdot (x'=0)` at `x=0`, the finite relation has only the composed
edge `0 -> 0`: `ENABLED A` is true, `ENABLED <<A>>_x` is false, and `WF_x(A)` and
`SF_x(A)` hold on the stuttering word. Pinned experimental TLC instead reports
both queries true and both fairness properties false (exit 13, one-state
stuttering counterexample). [D-022](#d-022-action-composition-works-by-default-with-its-tla-meaning) covers exactly these nonstuttering-enabledness
and consequent fairness truth differences; it grants no unrelated observable
exception. The unchanged oracle facts and independent finite controls are in
`crates/dive-trace/tests/fixtures/composition/` and `tests/composition.rs`.
Replay still refuses `off`/`tlc`, effects, nested or unproved compositions,
partial states and nondeterministic tails. See [trace validation](user/traces.md).

**Evidence.** `crates/dive-eval/tests/cdot.rs`, with specs and compact generated pinned-TLC results in `crates/dive-eval/tests/fixtures/cdot/`:

| Probe | TLC with the switch (generated / distinct) | dive default |
|---|---|---|
| `CdNext` (top-level composition) | success, 65 / 18 | deadlock: a state with `y = 3` and `x < 4` has `A` steps but no `B` step |
| `CdDeadlock` (the first spec above) | success, 2 / 1 | deadlock, 1 / 1 |
| `CdConj` (the second spec above) | deadlock, 1 / 1 | refused by name |
| `CdEval` (action property) | `Prop` violated, 5 / 5 | the same |
| `CdEnabled` (`ENABLED` in an invariant) | `Inv` violated, 2 / 2 | the same |
| `CdOff` (no switch) | error 1000 | the same under `--compat=tlc` |

`--action-composition=tlc` matches every TLC row. `MCCRDT` and `MCReplicatedLog` now get past the composition in both profiles, but both also check liveness properties, which `dive check` still refuses. Their safety part, checked without `PROPERTIES` at one worker, matches the pinned TLC goldens' counts:

| Model | TLC golden (generated / distinct / depth) | `--compat=tlc --action-composition=tlc` | dive default |
|---|---|---|---|
| `MCCRDT` | 1,350,001 / 25,000 / 16 | 1,350,001 / 25,000 / 16 | 900,001 / 25,000 / 16 |
| `MCReplicatedLog` | 17,425 / 1,363 / 13 | 17,425 / 1,363 / 13 | 11,617 / 1,363 / 13 |

The default profile's generated counts leave out TLC's 450,000 and 5,808 intermediate states.

The conformance registry recognizes exactly these two generated-count pairs in
its default profile, with no other differing field allowed. Matching runs are
`expected-divergence`, never strict agreement. Any other count, verdict, exit,
depth, property or trace-validation difference remains a finding; TLC and
TLC-with-overrides profiles receive no exception. This harness rule does not change the historical safety-only evidence
above or certify a new temporal capability.


## D-023: Default process exits are distinct conformance observables

**Current CLI decision ([check exit conventions design](decisions/0097-check-exit-conventions.md)):** ordinary checking now selects TLC physical
statuses independently of native semantics; `--exit-codes native` and the
historical Rust API preserve the old contract. This changes actual runtime
status, not conformance comparison. The measurements below are historical;
new complete matrix results and the original floors remain separate.


The earlier accounting correction preserved the then-default statuses described
in [D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents): property violations exited 10 and rejected inputs exited 1. Those
statuses now require `--exit-codes native`; ordinary checking selects the pinned
TLC status independently of evaluation. For example:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --exit-codes native --workers 1 --seed 7 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --workers 1 --seed 7 --fp 0 --output json
```

The same invariant violation exits 10 with the native convention and 12 with
the default TLC convention. Both commands retain native evaluator semantics.
The corpus harness now compares actual numeric process exits exactly in both
its ordinary and worker-specific paths. A registered default difference can
be ExpectedDivergence, with the exact TLC/native numeric pair and every other
obligation met; it is never Agree. Absent, signaled and unrelated statuses or
failed independent counterexamples remain failures. Finite primary temporal
EC2116/exit12 retains its established classification; no general mapping to 13
is introduced.

The first correction registers only the 14 measured historical safety/frontend
paths and adds exact exit pairs to four existing semantic entries. It does not
register future liveness or scheduling differences. Old default agreement counts
omitted this required observable; [truthful default numeric exit conformance design](decisions/0096-truthful-default-numeric-exit-conformance.md)
qualifies them, and [the scoreboard](scoreboard.md#corpus-scoreboard) has the current measurements.


[D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-agents) PlusCal checksum/API continuation: schema2 translation reports contain real
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
backends and ordinary model checking of translated output are not complete. See the current
[PlusCal guide](pluscal-translation.md) and [pluscal translation design](decisions/0084-pluscal-translation.md).

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

## D-024: Pinned local Bits checking requires one worker

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

This is a proposal, not implemented behavior.


[D-013](#d-013-a-deterministic-short-liveness-counterexample) query-language continuation: exported INSTANCE ENABLED searches now retain
partial successors by canonical variable name, including hidden names. Both
profiles follow the measured cutoff/captured-context distinctions documented in
[temporal checking](user/temporal.md#instance-query-namespace-controls).
Independent INSTANCE query-word validation and general callable/temporal
integration remain unfinished. Under [D-022](#d-022-action-composition-works-by-default-with-its-tla-meaning), composition still requires root-only
intermediates; assigned hidden query names refuse instead of being silently
dropped. This changes no comparison, model limit or approved default.

## D-026: Run budgets stop a run with a structured result

TLC runs until it finishes or its JVM runs out of memory. A runaway liveness
check can occupy a machine for hours, and running out of heap prints
`SYSTEM_OUT_OF_MEMORY` and exits 1 or 255 with nothing to act on. dive bounds
every `dive check` by two budgets ([run budgets design](decisions/0107-run-budgets.md)):

```sh
dive check Spec.tla --time-limit 10m --max-memory 8g --output json
```

- `--time-limit` defaults to `none` in both profiles, which matches TLC. Set
  it to bound a run.
- `--max-memory` defaults to `auto`, half of physical memory, in both
  profiles, including `--compat=tlc`. That is at least the footprint TLC gets
  by default (a quarter of memory for its heap, plus off-heap fingerprints).
  `none` removes it. Like the host-access grants of [D-017](#d-017-host-access-requires-explicit-grants), the ceiling is a
  host-resource exception to compatibility mode: it protects the machine and
  can only stop a run, never change a result. Its TLC spelling is `-Xmx`:
  `dive tlc -Xmx8g Spec` sets it, as does `--max-memory 8g`. An explicit
  `--time-limit` under `--compat=tlc` labels the profile `tlc+overrides`,
  because TLC has none.

When a budget runs out, dive stops and reports what it knows instead of a
verdict:

```json
{ "verdict": "stopped",
  "stopped": { "resource": "time", "limit": 600000, "observed": 600050,
               "elapsed_ms": 600061, "unit": "ms", "phase": "safety-search",
               "hint": "raise --time-limit, or pass --time-limit=none; ..." },
  "statistics": { "distinct": 81234567, "left_on_queue": 412,
                  "reported_by_tlc": false } }
```

The exit status is 5 under both `--exit-codes` conventions. `observed` is
when the budget ran out; `elapsed_ms` is when dive actually stopped, which can
be later because binding, `ASSUME` evaluation and a single long evaluation
(for example an `Init` whose domain must first be built) are not interrupted. A stop is never a
success, a violation or `unsupported`: nothing was proved about the model. To
get TLC's unbounded behaviour, pass `--max-memory none`.

`dive simulate` and `dive dfid` also accept these time/memory flags, with the
same defaults and TLC-equivalent settings. They report `result: stopped`, mode
phase and completed-prefix progress, exit 5, and no certified verdict. Their
clock begins after model loading. Execution, retained-state serialization
checkpoints and fresh independent finite-word replay share one budget across
all traces or iterations. Exhaustion skips unprocessed retained traces. Initial
loading and individual value encoding/decoding remain uninterruptible; a final
resource sample precedes publication. See [simulation](user/simulation.md#run-budgets) and
[DFID](user/dfid.md#run-budgets).

Two further opt-in search budgets, `--max-states N|none` and
`--max-depth N|none`, bound discovered in-model identities and BFS discovery
depth (initial states have depth one). Both default to `none`, their
TLC-equivalent setting; TLC's simulation `-depth` does not bound exhaustive
search this way. Finite settings under `--compat=tlc` are reported as
`tlc+overrides`. A new identity that would exceed either ceiling is not
admitted; the run reports resource `states` or `depth`, actual admitted
progress, `statistics.exhaustive: false`, and exit 5. A finite graph exactly
at the ceiling can still exhaust and finish normally. See the
[checking guide](user/checking.md) for worker and recovery semantics.


Native `dive debug` and Rust sessions observe actual compiled Core safety
execution in either semantic profile. They use a bounded DAP protocol and
stop-scoped readable scalar storage; broader maps/inspection and TLC `-debugger`
transport remain unfinished. Ordinary checking leaves observation off. See
[the native debugging guide](user/debugging.md).

## D-028: Canonical finite sets

Core constructs finite set literals and mapped sets as mathematical sets:
construction order, repeated elements and TLC's prior normalization of the same
object do not change their identity. For example, both orders of
`{<<1>>, [a |-> 1, b |-> 1]}` are accepted by the default profile. This extends
[D-009](#d-009-comparisons-that-tlc-refuses-although-tla-answers-them) under the [core is the product design](decisions/0153-core-is-the-product.md) decision. Undefined direct scalar comparisons,
such as `1 = "a"`, still report an error.

Canonical enumeration can also reduce generated counts. The pinned
`NxInLazyCup` example (`r' \in [a : {1}] \cup [a : {1, 2}]`) generates
10 states in TLC and 7 on default Core; both find the same 3 distinct states
at depth 2. This exact [D-028](#d-028-canonical-finite-sets) difference is separate from exploration-order
changes ([D-010](#d-010-the-order-in-which-sets-are-explored)).

Symbolic powersets, function sets and sequence sets retain lazy membership and
streaming; this policy does not require enumerating huge or infinite domains.
Using a symbolic definition as an element, function key or stored state can
require an identity that Core has not established. Such requests explicitly
return `unsupported`; symbolic structure is never treated as mathematical
inequality. The independent trace validator also refuses unproved identity,
including in words claiming a specification evaluation error.

### Default outcomes and real errors

Canonical construction also applies in the shared evaluator and independent
replay. Raw mixed-set normalization no longer masks a mathematical result:
`graph-demand/DemandedEdges` is an independently validated initial `Inv`
violation, while the finite graph/Product/projection controls listed in the
[D-028 registry](divergences.md#d-028-canonical-finite-set-construction) complete
with exact tested counts. Malformed graph fields, non-function arguments to
`AntiFunction`, and empty arguments to `LongestCommonPrefix` still report their
real errors. Core also reaches the non-set operand error in
`UnknownFirstProjection` and `PrefixUnknownFirst`; it refuses the unproved
infinite enumeration in `PrefixInfiniteFirst`. These checks do not require
constructor or receiver history.

### Compat gaps

Constructor admission can refuse before an operation TLC would leave unused.
The explicit regression lists include `graph-demand/SelfDemand`,
`graph-empty-product/LazyEmptyProduct`,
`product-operations/FiniteRawAxisHistory` and
`set-conversion-demand/NormalizedProjectionSkipRaw`; these nominal Core/TLC
refusals name the mixed-shape comparison gap. Default checks and pinned
Reference/TLC checks retain their separately tested outcomes.

`--compat=tlc` retains inexpensive existing switches, including integer and name
order. Core also uses canonical set producers in this mode. It refuses mixed
comparison shapes by name rather than emulating TLC's constructor history.
Examples that TLC can leave unnormalized but Core refuses include
`SeqProductSkipRaw`, `MappedEmptySubsets`, `SeqProductFiniteSkipRaw`,
`SeqProductFiniteDemandRaw` and `MappedMixedSubsets` in
`crates/dive-api/tests/fixtures/core-receiver-consumers/`. Mixed-shape literal-history and core-normalization fixtures likewise retain
named compat refusals, including `OrderShortCircuit`, `SizeShortCircuit` and
`ExcludedRawState`. These gaps do not
change the trace format or permit an unvalidated counterexample.

## D-029: Predicate-witness counts

Core evaluates an applied recursive Boolean function in `Next` as a predicate
when the complete compiled plan has no effects or unknown call targets. This
conservative check includes indirect compiled function calls. If several
existential witnesses prove it true, they do not each create another
copy of the same successor. For example:

```tla
Next == LET f[n \in 0..1] == IF n=0 THEN TRUE ELSE \E k \in {1,2}: f[n-1]
        IN f[1] /\ UNCHANGED x
```

With `Init == x=0`, TLC generates three states (Init plus two identical
successors); Core generates two, with the same one distinct state, depth one
and successful verdict. Original Examples `byzpaxos/VoteProof.cfg` likewise
succeeds with 6,962 distinct states and depth 14, generating 27,727 states on
Core versus TLC's 318,877. Core reports its actual counts.

This approved default avoids redundant predicate-witness bookkeeping.
The exact two generated-count pairs are registered as default expected
divergences. For these controls, verdicts, distinct counts, depth, properties,
error classes/exits, queue counts and independent word validity keep their
contracts.

Boolean short-circuiting is also deliberate. In the pinned `OrError.cfg` control,
the predicate is already true, so Core does not demand a later `1 \div 0`
alternative. Core succeeds with 1 distinct / 2 generated / depth 1 and exit 0;
TLC's action generator instead reports division by zero (2179, exit 255) with an
error prefix. A separate exact-model default registration records only those
verdict/class/error-prefix and exit-pair differences. Counts and independent
word-validation obligations remain unchanged; other findings are not waived.

Nominal `--compat=tlc` uses the same Boolean evaluation on Core; it does not
reconstruct TLC's duplicate witness expansion. Strict TLC comparisons still
report these count and short-circuit differences. Plans with compiled effects
(including register operations, randomness or output) or unproved calls refuse
rather than changing observable behavior. Recursive evaluation, including
arguments, polls budgets and refuses beyond 64 active calls instead of aborting
on host stack exhaustion. Escaping recursive functions, primed applications,
unproved lexical/operator captures, nested recursion and other unsupported
contexts retain their named refusals.
