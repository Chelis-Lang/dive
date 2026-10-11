# Where dive differs from TLC, and why

dive is a Rust implementation of the TLA+ tools that is checked continuously against the canonical Java tools (SANY and TLC). This page lists every place where we think TLC does something wrong, or less well than it could, and dive does something different.

In every case below:

- **You can always get TLC's exact behaviour.** `--compat=tlc` (or `DIVE_COMPAT=tlc`) switches *everything* to TLC-equivalent behaviour at once. Each entry also names its own per-feature switch.
- **TLC-compatible behaviour is proven, not assumed.** dive's conformance harness checks it against the pinned TLC across the full test corpus.
- **dive never differs silently.** When a result in dive's default mode would come out differently under `--compat=tlc`, the output says so.

> **Status of this page.** dive is in early development. Entries marked *planned* describe agreed behaviour that is not built yet. This page is maintained alongside the code (see "Maintaining this page" at the end) and will become part of the user documentation.

**Status values:**
- **planned:** decided, not yet built.
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
| **Status** | implemented (`dive check --workers N`, M5 WP-D3): the deterministic parallel engine of ADR 0015. Every result is identical at every worker count and under randomized scheduling, and equals TLC's `-workers 1` golden under `--compat=tlc`. Specs that depend on TLC's sequential side effects run in *sequential mode* and say so (see below). |

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
- **`--compat=tlc`.** 70 models agree with TLC's single-worker golden at every worker count. The one exception is by design: `DieHard/MCDieHardest` asserts `TLCGet("config").worker = 1`. Under `--compat=tlc` dive reports the physical worker count as TLC does (D-011), so at `--workers 4` the assumption fails exactly as it does in TLC at `-workers 4`. In the default profile it reports 1 and the model agrees at every worker count.

Scaling figures are in ADR 0015 ("Implementation and measurements (WP-D3)").

**Evidence.** Probe P8 in `docs/survey/competitive.md`, ADR 0004, ADR 0010, ADR 0015, `crates/dive-check/tests/invariance.rs` (every probe and a wide spec stopping with every kind of event, at 1–64 workers, under chaos, in both profiles and stores), and the harness worker-invariance reports in `docs/conformance.md`.

---

## D-002: Randomness is always replayable

| | |
|---|---|
| **Kind** | Same answer, better path (reporting); different answer only when you fix a seed |
| **Default** | Fresh seed per run, as in TLC, but **the seed is always reported**; `--seed S` alone replays a run exactly, at any worker count |
| **TLC behaviour** | TLC's seeding and TLC's draws, bit for bit, under `--compat=tlc` (given TLC's `-seed S` and `-fp N`) |
| **Status** | implemented and wired into checking, in both modes, and verified against TLC. `dive check` accepts `--seed`, always reports the seed (and the fingerprint polynomial), and draws `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` from the run's streams: the start-up stream for constant definitions, `ASSUME`s and `Init`, and a stream per expanded state. Under `--compat=tlc` the draws are TLC's bit for bit (fixtures `TmRandom` and `TmRandomStartup` in `crates/dive-check/tests/fixtures/tlc-module/`). Like TLC, dive rebuilds a counterexample from a fresh start-up stream; when draws before `Init` make that impossible, dive reports the violation without a trace, as TLC does, and says why. `RandomSubsetSet` is still refused by name. The independent trace validator replays the draws too: given the run's seed and polynomial (`dive validate-trace --seed S [--fp N]`; the conformance harness passes the goldens' `-seed 0 -fp 0`), it checks a counterexample's drawn values against the run's streams, and refuses by name a draw it cannot place (ADR 0016, amendment 2). |

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

Under `--compat=tlc`, supported untransformed checking reproduces TLC's choices for a fixed `-seed` and `-fp`: Java's `java.util.Random`, TLC's per-state seeding and each operator's algorithm. Next-state draws use TLC's variable order (ADR 0014), and LET expressions are evaluated where TLC evaluates them. Checking-path randomness runs inline with its actual width reported.

**Current dive limitation (#178).** A configured VIEW combined with any source-level random operator refuses before binding, output or search in both profiles. The scan includes unused definitions, native aliases and cfg-selected native replacement targets: this is a conservative temporary boundary, not a claim that every such definition executes. TLC seeds expansion from the transformed VIEW fingerprint; dive's unfinished path used the raw predecessor. On the minimal `V == n` probe at seed 7/fingerprint 0, both found four states and the same invariant violation, but TLC assigned `x` values `0, 78, 49, 21` while the old native path assigned `0, 80, 94, 100`. Matching counts concealed wrong concrete draws. SYMMETRY/random combinations also remain refused. The preserved [source, clean primary and native outputs](../conformance/probes/view-random-seeding/README.md) keep that defect separate from TLC's own limitations. Both old concrete traces receive an explicit independent-validator refusal; no independent certificate was demonstrated.

**Evidence.**
- Harness review finding M5, probe P9, ADR 0010 §4.
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
| **Status** | the random streams are implemented in `dive-stdlib`: TLC's worker seeding, and dive's per-trace streams. Finite pure single-worker safety simulation is implemented (`dive-sim`, ADR 0020); other modes, spec-level randomness/effects and additional workers are named refusals. |

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
`crates/dive-sim/README.md` remain unsupported. Existing Java RNG and stream
controls live in `crates/dive-stdlib/tests/java_random.rs` and `dive_mode.rs`.

---

## D-004: Integers are unbounded

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Arbitrary-precision integers |
| **TLC behaviour** | `--int=tlc` (included in `--compat=tlc`): 32-bit integers with TLC's overflow error |
| **Status** | implemented in the value layer (`dive-value`, ADR 0012), in both modes. The CLI switch and the default flip follow once both modes pass the conformance harness. |

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

**What dive does.** dive computes with unbounded integers, as the language intends. Small numbers still use an inline 64-bit fast path. With `--int=tlc` every one of the reports above is reproduced word for word. When a run would have stopped with TLC's overflow error, dive's output says so, so the difference is never silent.

**Why it matters.** Four of the five other Rust or Go TLA+ checkers we measured widen integers silently, with no TLC mode and no indication in their output. dive does better by default and is TLC-exact on request.

**Evidence.** Probe P1, harness fixture `Overflow`, ADR 0010 (owner decision of 2026-10-08), ADR 0012. The fixtures `Ov*`, `Ar*` and `ErrCardSubset`/`ErrIntvSize` in `crates/dive-value/tests/fixtures/` hold TLC's transcripts, and dive's messages match them byte for byte.

---

## D-005: Symmetry with liveness uses the full state space

| | |
|---|---|
| **Kind** | Different answer |
| **Intended default** | Unreduced checking; disable symmetry for the whole liveness run |
| **TLC behaviour** | `--symmetry-liveness=tlc` (included in `--compat=tlc`): warn and proceed |
| **Status** | planned (with `dive-liveness`) |

**What TLC does.** Symmetry reduction lets TLC explore only one representative of each group of states that differ only by a permutation of interchangeable values, such as process ids. Combined with liveness checking ("something good eventually happens"), symmetry can make TLC miss real violations. TLC knows this. It prints a warning ("Declaring symmetry during liveness checking is dangerous. It might cause TLC to miss violations…", `EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), but then carries on and can report success.

Measured on the pinned TLC, it goes wrong in both directions:

- **A missed violation.** `x` moves between three interchangeable model values `a`, `b`, `c` at every step, under weak fairness. `PROPERTY []<>(x = a)` is false: `a, b, c, b, c, …` never returns to `a`. Without `SYMMETRY`, TLC finds that counterexample. With `SYMMETRY Permutations({a, b, c})`, TLC prints the warning and reports **no error**, after exploring 1 state.

  ```tla
  Next == x' \in S \ {x}
  Spec == Init /\ [][Next]_x /\ WF_x(Next)
  InfOftenA == []<>(x = a)
  ```

- **A false violation.** A token alternates between two interchangeable processes under weak fairness, and the property "every process holds the token infinitely often", `\A p \in S : []<>(tok = p)`, is true. The spec and the property are both symmetric. With `SYMMETRY`, TLC reports `Temporal property Fair was violated`, with a counterexample of one state followed by "Stuttering". The fairness condition forbids that behaviour.

**What dive will do.** The approved default checks the whole unreduced state space and reports that symmetry was disabled. An explicit `refuse` mode remains available. These modes are not implemented yet; current runs still refuse temporal properties and symmetry by name.

Under `--symmetry-liveness=tlc` dive reproduces TLC's answers, both wrong ones included. A counterexample that comes from TLC's symmetry reduction and is not a real behaviour of the spec is labelled as such.

**Evidence.** ADR 0010 (owner decision of 2026-10-08); ADR 0019 §4; probes `SymLiveMissed`, `SymLiveMissedNone`, `SymLiveSymProp` and `SymLiveWarn` in `crates/dive-liveness/tests/fixtures/tlc-probes/`; survey probe P4 (another checker raised a false liveness alarm under symmetry).

---

**Owner update, 2026-10-09.** The intended default is now `--symmetry-liveness=unreduced`: ignore `SYMMETRY` for the entire liveness run, including its safety checks, and report the unreduced counts. This replaces the earlier approved refusal default. `=refuse` remains available. `--compat=tlc` selects `=tlc`, including TLC's missed and false violations in the probes. Today these modes are planned; symmetry and temporal properties remain explicit refusals. Compare the sound mode against TLC run without symmetry, rather than excusing arbitrary verdict differences.

## D-006: `CHOOSE` does not depend on unrelated text in the module

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | A canonical order, independent of the module text |
| **TLC behaviour** | `--choose-order=tlc` (included in `--compat=tlc`): TLC's order, including string interning order |
| **Status** | implemented in the value layer (`dive-value`, ADR 0012), in both orders. `--choose-order=tlc` also needs names recorded in TLC's encounter order: `dive-model` does that, and on 556 corpus models no name both tables hold is out of TLC's order (ADR 0012, "For the front end"). |

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
  - comparing other kinds is an error (D-009).
- **Equal elements keep the first form written.** `{1..3, {1, 2, 3}}` prints as `{1..3}`, while `{{1, 2, 3}, 1..3}` prints as `{{1, 2, 3}}`.

**What dive does.** dive uses a canonical order that does not depend on the module text: names in the lexicographic order of their characters (Unicode code points), and everything else structured exactly as TLC orders it. Reordering or adding definitions never changes a result. This is one of the charter's metamorphic guarantees. With `--choose-order=tlc`, the order and the printing match TLC's.

**Evidence.**

- Probes P2 and P7 in `docs/survey/competitive.md`.
- ADR 0010 (owner decision of 2026-10-08) and ADR 0012.
- In `crates/dive-value/tests/fixtures/`:
  - `ChooseIntern` and `ChooseInternUnused` (this example);
  - `PrintChoose`, `PrintSets` and `PrintFcns` (122 printed values in all, matched byte for byte).

---

## D-007: Exhaustiveness claims can be unconditional

| | |
|---|---|
| **Kind** | Same answer, with a stronger guarantee |
| **Default** | Fingerprint mode, as in TLC, always reporting its collision bound |
| **Better option** | `--store exact`: full states, with an unconditional claim that every state was checked |
| **Status** | implemented: `dive check --store exact` runs the same search over canonical state encodings. It gives the same results as fingerprint mode on the TLC probes and harness fixtures (`crates/dive-check/tests/probes.rs`). Ordered safety-frontier spill is available with `--frontier-memory-bytes N` (zero forces disk), under both profiles/stores. It preserves concrete traces and ordered counts, and reports explicit I/O/cleanup failures. Visited stores, candidates, traces and graphs remain outside the frontier-only budget; their spill is still planned. See [frontier storage](frontier-storage.md) and ADR 0024. |
| **Status** | implemented: `dive check --store exact` runs the same search over canonical state encodings. It gives the same results as fingerprint mode on the TLC probes and harness fixtures (`crates/dive-check/tests/probes.rs`). Separate predecessor spill is available; seen/graph spill is planned. |

**What TLC does.** TLC stores 64-bit fingerprints of states, not the states themselves. A collision between two distinct states would silently skip part of the state space. TLC prints a probabilistic estimate of this at the end of a run, but the "no error found" verdict itself is probabilistic.

**What dive does.** dive offers an exact mode that stores full states and makes an unconditional exhaustiveness claim. Every result says which mode produced it.

**Separate disk predecessor option.** Safety BFS accepts
`--predecessor-memory-bytes N` (zero goes straight to disk). This moves the
parent/ordinal records used to rebuild concrete counterexamples, with separate
capacity/write/cleanup evidence. It does not bound the seen set, frontier,
candidate or reconstructed-path memory. Omit the setting for the existing memory
counterpart in either profile. Storage and cleanup failures never certify
completion. Simulation, DFID and graph capture refuse this staged setting.
See [predecessor storage](predecessor-storage.md) and ADR 0031.

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
frontier without Init regeneration; [native checkpoint documentation](native-checkpoint.md)
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
refused. See the [separate parallel study](../conformance/probes/native-checkpoint-parallel/README.md).

**Configured-key recovery (ADR 0038).** Pure supported VIEW and safety symmetry
now recover through the same reduced-key path as live search. The primary
minimal CpView/CpSymView/CpSymConcrete controls report 7/4/4, 5/3/3 and 3/3/3
(generated/distinct/depth) on pinned TLC; ordinary dive agrees. CpSymConcrete
keeps owner p2 in the actual witness even though the canonical key represents
p1. Recovery preserves that concrete state. Equivalent keys do not make a changed
Init or Next witness valid, and archives remain trusted local state, not proof.
Both profiles/stores retain the TLC counter-reset default; Retain is still
explicit.
Public Rust context aliases resolving constant replacements are admitted by their
actual definition IDs before binding. An effectful local alias refuses before
output or an archive; structurally valid arena metadata alone is not a purity
certificate. This is a native admission repair ([issue #164](https://github.com/Chelis-Lang/dive/issues/164)), not a TLC defect. See [raw controls](../conformance/probes/native-checkpoint-keys/README.md).

D-007 native recovery additionally composes with the existing opt-in frontier
and predecessor budgets, independently or together. This removes a native
limitation; it is not a new TLC defect or result-changing default. Saved concrete
frontiers/parents are reconstructed into fresh transient stores in reference
order. Archive/discovery workspace is excluded from both budgets; no total RAM,
Java byte-interop or archive certificate claim. Counter reset/Retain and pure
VIEW/symmetry witnesses retain their existing semantics. See
[the runnable recovery guide](native-checkpoint.md#recovery-with-transient-disk-storage)
and [separate preserved study](../conformance/probes/native-checkpoint-spill/README.md).


**Native recovery admission update (D-007, 2026-10-11):** pure runtime string/name
construction is admitted with the complete saved ordered name table. For
`n' = n + 1 /\ s' = ToString(n + 1)`, the bounded recovery example reaches the
same 4-state invariant violation as uninterrupted checking and clean pinned TLC.
TLC encounter order uses a reported actual one-worker inline fallback; the
canonical profile may use the parallel engine. Runtime effects/register reads
and independent TLC witness replay of name history remain honest refusals. This
removes dive's own checkpoint limitation, not a reference implementation defect.
See `docs/native-checkpoint.md` and the source/raw control study at
`conformance/probes/native-checkpoint-runtime-names/`.


## D-008: Results are data, and diagnostics are built for people and agents

| | |
|---|---|
| **Kind** | Same answer, better path |
| **Default** | Human-readable output; `--output json` gives a versioned schema |
| **TLC behaviour** | TLC-style output conventions under `--compat=tlc` |
| **Status** | `dive parse`, `dive analyze` and `dive check` implemented; the rest planned |

**What TLC does.** Results come as human-oriented text, or as the `-tool` message stream meant for the Toolbox. Counterexamples are available as JSON only through `-dumpTrace json`. SANY's error messages give a location, but no stable error codes and no suggested fix. SANY's command line exits 0 on semantic errors unless you pass `-error-codes`.

**`dive check` exit codes.** Under `--compat=tlc` a verdict exits with TLC's status (0, 10 to 14, 75 to 77, 150, 151, 255). dive's default profile uses its own, documented in `dive --help`: 0 no error, 1 rejected input, 10 property violated, 11 deadlock, 12 assumption, 13 evaluation error. In both profiles 2 is a usage error, 3 a refusal or resource limit (no verdict) and 4 an internal error.

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

`--output json` gives verdicts, statistics and counterexamples as structured state sequences, along with diagnostics and an `options` block that records the active profile, options and seed. Coding agents and Rust callers can act on this programmatically. The library API (`dive-api`) gives the same results in-process, with no JVM.

`dive analyze` also gives every semantic diagnostic the SANY error code it corresponds
to (`sany_code`), so tools written against SANY's codes keep working.

**Evidence.** ADR 0009, ADR 0011, `dive parse --output json`, `dive analyze --output json`, and probes M1 and M2 (diagnostic comparison in `docs/survey/competitive.md`).

---

**Bounded PlusCal multiline reports.** Sequential non-junction multiline
expressions now generate with reference lexical changed-variable substitution.
LF/CRLF, tabs, blank/comment lines and UTF-8 comments have separate retained
controls. Junction indentation, binders and macro/process/procedure expression
contexts still yield named unsupported reports. Full/body CRCs, whole operator
CST/SANY admission and finite counts/independent witness checks are separate;
neither a checksum match nor a translation report claims a checked model.
The input file is unchanged. See `docs/pluscal-multiline.md` and its complete
primary/refusal history. D-020's fair-only source-detection weakness is preserved,
not silently corrected.

---

## D-009: Comparisons that TLC refuses although TLA+ answers them

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | **TLA+ answers** for `=` and `\in` between functions, records, tuples and sets (`--comparisons=tla`; owner decision 2026-10-08). The error is kept only where TLA+ leaves the answer unspecified, such as `1 = "a"`. Set construction still follows TLC's rules. |
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

The owner approved TLA+ comparisons as the default on 2026-10-08. This changes
results for defined comparisons that TLC stops with an evaluation error; the
TLC counterpart remains implemented and tested. Set construction still follows
TLC's normalization rules, so some heterogeneous set literals remain refusals.

**Evidence.**

- ADR 0012.
- The fixtures in `crates/dive-value/tests/fixtures/` hold TLC's transcripts:
  - `AsymSetRecFirst`, `AsymSetTupleFirst`, `AsymEqTupleRec` and `AsymEqRecTuple`;
  - `ErrMemRcdTuple`, `ErrMemProductRec`, `ErrMemEnumStr` and `PrintMember`;
  - `RvFcnRcd1`–`RvFcnRcd8` and `RvCanonAsym`.

---

## D-010: The order in which sets are explored

| | |
|---|---|
| **Kind** | Different answer, only at a violation |
| **Default** | **Canonical order** (`--explore-order=value`; owner decision 2026-10-08): sets are explored in the run's value order, which is canonical in the default profile (D-006). Runs where the two orders would have stopped differently are flagged. |
| **TLC behaviour** | `--explore-order=tlc`, part of `--compat=tlc` |
| **Status** | default in `dive-eval` (`ExplorationOrder::Value`). `dive check` in the default profile notes, at every stop at a violation, deadlock or error, that the result may differ under `--compat=tlc`. Detecting whether the order actually mattered in a run is planned. |

**What TLC does.** TLC explores the elements of a set in its internal value order: the elements of `\E x \in S`, `\A x \in S`, `{x \in S : P}`, `{e : x \in S}`, `[x \in S |-> e]` and `x' \in S`. For strings and model values, that order is the order in which names first appear in the module (D-006). So the module text decides which successor TLC generates first, and with it which of two violations at the same depth TLC reports:

```tla
VARIABLE y
Init == y = "init"
First == "zeta"
Next == \E v \in {"alpha", First} : y' = v
InvAlpha == y # "alpha"
InvZeta == y # "zeta"
```

With `INVARIANT InvAlpha InvZeta`, the pinned TLC reports `InvZeta` after 2 states generated, because `"zeta"` appears in the text before `"alpha"`. Exploring in the canonical order reports `InvAlpha`, also after 2 states.

**What dive does.** In the default profile, dive explores sets in the canonical order, as it does for `CHOOSE` (D-006), so the whole default profile is independent of the module text. Complete searches give the same states, totals and depth as TLC. Only *which* of several violations at the same depth is reported first, and the counts at that stop, can differ. dive flags runs where that happened. `--explore-order=tlc` (part of `--compat=tlc`) reproduces TLC's order exactly.

**Evidence.** Probe `StringOrderViolation` in `crates/dive-check/tests/fixtures/tlc-probes/`; `crates/dive-eval/tests/options.rs`. Two Examples models show it in the default profile, with the same violated property, depth and counterexample length as TLC, and only the counts at the stop differing:

| Model | TLC (distinct / generated / queued) | dive default profile |
|---|---|---|
| `MissionariesAndCannibals` (`Solution`) | 61 / 225 / 11 | 61 / 226 / 11 |
| `acp/ACP_NB_WRONG_TLC` (`AC1`) | 11,852 / 59,286 / 1,248 | 12,645 / 64,357 / 970 |

Both are registered as expected divergences in `conformance/expected-divergences.toml`, narrowed to the fields that differ. Under `--compat=tlc` both agree with TLC exactly (the CM1 conformance gate).

---

## D-011: `TLCGet("config").worker` reports the logical worker count

| | |
|---|---|
| **Kind** | Different answer (only for specs that read it) |
| **Default** | `1`, the *logical* worker count, at every `--workers` setting (owner decision 2026-10-08) |
| **TLC behaviour** | Under `--compat=tlc`, the physical worker count, as TLC reports it. `--compat=tlc` also defaults to 1 worker, as TLC does |
| **Status** | implemented: `TLCGet("config").worker` is 1 by default and the requested `--workers` under `--compat=tlc` (`dive-check`'s `run_config`) |

**What TLC does.** `TLCGet("config").worker` returns the number of worker threads. Specs read it to protect themselves from TLC's approximate multi-worker breadth-first search (D-001). For example, Examples `DieHard/MCDieHardest` declares:

```tla
ASSUME /\ TLCGet("config").mode = "bfs"
       /\ TLCGet("config").worker = 1
```

so that TLC is guaranteed to find a shortest counterexample. Run with `-workers 4`, TLC fails that assumption, and the spec refuses to run.

**What dive does.** dive's parallel search is, by construction, TLC's single-worker search executed in parallel (D-001, ADR 0015). So by default dive reports the logical worker count, 1, at any `--workers` setting. Specs like `MCDieHardest` therefore run at full parallel speed and get exactly the guarantee they ask for. dive's own output still reports the physical thread count.

**Conformance.** Under `--compat=tlc` such a spec can behave differently at different worker counts, exactly as TLC does. The conformance harness therefore compares a spec that reads `TLCGet("config")`, run under `--compat=tlc` with more than one worker, with TLC's own result at the same worker count, on the verdict and the error class (ADR 0004, amendment of 2026-10-09). In the default profile the single-worker result applies at every worker count.

**Evidence.** Harness worker-invariance report (`MCDieHardest`), ADR 0014 §7, ADR 0015. `MCDieHardest` at 1, 3, 4 and 8 workers agrees with TLC in both profiles: under `--compat=tlc`, at 3, 4 and 8 workers both TLC and dive fail the assumption (`TLC_ASSUMPTION_FALSE`).

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

`dive validate-trace` reads dive's own trace format, whose values are exact, and TLC's `-dumpTrace json`, which it reads by trying the interpretations of the ambiguous parts (see ADR 0016). From Rust, `dive_api::trace::SpecModel` checks a program's own recorded states against a specification.

**Random values.** A trace whose values come from `RandomElement` or the `Randomization` operators is checked against the run's seed: give `dive validate-trace` the `--seed` (and `--fp`) that `dive check` reported. The validator draws the same values while it evaluates `Init` and each step, so a changed value is rejected. Without the seed, it refuses by name.

**Limits.** The validator confirms deadlocks when an independent finite-cover procedure proves absence of successors, and rejects claims when it finds a witness. Missing covers remain refused by name; only those cases use TLC's own ending state for harness confirmation. Pure concrete liveness words now validate when the trace supplies the complete, nonempty `properties` list. The validator checks the closing edge, all behavior temporal conjuncts, and that exactly the claimed temporal properties are false. Fairness and direct `ENABLED` now use independent finite covers; missing covers, effects, VIEW/symmetry closing evidence and unnamed legacy liveness words remain refused. Error traces are checked step by step, but re-raising their final error remains unsupported. Random values drawn outside `Init` and the next-state relation (in an invariant, say), under a `VIEW` or `SYMMETRY`, or in an order only the successor generator knows (one action's draws before another's, when the trace took the second) are refused by name too. See *Where dive is currently more limited than TLC*.

**Periodic-word evidence.** Ten pinned liveness traces validate, including recurring fair actions, intermittent enabledness (`LvWFIntermittent`) and a fair disabled-action stutter (`LvStutterEnd`). A non-stuttering occurrence on the repeated cycle proves both weak and strong fairness; occurrences only in the prefix, stutters and empty subscripts do not. Without that certificate, weak fairness needs a disabled cycle position and strong fairness needs all cycle positions disabled. These are checked independently; incomplete covers remain refused. `LvStutterActionConstraint` demonstrates that TLC adds implicit stuttering even under `ACTION_CONSTRAINT x' # x`: the pinned oracle reports a one-state stuttering violation at `x = FALSE` (exit 13, 3 generated / 2 distinct / depth 2). dive permits that implicit stutter while checking constraints on generated transitions. Closing-edge and property-claim mutations are rejected (`crates/dive-trace/tests/lasso_claims.rs`; ADR 0018 §8). This validator does not yet enable liveness model checking.

**Evidence.** ADR 0016 and its amendment 1. `dive-conformance trace-controls` runs TLC's own counterexamples through the validator: 12 of TLC's 15 safety counterexamples in the Examples goldens validate, and 3 are refused by name (two on evaluator operators not implemented yet, one whose `ALIAS` hides the variables). Every deliberately broken copy of them (a state dropped, two states swapped, a value changed, the trace cut short, the wrong invariant claimed) is rejected.

---

## D-013: A deterministic short liveness counterexample

**Status:** planned, owner-approved default. `--lasso=short` chooses a shortest prefix and a deterministic short cycle. It does not promise the globally shortest lasso. `--lasso=tlc`, included in `--compat=tlc`, reproduces TLC's selection.

**Library progress.** The bounded final-only coordinator now handles pure
predicates and WF/SF via scoped enabledness and non-stuttering actions. Ten
pinned violations match TLC counts in both stores and yield independently valid
candidates; three success controls also match, including enabled successors
outside a constrained graph. Every false configured property is attributed
and independently checked; omitted or extra claims are rejected. Capture now
uses ordered parallel source waves with reported pool width and a separate
scratch admission bound. TLC-exact planning/selection, phase-L error
accounting and periodic capture remain pending. Ordinary
`dive check` still refuses temporal model checking.

The Rust API [`dive_api::temporal::check_final`](temporal-api.md) returns a
periodic violation only after both its in-memory word and lossless trace
validate against a separately bound model. Unsupported validation stays a
refusal. Safety stops retain their statistics and finite counterexamples.
This API always selects short lassos, including with TLC value semantics;
it does not claim TLC-exact selection or periodic/error timing.
Finite constant-domain quantifiers and tuple bindings are now supported, with
original configured names retained even when the binder splits roles. Five
additional pinned probes match verdict truth and counts in both profiles/stores.
The empty-domain probe uses TLC's finite-prefix exit 12; exact class/trace timing
and the oracle parser correction remain tracked in issue #77.


**Example and measurement.** `LvNoFair` and `LvTwoLoopsRev` in the liveness probe corpus have several violating loops. Changing only TLC's fingerprint polynomial changed the counterexample on 6 of 21 measured models; `MCDistributedReplicatedLog` gave five choices, of 4, 5 or 6 states and with different named properties. Those implementation-dependent choices make diagnosis harder. Every dive lasso will be independently checked, including fairness and its claimed properties. Evidence: ADR 0017 §10 and ADR 0018 §5–§6.

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
Future action attribution remains tracked separately (#134). A further 84 supplied-formula controls include four named
shape refusals and TLC's integer-overflow bound on DNF expansion.

This supports the future TLC-compatible path; it does not change which
counterexample ordinary `dive check` returns. Strict integrated liveness,
strict execution, timing and selection remain refused. The existing
final/short path and its reported restrictions are unchanged. Evidence:
`conformance/probes/tlc-planning/README.md` and ADR 0025.

D-013 diagnostic planning now also projects bounded TLC particle/tableau
numbering and promise identities (ADR 0027). Actual pinned graph fields and
independent periodic/closing-witness controls cover this artifact; it never
reports a temporal verdict. Empty/nonpositive direct structures, actions,
unchecked group/tag/bin metadata, opaque/effectful scopes and resource limits
remain named refusals. Strict integrated TLC liveness, phase-L timing, SCC and
witness selection remain unsupported by this stage. Original configured/Action
claims and derived role names stay separate; no new default is adopted.

## D-014: Periodic liveness checking has a replayable schedule

**Status:** planned, owner-approved default. `--lncheck=deterministic` checks at the documented level boundaries in ADR 0018. `--compat=tlc` retains TLC's wall-clock scheduling for `--lncheck=default`; `--lncheck=final` checks the complete graph and remains the golden setting.

**Example and measurement.** The minimal `LvPeriodic` probe stopped at 418,795, 167,402 and 162,021 distinct states in three pinned-TLC runs. All found the same violation, but the stopping counts depended on time. Fixed level boundaries make those counts reproducible across workers. They are a named scheduling difference, not a claim of exact TLC scheduling. Evidence: ADR 0017 §6 and the recorded probe transcripts.

## D-015: Symmetry functions must actually be permutations

**Status:** bounded checked/TLC groups, supported pure startup warnings and pure safety key reduction are implemented. Unverified startup exceptions, declaration shapes, lazy/deferred permutations, random seeding and symmetry with liveness remain named refusals. A declaration containing `[a |-> b, b |-> b]` is not bijective. TLC accepts it and closes the set under composition, permitting an unsound reduction. The owner-approved model default will name and reject the offending function; `--compat=tlc` retains TLC's acceptance. The minimal missed-invariant control below supplies numerical evidence for that policy.

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
This does not enable symmetry with liveness or the opt-in symmetry audit.

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

**Status:** planned, opt-in. The `SymAsym` and `SymAsymLate` probes declare symmetry that the transition relation does not respect. TLC trusts the declaration, so the reduced search can hide behaviors. `--check-symmetry` will check initial states, successors, invariants, constraints and VIEW under the declared generators and report the first counterexample to the declaration. Ordinary runs, including `--compat=tlc`, omit this expensive audit. Evidence: ADR 0019 §6 and the probe transcripts; implementation measurements are pending.

## D-017: Host access requires explicit grants

**Status:** planned; host-access operators currently refuse by name. `IOEnv`, `IOExec`, `CSVWrite` and serialization operators can read the host or execute effects while a spec runs. For example, Examples `TLC/TestGraphs` reads `IOEnv` at startup and `ewd998/EWD998ChanTrace` reads it in its next-state relation. These dependencies cannot be replayed from the random seed alone.

The approved interface is `--allow-io=env,files,exec`, with no categories granted by default. Environment and file access come first; process execution stays deferred. Even `--compat=tlc` requires explicit grants and reports restricted compatibility when relevant operators are refused. Granted access and external inputs are recorded in the run. Evidence: the current named refusals and the 22 host-access assumptions in CommunityModules `AllTests`; no granted-access measurements exist yet.

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

- **Which liveness counterexample is reported.** TLC's liveness counterexample depends on the order of its internal hash tables, so it depends on the fingerprint polynomial (`-fp`), which TLC picks at random unless you set it. On the Examples spec `MCDistributedReplicatedLog`, six polynomials gave five different counterexamples, of 4, 5 or 6 states, naming either `InSync` or `AllExtending and InSync` as violated. TLC's choice is also not the shortest available: for `[]<>(x = 0)` on a counter `0, 1, 2` with no fairness, TLC shows three states and then stuttering, where stuttering after two states already violates the property. dive's proposed default (`--lasso=short`) reports a counterexample with the shortest possible prefix, the same one at any polynomial and any worker count. TLC's exact choice would stay available as `--lasso=tlc`, in `--compat=tlc`. The verdict and the state counts are the same either way. Open question 14; ADRs 0017 and 0018; probes in `crates/dive-liveness/tests/fixtures/tlc-probes/`.
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
- **"States generated" counts that depend on how a set is represented.** TLC counts every successor its enumerator produces, duplicates included (ADR 0015). For `x' \in S` it walks `S` without normalizing it when `S` is a lazily represented set, so an element in both halves of a lazy `\cup` is generated twice:

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
- **String length in UTF-16 units.** TLC's `Len`, `SubSeq` and `Tail` on strings count UTF-16 code units, not characters (tlaplus#1076). See `docs/survey/oracle-history.md`.
- **Quirks of TLC's `.cfg` reader.** TLC's model-configuration reader accepts some files that it will later fail on, and it reads some values in ways you might not expect. Each case below was measured on the pinned TLC:
  - `CONSTANT N = -1` is not a negative number. TLC reads `-` as a model value, and then fails on the next line with "expecting = or <-" (`CFG_EXPECTED_SYMBOL`, exit 151). A cfg file has no way to write a negative integer.
  - `N = 2147483648`, or `N = \h1F`, stops TLC with an unexpected `NumberFormatException` (`GENERAL`, exit 255) instead of a configuration error.
  - `INVARIANT Inv1, Inv2` is accepted, with the comma read as an invariant name. TLC then fails with "The invariant , … is not defined".
  - `INIT NEXT` (a forgotten name) makes `NEXT` the initial predicate.
  - `{1,}}` is read as a set containing the model value `}`.
  - String values keep their escapes verbatim, so `"a\nb"` in a cfg is four characters, unlike the same literal in a module.

  dive matches TLC exactly, and adds warnings (`DIVE-C0100` to `DIVE-C0102`) where TLC accepts input it can never resolve. A future opt-in `--cfg=strict` could reject these files. The cases and TLC's recorded runs are in `crates/dive-config/tests/fixtures/` and `crates/dive-config/acceptance/README.md`. TLC versions before `d17f1d35e` did worse: a lexical error silently truncated the file, so TLC could report success while ignoring an `INVARIANT`. The pinned TLC rejects such files, and so does dive.

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
  - a Java-overridden operator dive does not implement yet: `JavaTime`, the `Json` operators, most of `TLCExt` (`AssertError`, `TLCCache`, `TLCFP`, ...), `Randomization!RandomSubsetSet`, and the CommunityModules operators `FiniteSetsExt!kSubset`, `Combinatorics`, `Statistics!ChiSquare`, `VectorClocks!CausalOrder`, `GraphViz` and `SVG`. TLC never uses these operators' TLA+ definitions, so dive does not either;
  - cases of implemented overrides whose TLC result depends on something dive does not track: `BagsExt!FoldBag` on a bag of two or more elements, `Bags!BagOfAll` with an operator that has side effects, and the error a few `Bags`, `Functions` and `IsInjective`/`AntiFunction` cases raise when values TLC cannot compare meet in an order dive does not know;
  - `TLCGet` keys that read TLC's internal description of the run (`"spec"`, `"action"`, `"revision"`) or its JVM (`"-D..."`), the live counters (`"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"stats"`, `"duration"`) while states are being checked (they are provided while assumptions are checked), and `TLCSet("pause", TRUE)`;
  - the random operators when the evaluator runs without a model checker providing a random stream (for example `ASSUME`s evaluated on their own);
  - `ENABLED` in a build without the successor generator (the independent trace validator, `dive-trace`);
  - a function that TLC keeps unevaluated (`[x \in S |-> e]` over an infinite domain, or a recursive function) stored inside another value, when computing it could fail or has side effects;
  - enumerating `S \X T` or `[a : S]` with an infinite component, a set too large to count when it is enumerated lazily, and `IsFiniteSet` of a lazily evaluated set;
  - applying a function of 32 or more arguments to a value TLC cannot compare with its domain.

  An evaluation nested deeper than the depth limit is a resource limit, as TLC's stack overflow is.
- **Successor-generation refusals** (`dive-eval`'s successor generator). It refuses, naming the case, rather than guess the successors TLC would generate:
  - `x' \in S` where `S` is a lazily represented set (for example `[a : S] \cup [a : T]`, or a set filter or `UNION` built from one) whose TLC enumeration lists an element twice, or in an order other than the normalized one. TLC's answer depends on whether it has already compared, sized or fingerprinted that same set object (see *Under consideration*);
  - applying, as an action, a lazily evaluated function that has pending `EXCEPT`s or an argument outside its domain;
  - a built-in operator passed as an operator argument and applied as an action (`Op(\/)`);
  - `x' \subseteq S` where `S` is not a set.
- **`dive check` refusals.** The checker refuses, naming the feature, rather than give an answer it cannot vouch for:
  - temporal properties (liveness checking is not implemented yet) and `SYMMETRY`;
  - simulation mode and action composition (`\cdot`);
  - anything the binding stage or the evaluator refuses (for example `IOUtils`, `Json` and the CommunityModules operators listed above);
  - evaluation errors whose exact TLC error class depends on value-source metadata not yet retained by dive. The 175-model error matrix identifies ten such cases explicitly; dive reports their messages but marks the deciding class as unverified, and the conformance harness refuses agreement.

  Also: `ALIAS` is not yet applied to printed counterexamples (dive prints the states themselves, and says so), and `POSTCONDITION`s are evaluated after a complete search but not after a violation.
- **Trace validation** (`dive-trace`, `dive validate-trace`). The validator refuses, naming the reason, rather than trust the checker:
  - that the last state of a trace is deadlocked: checking it needs a search for successors, which the validator deliberately does not have. Every other check is still made, so a wrong deadlock trace can still be rejected. The conformance harness confirms deadlocks against TLC's own counterexample;
  - ordinary strict CLI temporal checking still refuses; explicit final/short CLI and library checks provide independently validated, complete property claims with bounded ordered parallel capture, while explicit schema-2 final-only pinned selection now has a separate API/CLI/harness contract; ordinary schedules and wider recovery remain refused;
  - liveness claims that lack a complete named-property list or need unestablished fairness, `ENABLED`, effects or VIEW/symmetry closing evidence. Pure concrete periodic words are checked through their closing edge and complete property set;
  - the final claim of an assertion failure or evaluation error (re-raising it needs the successor generator). The steps are checked;
  - a TLC `-dumpTrace json` document written under an `ALIAS` whose records do not contain the variables, and one too ambiguous to search exhaustively when no interpretation tried validates;
  - traces whose states leave variables unassigned (partial implementation traces), which need a search;
  - whatever the evaluator refuses, and reads of the live counters, which depend on the search rather than on the trace. `TLCGet("level")` is the level of the state being checked, and `TLCGet("config")` is provided when the run's `-seed` and `-fp` are known (the conformance harness passes the goldens'). Registers follow the trace's own steps;
  - draws of the random operators without the run's `-seed` and `-fp`. With them, draws in `Init` and the next-state relation are replayed from the run's streams. Still refused: a draw in an invariant, constraint, property or label, any draw under a `VIEW` or `SYMMETRY`, an `ASSUME` that draws and fails on a fresh start-up stream, and a step that holds only with its draws taken further into the stream than a predicate reaches them (the successor generator drew for other successors first, ADR 0016 amendment 2).

  A step label that names an operator with parameters is noted and not checked.
- **Configurations embedded in a `.tla` file.** TLC can read a configuration from a `---- CONFIG Name ----` section of a `.tla` file. `dive parse --config X.tla` refuses this as `unsupported`.
- **Binding refusals** (`dive-model`). Binding a specification to its configuration refuses, naming the feature:
  - a constant operator given values in the `.cfg` (`F(1) = 2`), which TLC holds as a table of values (`OpRcdValue`);
  - a Java class next to a module (`Bits.class` next to `Bits.tla`): TLC loads it and uses its methods instead of the module's definitions, and dive cannot run Java;
  - TLC's undocumented `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE` sections;
  - a definition TLC evaluates at start-up that dive's evaluator refuses, when the result decides how the model is bound (it is named in the configuration, it is a conjunct of the behavior specification, or it prints or draws), or when the refused construct splits the next-state relation;
  - the few messages whose text depends on Java objects dive does not have (a Java override used as a configuration name, a state variable used as a conjunct of `SPECIFICATION`).

  Models that check temporal properties, or use `SYMMETRY`, bind, but checking them is refused until liveness checking and symmetry are implemented.

---

Seeded trace validation certifies straight-line assignments behind unprimed guards. Quantified or multi-successor random actions remain unsupported even if their draws happen to match: a finite random-stream search cannot prove placement. The `RndWide` and `RndCoincidence` fixtures exercise these refusals (ADR 0016 amendment 2).

On the pinned oracle at seed 7, `RndCoincidence` reports a safety violation and two states. `RndWide` reports the violation and the initial state, then fails to finish counterexample reconstruction within the capture's 300-second timeout. Its clean capture is recorded as a timeout, not a usable trace or golden. dive produces the two-state violation, but its independent validator explicitly refuses the unproved random position. Evidence: `crates/dive-api/tests/fixtures/random-traces/`.

## Maintaining this page

This page is the narrative companion to the terse registry in [`docs/divergences.md`](divergences.md). The rules for agents and contributors are in `AGENTS.md`. In short:

- Every registry row has a `D-NNN` entry here, added or updated in the same PR.
- IDs are stable and never reused. An entry that is withdrawn stays, marked *withdrawn*, with the reason.
- Every entry includes:
  - a minimal example;
  - what TLC does, measured on the pinned oracle, with numbers;
  - why we think it is wrong or suboptimal;
  - dive's default;
  - the switch that restores TLC's behaviour;
  - the status and the evidence.
- When an entry's status changes (planned → implemented → default), update it in the same PR as the code.
- Write for users who know TLA+ and TLC but not dive's internals. Link to ADRs for the design detail rather than repeating it.

### D-013 candidate: more temporal constructors

TLC accepts the syntax of the following properties, then refuses them during
liveness planning:

```tla
VARIABLE x
Init == x = FALSE
Next == x' = ~x
P == [](IF x' # x THEN <>x ELSE <>~x)
Q == [](CASE TRUE -> <>x [] x -> <>~x [] OTHER -> FALSE)
```

On the pinned oracle, P stops with 2214 (wrong formula format), Q with 2213
(cannot handle formula), both exit 255 before exploration. Pure state guards
and even a temporal guard such as `IF []<>x THEN <>[]x ELSE <>[]~x` do work.
The captures live in `crates/dive-api/tests/fixtures/temporal-conditionals/`.

Dive implements the working IF forms in its bounded final Rust API and
independent validator. It keeps the other forms as named planning refusals in
both profiles. Supporting more constructors could remove an implementation
restriction, but is only a recorded candidate. No new default or compatibility
claim is adopted here.

**D-013 implementation update:** `dive check --lncheck final --lasso short`
now exposes the bounded final backend. Its periodic counterexamples pass both
in-memory and lossless-document validation before return. Schema-2 reports
include the complete false-property set and actual worker width. With
`--compat=tlc`, the report says `tlc+overrides`; it does not claim TLC-exact
lasso choice, temporal error classes, finite-prefix stopping or periodic checks.
Ordinary strict checking remains a named temporal refusal. See
[command-line access](temporal-api.md#command-line-access).

**D-013 supported-language update:** pure constant-argument operators now work
in final/short checking, including nested calls, tuple arguments and calls from
finite temporal quantifiers. The five `temporal-calls` probes match the pinned
oracle's final verdict truth and state counts in both profiles/stores. Each
returned lasso validates independently. Broader lazy/higher-order arguments,
ordinary scheduled timing and strict CLI integration remain pending; bounded final-only pinned selection is available in the separate Rust facade. No new default or
compatibility claim is introduced.

**D-013 validation update:** independent finite ENABLED covers now preserve
constant scalar/tuple arguments through action calls. A pinned 36-state probe
checks three ENABLED identities, and a fair stuttering counterexample requires
disabledness rather than a recurring action certificate. Three additional
`scoped-fairness` probes match final truth/counts in both profiles/stores.
Missing covers remain validation refusals; this expands the validated subset
without claiming TLC-exact temporal selection or error timing.

**D-013 scoped-language update:** final/short checking now accepts pure state
arguments such as `Live(x)` for `Live(p) == []<>p`. Caller scopes remain lazy
through nested calls, guards, tuple values and priming. Seven pinned `Sp*` probes
match final truth/counts in both profiles/stores and every returned periodic
claim validates independently. SANY's rejection of bare eventual actions is
also retained as a negative control. Action/temporal and higher-order arguments
still refuse; this adds supported semantics without a new default or exact
TLC temporal-conformance claim.

**D-013 quantified-action update:** final/short checking now accepts finite
constant-domain predicates such as `\E p \in Processes : Move(p)` inside
fairness. Six pinned probes match final truth/counts in both profiles/stores,
and every returned periodic claim validates independently. A 36-state probe
checks 108 independent ENABLED decisions, including contradictory universal
constraints and an empty existential domain. Missing covers and exhausted budgets
still refuse. This expands supported semantics without a new default or exact
TLC temporal-conformance claim.

**D-013 LET update:** final/short checking now accepts local values and
operators in temporal predicates. Captured state stays lazy, including nested
calls and priming. Six pinned `Lt*` models match final truth/counts in both
profiles/stores, with every returned periodic claim independently validated.
A fair stuttering ending requires independent disabledness. Effects, checker
reads, missing covers and explicit budgets remain named refusals; strict TLC
selection and temporal stopping remain pending.

**D-013 INSTANCE update:** final/short checking follows pure value and variable
substitutions through nested modules without freezing the current state. Six
pinned `Is*` models match final truth/counts in both profiles/stores, with every
periodic claim independently validated. Substituting a state expression for a
constant remains rejected as in SANY. Effectful/checker-dependent bindings,
operator substitutions and unproven substituted disabledness remain named
refusals. This is supported-language coverage, not TLC-exact temporal selection
or stopping. Evidence: `crates/dive-api/tests/fixtures/temporal-substitutions/`.

**D-013 finite-data update:** temporal predicates can use set maps, set filters
and function constructors over finite constant domains. For example:

```tla
P == []<>({IF b THEN x ELSE FALSE : b \in BOOLEAN} = BOOLEAN)
```

With a Boolean variable initially TRUE and a toggling Next action, TLC reports
a violation: infinite stuttering at FALSE fails P. It generates 3 states,
retains 2 distinct states and reaches depth 2. Both dive profiles/stores match
this result and independently validate the returned periodic witness. Local
bindings and captured state remain separate through nested constructors and
function arguments. Empty and bounded state-dependent predicate domains are
supported; effects and explicit resource limits remain named refusals. Evidence:
`crates/dive-api/tests/fixtures/temporal-constructors/`, seven supported pinned
truth/count controls and exhaustive short Boolean words. Exact TLC temporal
selection, stopping and parallel capture remain pending.

### Symbolic function-set enumeration

The pinned oracle prints `[[Nat -> {}] -> {1}]` as `{<<>>}` and succeeds
(`EmptySymbolicFunction`, two generated states, one distinct state, depth 1).
The value layer now recognizes empty function ranges before enumerating their
domains, and recognizes the singleton empty function without enumerating its
range. Size, equality, hashing, fingerprints and printing agree with the finite
representation. A singleton range over a non-enumerable domain still refuses
enumeration; diagnostic printing retains a symbolic fallback instead of
recursively retrying the failed expansion.
Evidence: `crates/dive-value/tests/fixtures/printing/`.

### Independent disabledness with primed domains

A prime outside a quantifier also primes its domain. For example, with
`S = {}` and `t = FALSE`, the action `(\E b \in S : t=b)'` has a concrete
successor `S = {TRUE}`, `t = TRUE`. Enumerating the current empty domain
would miss that successor and could incorrectly validate a deadlock. The
independent validator now refuses successor-dependent primed domains with
`DIVE-T0020`; constant primed domains still require complete finite covers
for all body reads. Ordinary current-state empty domains remain decidable.
This repairs dive's own soundness defect (#94), rather than introducing a
different TLC default. Evidence: `crates/dive-api/tests/primed_covers.rs`,
including concrete witnesses, nested quantifiers and an end-to-end deadlock
claim; the primed-domain regression fails against the old domain scan.

**D-013 independent-constructor update:** finite set maps, filters and function
constructors retain lexical scopes during independent ENABLED checks. Three
pinned fair-stuttering violations and one success match final truth/counts in
both profiles/stores. The eight-state `EnabledConstructorCover` pins 48
independent decisions. Current-state domains are read at that state; primed
domains and missing finite successor covers refuse rather than certify a false
deadlock. Complete TLC temporal stopping, planning/selection and parallel
graph capture remain open.

**D-008 frontend evidence update:** a malformed PlusCal comment can make SANY
reject or crash after TLA+ grammar parsing succeeds. For example, the existing
`PcalEndWithoutBegin.tla` case exits 255 with an array index of -1, while the
pinned root grammar accepts it. `dive analyze` reports `DIVE-E0009` for the
translation-validation problem; `dive parse` measures the TLA+ grammar only.
Deep campaigns now retain both facts separately, with no acceptance inferred
from a generic crash. See `docs/frontend-campaign.md` and issue #98.
**D-013 state-domain update:** final/short predicates may quantify or construct
values over a finite domain that changes with the current state. For example:

```tla
P == []<>({b : b \in IF x THEN {TRUE} ELSE {}} = {TRUE})
```

Starting at FALSE with a toggling Next, pinned TLC reports a liveness violation
(exit 13): infinite stuttering at FALSE leaves the domain empty. It generates
3 states, retains 2 and reaches depth 2. Both dive profiles/stores match truth
and counts; a fresh independent validator certifies the periodic witness.
Planning never picks a sample element or freezes the domain. Concrete evaluation
checks finite-entry bounds at every state and closing edge. Evidence is in
`crates/dive-api/tests/fixtures/temporal-state-domains/` (eight pinned models)
and exhaustive short-word tests. State-dependent expansion of temporal
quantifiers, infinite/oversized domains and effects still refuse by name.
Ordinary strict temporal checking and exact TLC selection/timing remain pending.

**D-013 finite-choice update:** temporal predicates can use bounded CHOOSE,
including definitions such as:

```tla
Min(S) == CHOOSE b \in S : \A c \in S : b <= c
P == []<>(Min(IF x THEN {1,2} ELSE {3,4}) = 1)
```

With x initially FALSE and a toggling Next, pinned TLC reports a liveness
violation (exit 13), generating 3 states, retaining 2 and reaching depth 2.
Both dive profiles/stores match final truth/counts and independently validate
the periodic witness. Each choice here is unique; ambiguous choice keeps the
existing canonical default and TLC restoration switch from D-006. Finite
domains retain actual state context even when a body does not read its binder.
Empty or unsatisfied choices raise errors rather than inventing a value;
infinite/oversized domains, effects and checker reads remain named refusals.
Evidence: `crates/dive-api/tests/fixtures/temporal-choose/`, five pinned models
and exhaustive short-word/context tests. This does not establish strict TLC
temporal selection, scheduling or parallel graph capture.

**D-013 action-helper update:** a pure value helper may inspect the actual
current and next state, including through nested and captured arguments:

```tla
Less(a,b) == a < b
P == []<><<Less(x,x')>>_x
```

For x initially 0 and Next alternating 0 and 1, pinned TLC reports a liveness
violation (exit 13): a stuttering behavior never has a Less step. It generates
3 states, retains 2 and reaches depth 2. Both dive profiles/stores match final
truth/counts and independently validate the periodic claim. Sequence-prefix
helpers such as `IsStrictPrefix(log, log')` use the same retained caller edge.
Evidence: `crates/dive-api/tests/fixtures/temporal-action-args/`, four pinned
models and exhaustive short-word/closing-edge controls. Temporal helpers still
refuse action-level arguments; temporal-valued/higher-order forwarding and
effects remain unsupported. Full TLC temporal selection and timing are pending.

**D-013 fairness storage update:** final/short checking handles five independently
fair processes without materializing every combination of recurring choices:

```tla
Init == x = [p \in 1..5 |-> FALSE]
Flip(p) == x' = [x EXCEPT ![p] = ~@]
Next == \E p \in 1..5 : Flip(p)
Spec == Init /\ [][Next]_x /\ (\A p \in 1..5 : WF_x(Flip(p)))
P == \A p \in 1..5 : []<>(x[p])
```

Pinned TLC finds no violation: 161 generated states, 32 distinct states, depth 6.
Removing the fifth process's fairness violates P (exit 13) at the same counts.
Dive's explicit final/short mode matches both results in both profiles/stores;
the weak-fairness probes fit a 64-node tableau bound that unfactored construction
exceeds. Every reported violation is independently validated. A witness mutated
to stutter forever is rejected specifically for violating fairness.

Recurring conditions are checked on the infinite cycle, including its closing
edge. They do not constrain the finite prefix. Reachable products preserve state,
parallel-edge and witness tie-break order; resource defaults are unchanged.
Evidence: `crates/dive-api/tests/fixtures/fairness-factoring/`, independent
short-word/path-closure controls and the generated LM1 final/short study.
DiningPhilosophers now matches TLC's final truth/counts; MCDistributedReplicatedLog
remains refused for VIEW capture. Actual capture is still one worker. Full TLC
temporal planning, selection and timing remain pending; `--compat=tlc` alone
retains the strict temporal refusal until those are proven.


## Current temporal coverage limitation, 2026-10-10

The [expanded final/short study](temporal-study.md) selects 53 pinned Examples
models: 26 match final truth/counts, 22 refuse features or resource limits, and
five time out without a verdict. Ten refusals involve finite lazy binding
domains. There are no observed disagreements or pending validations, but the
timeouts are findings and the study exits nonzero. This is explicit final/short
mixed-profile evidence; ordinary strict temporal checking remains refused.
TLC-exact temporal classes, phase-L timing and lasso selection, wider scoped
language support, symmetry/VIEW evidence and capture scaling remain open.


**D-013 finite lazy-domain update:** temporal formulas can now quantify over
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


**D-013 captured-filter update:** final/short temporal formulas can quantify
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

**D-013 retained temporal argument update:** pure expression arguments such as
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
invalid redirect target can still panic before these endpoints run (#129).
General caller-created IR/context validation remains a separate partial API
obligation; this increment does not redesign the evaluator constructor.

### Current limitation: runtime names in TLC order (#122)

A model such as `s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)`
observes the order in which runtime strings are created. Speculative evaluation
can change that order even with one requested worker. TLC additionally creates
its configuration strings before assumptions: with `x \in 0..15` and the
invariant in `crates/dive-check/tests/fixtures/runtime-names/DynamicStringOrder.tla`,
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

### Current limitation: independent runtime-name trace validation (#124)

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

TLC-order trace validation now reports **unsupported: runtime string encounter
order** for reachable runtime-name constructors, before predicate evaluation
or lossy trace interpretation. A seed and fingerprint do not supply missing
namespace history. This applies to direct decoded traces, JSON traces and the
Rust API. Canonical default validation accepts the actual `"6"` behavior and
rejects the forged `"7"` step. Ordinary literal strings remain supported.

This is dive's own limitation, not a new language default. Establishing namespace
provenance independently is required before lifting the refusal. The admission
analysis conservatively includes operators with both tuple and string forms.


Finite safety simulation also refuses TLC runtime-name models before sampling
until its simulator startup and independent namespace provenance are proved.
Default canonical sampling remains supported and independently validated. This
is an explicit temporary limitation, not a claimed mode-specific match (#111,
#122, #124).
The same restriction applies to directly supplied lasso formulas and independent
`ENABLED` predicates, even if they are not configured as checked properties.
The configured model flag alone cannot describe an arbitrary Rust caller's
predicate. These APIs preflight the supplied context before computing a result.
Stable checker configuration strings remain supported: a separate forged-prefix
control verifies that the seed is interned before the fingerprint, independently
of the order in which a trace mentions them.

### Current limitation: bounded parallel temporal capture

**D-013 parallel-capture update:** explicit final/short checking now honors the
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
| **Status** | Native pure safety slice implemented; full mode/output acceptance remains pending (ADR 0021) |

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
transcripts are under `crates/dive-check/tests/fixtures/dfid/`; no RNG or trace
instrumentation changes the oracle in these controls.

**D-002 replay extension:** even with seed 7, aril 3 and fingerprint index 0,
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
a concrete witness bug, separate from DFID's unseeded worker choices (D-002).

**Dive.** Native DFID keeps the actual concrete path, returns `1,2`, and accepts
the violation only after independent validation. A printed-witness restoration
mode is explicitly unsupported in this slice; `--compat=tlc` retains the
verified search/counting contract but does not label a malformed word valid.
Reports state this witness policy. No TLC-matching trace claim is made here.

**Status and evidence.** Source and primary raw observations are in
`crates/dive-check/tests/fixtures/dfid/`; `oracle-controls.json` records the clean
selection. Separately labelled seeded-worker reference controls reproduce both
root choices. This correction follows the soundness-first contract; a broken
witness never certifies a violation.

## Current PlusCal limitation, 2026-10-10

Native `dive-pluscal` now parses current C/P source into typed, source-mapped
ASTs. This is the first translation stage, not a translated specification:
a separate bounded sequential path now normalizes explicit labels/control flow
and emits validated enclosing modules. Macro expansion, processes/procedures,
branch-internal labelled expansion, unproved name disambiguation, multiline/binding
expression substitution and historical translator options/checksums remain named
refusals. Bounded sequential automatic labels, default model-value initialization
and `L:+` / `L:-` are now matched separately; see
[the normalization contract](pluscal-normalization.md). See
[the translation contract](pluscal-translation.md).

The [single-process contract](pluscal-processes.md) now admits unfair literal-ID
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


### Translator success does not ensure a valid TLA module (candidate)

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

**D-008 public Rust context update:** a caller-created invalid evaluator redirect
previously panicked after interning a literal, while malformed lazy expression
IDs could enter captured scopes. Constructor and scope admission now produce
stable `DIVE-EVAL-C001`–`C007` refusals before evaluation or interning. Independent
trace/predicate APIs also reject foreign model/evaluator metadata before replay
setup. Valid configured models keep TLC's existing startup diagnostics. The
checks apply equally to both profiles; no compatibility flag disables them.
Original native observations and source are retained in
`crates/dive-api/tests/fixtures/public-context/`; ADR 0028 documents migration and
remaining limits. Bare numeric IDs do not carry provenance, and opaque deferred
Rust bindings refuse rather than call user code to establish their shape.
F-035 remains partial; this is not a full Rust API or IR certification.


Temporal predicate integration (#149) fixes a native limitation: TLC's liveness evaluator skips an action body on a subscript stutter, while ordinary action evaluation checks the body first. Native shared temporal atoms and independent angle/box/fairness-occurrence replay now follow the liveness order. The minimal division-by-zero example and all 36 direct pinned controls are in `conformance/probes/temporal-action-order/README.md`. Broader conversion error precedence and strict liveness integration remain pending; this is a TLC-matching fix, not a new default divergence.

### D-013 implementation progress: concrete liveness successors

ADR 0029 adds real concrete successor collection with internally derived TLC
fingerprints, first representatives, grow-before-duplicate slot order and one
logical multiplier history. Null-tableau OOS action bins precede state bins;
initial tableau consistency is separately projected. Actual diagnostic execution
is inline (one worker); wider requests are refused honestly. Every receipt says
`graph_insertion=false`, and no receipt is proof evidence.

The complete `conformance/probes/tlc-phase-l/` selections retain 55 structural
controls, 35 actual synthetic LiveCheck controls, 51 bound model rows and 20 clean
TLC runs. Completed field/error projections, invalid syntax, effects/introspection/
state-IF refusals and tableau graph-pending rows are accounted separately. The
prior planning IF mismatch (#147) and body-first nonstuttering mismatch (#149)
remain available; conservative diagnostic refusal is not a match. Independent
trace controls reject forged Next/closing edges and claims. Previous corpus
reports are immutable and receive no new verdict credit.

F-020/F-021/F-022 remain partial/audit-required and F-023 missing. #140 stays
open: graph/product insertion, actual parallel commits, additional consistency/
error ordering, SCC/lasso selection and scheduling must still be implemented.

### D-013 implementation progress: actual graph commits

The bounded graph stage now keeps original generated successor values even when
an equal state was already stored, then writes concrete graph nodes with TLC's
slot order, bin labels and one logical capacity history. Source expansion really
uses 1, 3, 4 or 8 workers in the tested runs; ordered predicate/graph commits
report their own single-worker execution. Twelve completed pinned graph
projections match their defined fields. All fifteen selected models, errors,
tableau refusal, clean TLC outcomes and original failed adapter attempts remain
available in `conformance/probes/tlc-phase-l-graph/`.

This does not enable strict liveness yet. Complete safety exploration currently
precedes temporal evaluation, so its counts are not TLC's temporal stopping
prefix. Multiple OOS, dynamic tableau revisitation and done-state re-enumeration,
periodic checking and exact SCC/lasso selection are still required. Opaque
captured providers are refused before execution; per-source speculative quotas
can reject a skewed level conservatively. Independent validation still decides
a concrete periodic word and its claims, rather than trusting graph metadata.
No graph result or diagnostic receipt claims a model proof.

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

The [bounded process-fairness increment](pluscal-fairness.md) adds weak/strong
declarations, label guards and explicit pinned fairness options. Translation
artifacts remain nonproof results. Unproved/overlapping process domains, procedure fairness, renaming
and unproved domain normalization are still named refusals; full-family
certification remains pending.

**D-008 PlusCal fairness formatter observation:** the pinned translator prints
an **87-column** fairness line with its default **78-column** setting for:

```tla
--algorithm A {
  variables g=0;
  fair process (P \in{1,2}) variables x=0;
  { L: + x:=1; E: - g:=x; }
}
```

`ProcessFairness.singleLineWidth` omits the sole quantifier-prefix line and
conjunction separator. The retained `FairPositive_C_set_weak_both` primary output
shows the 87-column line. Current native translation preserves its measured
junction grouping in both profiles; full width/layout improvement is pending.
No alternate formatting default is silently adopted. Long-label controls also
compare full operator CST, since moving TLA junction indentation can change
expression structure. See [the fairness evidence](pluscal-fairness.md).

**D-008 public backend admission update (#157):** malformed public Rust IR
metadata now refuses before ordinary checking or checkpoint request handling.
Graph capture likewise checks arena/name and predicate/action ownership before
model effects. Error reports omit an invalid variable mapping instead of
panicking while reporting another refusal. Original panic source/hash evidence
and native controls are retained in `conformance/probes/public-ir-admission/`.
This is a native API safety repair, not new TLC semantic certification.

The [multiple finite-process increment](pluscal-multiple-processes.md) now binds
each admitted block to its own storage, labels and fairness context. Overlapping
IDs, name rewrites and cross-process local references remain explicit refusals;
translation artifacts still make no model-proof claim.

The [bounded native macro increment](pluscal-macros.md) reproduces lexical
substitution rather than inventing capture avoidance. The pinned translator can
return zero while producing TLA+ that SANY rejects (including `Set(b, 3)` whose
actual `b` is undeclared). Native raw generation retains those comparisons only
as diagnostic evidence; public translation rejects the invalid enclosing module
in both profiles and makes no proof claim (D-008). General binder/procedure/name
rewrites remain named refusals. Source PlusCal strings deliberately preserve the
pinned printable-ASCII boundary; no wider Unicode-string default is adopted.



The bounded sequential PlusCal procedure path now generates saved parameter/local
frames and call/return continuations. Its 84-row selection keeps four primary
SANY-invalid modules uncredited. Thirty generated invariant witnesses reproduce
TLC exploration but strict independent replay refuses runtime string encounter
order (#124); default/canonical replay is separately qualified evidence, not
TLC-profile certification. See [the procedure guide](pluscal-procedures.md).

The earlier nonrecursive PlusCal graph study admitted multiple disjoint procedures
and nonrecursive nested calls. Procedure fallthrough still generates `pc="Error"`
and may deadlock, matching the reference. That study refused cycles (including unused ones), name
rewriting, process-owned stacks and unproved procedure-label/macro interactions.
The current recursive path is described below; its original study is unchanged. Strict stack witness replay remains pending under

The bounded sequential PlusCal graph path now admits multiple disjoint procedures
and nonrecursive nested calls. Procedure fallthrough still generates `pc="Error"`
and may deadlock, matching the reference. Cycles (including unused ones), name
rewriting, process-owned stacks and unproved procedure-label/macro interactions
remain honest named refusals. Strict stack witness replay remains pending under
#124; separately qualified canonical replay confers no TLC-profile certification.
See [procedure graph use and limits](pluscal-procedure-graphs.md).

### Recursive zero-parameter tail call: invalid output (D-008 candidate)

The pinned translator reports success for this PlusCal source:

```tla
--algorithm A {
  variable x = 0;
  procedure First() { F: call First(); return; };
  { L: call First(); E: skip; }
}
```

Its generated `F` action contains a blank conjunction between `pc="F"` and
`pc'="F"`. `pcal.trans` exits 0 and writes the module; SANY rejects the next
`/\` with a parse error. The retained raw source and both stages are
`crates/dive-pluscal/recursive-procedure-acceptance/fixtures/Recursive_C_self_tail_no_params.*`.
The reference's same-procedure call/return path adds an empty assignment when
there are no parameters; this is not a valid translated model or a termination
proof. The native recursive increment retains this as diagnostic evidence
and public admission rejects invalid modules in both profiles. A repaired-output
option is not adopted silently. Full translation parity remains pending.

The disjoint sequential procedure path now admits ordinary self/mutual recursion
and reference same-procedure tail-call normalization without unrolling calls.
See [recursive procedure use and limits](pluscal-recursive-procedures.md).
Strict stack-witness namespace proof remains pending; bounded unbounded-model
stops never claim exhaustive search or termination. Process/scoped name handling
and full translator parity remain pending.

### D-013 progress: ordered temporal execution

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

### D-013 progress: pinned final-only SCC and witness selection

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

### Temporal IF identity repair (dive limitation, #170)

Pure state conditionals are available in the bounded final-only path. Temporal
conditionals still refuse there: the source-level shared-guard tag repair now
matches selected exact normalized/tableau fields, but execution and witness
admission remain staged. See [the maintained repair note](tlc-temporal-if.md).
No new result-changing default or ordinary periodic parity is claimed.

The proven direct temporal-IF subset now runs in explicit final/TLC mode after
shared-object tag repair, exact particle/edge checks and fresh independent
in-memory/JSON witness replay. The unchanged full selected controls retain
upstream 2213/2214 errors and source-prefix 2116/exit-12 cases as named refusals,
without matching-error credit. Compound guards and INSTANCE claim/role identity
are separately checked. Existing arena/work ceilings and whole-arena forwarding
admission remain; no aggregate memory bound or periodic-family claim is added.
See [the user guide](tlc-temporal-if.md) and
`conformance/probes/tlc-temporal-if-final/`.


**PlusCal process procedure status (D-008):** one unfair finite process block
can now call disjoint procedures in both syntaxes, including recursive and
self-tail transfers. Slots and stack are owned by the process ID, while explicit
source `stack` expressions remain as written by the reference. Invalid upstream
artifacts, including empty self-tail conjunctions, are still rejected in both
profiles. Multiple blocks with procedures, scoped rewriting/fairness and strict
runtime-namespace witnesses remain named limitations; canonical witnesses do
not certify TLC replay. See [process procedure use](pluscal-process-procedures.md).

### Procedure names can collide with generated ProcSet (D-008 candidate)

This minimal C-style PlusCal body uses a parameter named `ProcSet`:

```tla
--algorithm A {
  variable x = 0;
  procedure First(ProcSet = 0) { F: x := ProcSet; R: return; };
  process (Worker = 1) { L: call First(2); E: skip; }
}
```

The pinned translator exits **0**, declaring variable `ProcSet` and also emitting
operator `ProcSet == {1}`. Clean SANY with `-error-codes` exits **4**, reporting
the multiply-defined symbol. Naming the procedure itself `ProcSet` similarly
produces a zero-exit translation with conflicting zero/one-argument operators.
Both C/P versions are retained: four invalid primary artifacts in
`crates/dive-pluscal/process-procedure-acceptance/names/primary`.

Successful generation is not a usable artifact. Public native translation in
both profiles rejects the enclosing module and claims neither acceptance nor
proof. These rows earn diagnostic credit only; no automatic rename, corrected
output option or new default has been adopted. Status: measured upstream
artifact-generation defect, with source/raw/SANY and CLI/API rejection controls.

**Later shared process procedure status (D-008):** multiple disjoint finite
process blocks can now call the same procedures without sharing their frames.
Slots and stack initialize once over all IDs; procedure actions execute for the
calling ID. This lifts the earlier single-block limitation, while scoped
renaming, cross-process locals and process-declared fairness remain refused.
Both profiles reject invalid upstream artifacts; canonical witness checks
still do not certify strict namespace replay. See
[shared procedure use](pluscal-multiple-procedures.md) for executable C/P and
Rust examples and the exact supported/refused contract.

### Procedure local self in a constant-ID caller (D-008 candidate)

```tla
--algorithm A {
  variable x = 0;
  procedure First(a = 0) variable u = self; { F: x := u; R: return; };
  process (One = 1) { L: call First(1); E: skip; }
  process (Two = 2) { M: call First(2); N: skip; }
}
```

Pinned translation exits **0**. Its Init binds `self` when initializing the
procedure slot, but the constant-ID call actions initialize `u` with a free
`self` instead of the caller's literal ID. Clean SANY exits **4** with two
unknown-operator errors. Both C/P source/raw artifacts remain retained as
`MultipleProcedure_*_local_self` in the shared procedure study. Native public
admission rejects the enclosing artifacts in both profiles. No corrected
initialization default or strict artifact credit has been adopted.

The P-syntax empty-process control also retains a zero-exit translation with
an empty process operator and a `"null"` initial program-counter label; SANY
rejects the generated module. Native normalization refuses the empty body by
name. These observations explain why successful translation is not enough for
an admitted usable module; they are not silently repaired.

**D-008 process-owned fairness update:** finite, disjoint process callers now
receive WF/SF clauses for their reachable procedures in the pinned call order.
`Entry: +` adds a strong-fairness clause under a weak caller; `Entry: -` guards
that procedure's caller-owned pc location. Original `Entry+:` controls remain
source grammar rejections; the corrected syntax was captured separately before
implementation. `-nof` suppresses all fairness for process algorithms. Native
formula construction and reachability have explicit bounds, returning a resource
failure rather than silently dropping a clause. Generation does not prove
liveness or remove strict #124 namespace replay restrictions. See
[fair procedure use](pluscal-procedure-fairness.md) for runnable C/P and Rust
examples and the complete supported/refused contract.


## D-020: fair-algorithm checksums do not detect source edits

This minimal source has the same algorithm checksum after changing `x=0` to `x=9`:

```tla
---- MODULE FairCRC ----
EXTENDS Integers
(* --fair algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)
====
```

The pinned translator (commit `94d0c50`) emits pcal checksum **`7c28162a`** for
initial values 0, 1 and 9 in both C and P syntax. Their TLA-body CRCs are respectively
**`64f9cea5`, `7faa8ba4`, `a530a3ac`**. Editing just the algorithm initializer in
the original translated file from 0 to 9 still gives **`NO_DIVERGENCE`** in all
four original/edited C/P validator controls. This is a missed source edit, not
an admitted algorithm-equivalence claim: `trans.performTranslation` and
`Validator` hash `"fair"` alone because of Java ternary-expression precedence.

**Status: measured upstream candidate; no native correction adopted.** Native
block rewriting derives the reference CRC for its proved bounded sequential
post-translation AST subset, including this weakness, flagged as
`fair_header_crc_only=true`. `algorithm_checksum_verified` and `algorithm_matched`
mean reference CRC derivation/equality, not source identity or proof. Other AST
shapes remain named full-checksum refusals. Explicit `--checksums tla` disables
algorithm CRC and verifies only the generated body; that partial policy is
reported as an override under `--compat=tlc`. A future corrected option would require
a named TLC counterpart and its own approved default decision.

All six primary sources/artifacts and four unchanged-body source-edit controls
remain in `crates/dive-pluscal/rewrite-acceptance/fair-checksum/`. Body CRC and
usable artifact admission are separate contracts (D-008). See
[block rewriting](pluscal-rewriting.md) for executable CLI/Rust usage and limits.

Bounded final scope evidence (#134) now distinguishes configured names from
local/formal/INSTANCE derived roles; see [the scoped final guide](tlc-final-scopes.md).
Exported INSTANCE temporal-formal and opaque/effectful combinations remain named
refusals. The additive property-origin increment below exposes original
expanded Action names separately; its metadata is not a new result-changing
default or ordinary liveness certificate. Ordinary periodic scheduling remains
unsupported; the whole small-model study from the prior IF slice is unchanged.


**D-008 checksum continuation:** nested `elsif` and multiple `with` binders now
have bounded reference AST serialization. Strict operator-tree comparisons
caught dive omitting the pinned empty-`else` TRUE conjunct; the original mismatch
and paired empty/explicit-Skip controls remain visible. Native now preserves
that conjunct. Raw multiline checksum matching remains separate from public
translation, which explicitly refuses unproved multiline layout and empty
`then` generation. CRC agreement never means the model was proved or that an
invalid generated artifact is admitted. See
[the scope and executable examples](pluscal-checksum-expressions.md). D-020
continues to expose the fair-header-only checksum weakness without correcting
it by default.

### Current identity metadata limitation and evidence

The final-only JSON and Rust API expose three distinct temporal identity vectors:
configured properties, original expanded Actions and derived role labels. A
conjunction can have one configured `P`, two Action names `P`, and two source
location labels. Those labels are not interchangeable. This is a matching
metadata contract, not a new divergence or ordinary liveness certification.
Exported temporal-formal INSTANCE cases still refuse when the pinned reference
fails before producing that projection. See [the identity guide](tlc-property-origins.md).

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
selection remains evidence; see [the concrete capture guide](material-capture.md).
Ordinary periodic checking and full family certification remain unfinished.


**D-008 finite-process Full reports.** Declared-fairness finite process records
now have proved native post-translation AST/CRC derivation, separately checked
from admitted generated modules. Raw multiline process checksum matching does
not lift the generator's layout refusal. Supported explicit process fairness and
automatic labels have the separate [option contract](pluscal-process-checksum-options.md);
source-option declarations, name rewrites and procedure/macro ASTs remain named Full refusals. Both profiles
report nonproof editing results. See [process checksum examples](pluscal-process-checksums.md).
D-020's fair-header-only source detection weakness remains explicit and unchanged.

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
parity is unclaimed; see [the operator guide](temporal-operators.md).

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
For proved finite process shapes, dive derives Full CRCs from the exact explicit
CLI/API options and reports `translation_options` on every replacement request.
The API has one fairness enum; the CLI rejects repeated/conflicting fairness
flags before loading. Native generation and Full derivation admit only the proved source fairness/label
subset documented in [source options](pluscal-source-options.md); unknown,
malformed, conflicting and other unproved source contexts refuse by name. Store/replay explicit settings with the
artifact and configure reference validation consistently. An unconfigured
Validator result never proves the source was edited.

The compatibility counterpart is the same pinned CRC/options context under
`--compat=tlc`, not a new checksum scheme. `--checksums tla` is the explicit
partial body-only policy. The unchanged `--fair algorithm` weakness (D-020)
can also hide changes, and its `NO_DIVERGENCE` result does not certify identity.
Raw AST/CRC agreement, usable artifact admission, finite checking and independent
witness validation stay separate (D-008).

See [runnable option examples and limits](pluscal-process-checksum-options.md)
and `crates/dive-pluscal/process-option-checksum-acceptance/validator-context/`
for all primary/configured/unconfigured/native-before-guard evidence. Broader
source-option parsing, scoped serialization, exact formatting and file writes
remain pending rather than inferred from this bounded increment.

D-021 original-source control: a declaration inside a previous translation
block is still read by the pinned translator. The retained native producer
masked it first and admitted both Full and body-only output without requested
weak fairness. High-level native rewriting now refuses before masking; the
minimal source and both failed outputs are in
`conformance/probes/pluscal-source-option-masking/`. Issue #182 remains open
for broader source-option parsing and precedence; the bounded proved subset
is documented in [source options](pluscal-source-options.md). No stronger
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
never proves model correctness or a usable artifact (D-008).
