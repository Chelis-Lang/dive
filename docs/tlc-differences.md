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
- counterexamples come from the stored trace, not from evaluating `Init` again, so they cannot be lost.

The operators choose with TLC's algorithms (which element, which subset); only the source of randomness differs. A dive run that cannot be reproduced from its reported inputs is a bug.

Under `--compat=tlc` dive reproduces TLC's choices exactly for a given `-seed` and `-fp`: Java's `java.util.Random`, TLC's per-state seeding, and TLC's algorithm for each operator. Draws in `Next` also need TLC's own state fingerprints, which depend on TLC's variable order (ADR 0014), and the evaluator must re-evaluate `LET`s exactly where TLC does. Until both are built, dive refuses a spec that draws in `Next` under `--compat=tlc`, naming the feature. Draws in constants, `ASSUME`s and `Init` need neither.

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
| **Default** | Seed reported; `--seed S` gives bit-identical simulation at any worker count |
| **TLC behaviour** | Under `--compat=tlc` |
| **Status** | the random streams are implemented in `dive-stdlib`: TLC's worker seeding, and dive's per-trace streams. Simulation itself is planned (`dive-sim`). |

**What TLC does.** Even with `-seed 0` and one worker, the number of states TLC generates in simulation varies between runs. On Examples `EWD840_anim` it ranged from about 267k to 271k.

From TLC's source, its simulation uses two kinds of generator, and neither is tied to the trace being generated:
- each worker chooses initial states, actions and successors with its own generator. That generator's seed is the next value of a generator seeded with `-seed` and advanced by `-aril`;
- each worker evaluates `RandomElement` and the `Randomization` operators with a generator seeded with `-seed` itself, and never reseeded, so every worker draws the same sequence of values. What a trace draws depends on which worker generated it and on what that worker generated before.

**What dive does.** The same seed gives the same simulation, at any worker count. Each trace has one random stream, keyed by the seed and the trace number. It drives both the choice of states and actions and the operators' draws, so a trace is the same whichever worker generates it. This makes simulation usable in tests and in CI.

**Evidence.** Harness notes in `crates/dive-conformance/README.md` and the registry in `docs/divergences.md`. TLC's worker seeds are reproduced bit for bit (`crates/dive-stdlib/tests/java_random.rs`); the independence of trace streams is tested in `tests/dive_mode.rs`. TLC's simulation draws are read from its source (`Simulator.java`, `SimulationWorker.java`, `RandomEnumerableValues.java`) and not yet measured.

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
| **Status** | implemented: `dive check --store exact` runs the same search over canonical state encodings. It gives the same results as fingerprint mode on the TLC probes and harness fixtures (`crates/dive-check/tests/probes.rs`). Disk spill is planned. |

**What TLC does.** TLC stores 64-bit fingerprints of states, not the states themselves. A collision between two distinct states would silently skip part of the state space. TLC prints a probabilistic estimate of this at the end of a run, but the "no error found" verdict itself is probabilistic.

**What dive does.** dive offers an exact mode that stores full states and makes an unconditional exhaustiveness claim. Every result says which mode produced it.

**Evidence.** Charter, soundness contract rule 4.

---

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
- **In the default profile, membership.** Membership tests are a binary search in the run's order. If the set mixes values TLC cannot compare with the element being tested, the default profile can meet a different pair from TLC. It may then raise TLC's error where TLC answers, or answer where TLC raises one. This corner needs a set that TLC can build but whose elements are not mutually comparable, such as tuples mixed with records of other sizes.

**What we propose.** An option that answers these as TLA+ defines them, symmetrically and independently of element order and of how a function was built. It would keep the error where TLA+ leaves the answer unspecified (`1 = "a"`, `{1} = 1`), because an error is the sound response there. It changes results: specs that TLC stops with an evaluation error would get a verdict. So it awaits the owner's decision. It would also remove the membership corner above.

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
and independently checked; omitted or extra claims are rejected. Capture runs
inline and reports that width. TLC-exact planning/selection, phase-L error
accounting, parallel/periodic capture and CLI options remain pending. Ordinary
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

## D-014: Periodic liveness checking has a replayable schedule

**Status:** planned, owner-approved default. `--lncheck=deterministic` checks at the documented level boundaries in ADR 0018. `--compat=tlc` retains TLC's wall-clock scheduling for `--lncheck=default`; `--lncheck=final` checks the complete graph and remains the golden setting.

**Example and measurement.** The minimal `LvPeriodic` probe stopped at 418,795, 167,402 and 162,021 distinct states in three pinned-TLC runs. All found the same violation, but the stopping counts depended on time. Fixed level boundaries make those counts reproducible across workers. They are a named scheduling difference, not a claim of exact TLC scheduling. Evidence: ADR 0017 §6 and the recorded probe transcripts.

## D-015: Symmetry functions must actually be permutations

**Status:** planned, owner-approved rejection in the default profile. A declaration containing `[a |-> b, b |-> b]` is not bijective. TLC accepts it and closes the set under composition, permitting an unsound reduction. The default profile will name and reject the offending function. `--compat=tlc` retains TLC's acceptance. Evidence: ADR 0019 §1 and its pinned-oracle non-bijective probe; the numerical state-space comparison is to be recorded when this option is implemented.

## D-016: The declared symmetry can be audited

**Status:** planned, opt-in. The `SymAsym` and `SymAsymLate` probes declare symmetry that the transition relation does not respect. TLC trusts the declaration, so the reduced search can hide behaviors. `--check-symmetry` will check initial states, successors, invariants, constraints and VIEW under the declared generators and report the first counterexample to the declaration. Ordinary runs, including `--compat=tlc`, omit this expensive audit. Evidence: ADR 0019 §6 and the probe transcripts; implementation measurements are pending.

## D-017: Host access requires explicit grants

**Status:** planned; host-access operators currently refuse by name. `IOEnv`, `IOExec`, `CSVWrite` and serialization operators can read the host or execute effects while a spec runs. For example, Examples `TLC/TestGraphs` reads `IOEnv` at startup and `ewd998/EWD998ChanTrace` reads it in its next-state relation. These dependencies cannot be replayed from the random seed alone.

The approved interface is `--allow-io=env,files,exec`, with no categories granted by default. Environment and file access come first; process execution stays deferred. Even `--compat=tlc` requires explicit grants and reports restricted compatibility when relevant operators are refused. Granted access and external inputs are recorded in the run. Evidence: the current named refusals and the 22 host-access assumptions in CommunityModules `AllTests`; no granted-access measurements exist yet.

---

## Under consideration

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
  - temporal checking still refuses at the CLI; the pure final library coordinator currently captures inline and does not provide TLC-exact selection or a complete attributed report;
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
phase-L timing and TLC-exact selection remain pending; no new default or
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
