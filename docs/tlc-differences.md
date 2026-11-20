# Where dive differs from TLC, and why

dive is checked continuously against the pinned Java tools (SANY and TLC). Each
entry below is a place where dive's default gives a different answer from TLC,
or the same answer by a different path, with a minimal example, what TLC does,
what dive does and how to get TLC's behavior.

`--compat=tlc` (or `DIVE_COMPAT=tlc`) selects TLC's behavior where dive
implements it. The [compat gaps](#compat-gaps) list where it does not;
unsupported features refuse by name. Each default difference has a stable
`D-NNN` entry. The terse [registry](divergences.md) holds the evidence and
tests for every entry.

## Compat gaps

These are the known places where `dive check Spec.tla --compat=tlc` does not
reproduce TLC:

- TLC's construction and normalization history: construction errors,
  `RvMemNorm` replay and Next-side lazy-function fingerprint wrappers.
- Function-set diagnostics can name incomparable operands in a different order
  after canonical function construction, for example on an enumerated domain.
- Random operators need a supported draw placement and a replayable stream;
  dive does not match TLC's draw order in general, including with VIEW.
- Seeded simulation does not reproduce TLC's choice stream: for a given seed
  and aril, dive samples different traces and reports a different generated
  count than TLC (31 against 29 for `SimChoice` at seed 0, aril 0). Every
  sampled trace is still validated independently.
- Temporal checking uses dive's own schedule and validated counterexamples;
  TLC's `-lncheck off` and `-lncheck final` behavior is not reproduced.
- PlusCal rejects a call followed by a goto to an unknown label, which TLC's
  translator accepts (`CallGoto`).
- Multi-worker results equal the one-worker result; TLC's multi-worker queue
  order is not reproduced (Hanoi with `Bits` counts and depth).
- These refuse by name: bounded final temporal checking
  (`--lncheck final --lasso short`, and `dive tlc -lncheck final`);
  multi-worker liveness whose violated names are known only at run time;
  finite simulation and DFID that reach random or register effects dive does
  not replay; source and TLC logical coverage (`--source-coverage`,
  `--coverage 0`, `dive tlc -coverage 0`); external inputs (`--allow-io`); and
  recovery of a checkpoint archive in an older pre-release format.
- Boolean recursive predicate guards keep their
  [D-029](#d-029-predicate-witness-counts) successor counts under
  `--compat=tlc`. `byzpaxos/VoteProof` succeeds with TLC's 6962 distinct
  states and depth 14 but generates 27727 states instead of TLC's 318877.

None of these gaps can produce a false verdict or weaken counterexample
validation. Where an entry below says dive matches TLC exactly, this list
still applies.

**Status values:**
- **not implemented:** decided, not built; the option is refused.
- **implemented:** built and tested in both profiles.
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
| **TLC behavior** | `--workers 1` reproduces TLC's exact breadth-first results |
| **Status** | implemented (`dive check --workers N`). Every result is identical at every worker count and under randomized scheduling, and equals TLC's `-workers 1` result under `--compat=tlc`. Specs that depend on TLC's sequential side effects run in sequential mode and say so. |

[Distributed safety workers](user/distributed-safety.md) extend the same
ordered commit to trusted worker processes. Distribution is off by default and
in the TLC profile. It keeps results through complete-source retry; recovery
of a distributed run, temporal, storage and effect combinations are not
supported.

**What TLC does.** With more than one worker, TLC's breadth-first search is only approximately breadth-first (`tlc2/tool/TLCTrace.java`; tlaplus#883). Three things change from run to run and with the worker count:

- the number of states found before a violation;
- the reported search depth;
- the counterexample, which may not be the shortest.

Measured on the pinned TLC, on one spec with a safety violation:

| Workers | States explored when the violation was found | Counterexample length |
|---|---|---|
| 1 | 63,251 | 46 |
| 8 (three runs) | 64,416; 67,497; 65,409 | 46 |

On Examples `DieHard` with 4 workers, TLC reports depth 9 for a 7-state counterexample. The pinned TLC also has an upstream multi-worker race (tlaplus#1445) that can produce bogus states. So the only TLC results you can fully trust are single-worker ones, which are slow.

**What dive does.** Any worker count gives the same verdict, the same state counts, the same depth and the same shortest counterexample as TLC's single-worker run, at the speed of parallel checking.

dive explores one breadth-first level at a time. Workers expand states in parallel, but every successor carries its position in TLC's single-worker order, and when two workers find the same new state the one with the earlier position wins. The first violation, deadlock or error in that order stops the search, and the counts are computed for that point. `Print` output is buffered and released in TLC's order too.

**Sequential mode.** A few specs observe TLC's evaluation order directly, so no parallel search can reproduce them: they write TLC registers (`TLCSet`), read live counters (`TLCGet("distinct")` and friends), draw random values (TLC draws from one stream per expanded state), perform I/O, or have side effects inside lazily evaluated sets or functions. dive detects these from the source and checks them with one worker, in TLC's order. The header reads `4 worker(s) requested, 1 used` followed by the reason, and the JSON `options.workers` block has `"sequential": true` and the reason. `--sequential` forces this mode for any spec; results never change.

**Measured.** On the Examples small tier at 1, 3, 4 and 8 workers:
- **Default profile.** Every checked model gives the same results at every worker count.
- **Byte-identical JSON.** For the 60 agreeing models that run in under a minute, the JSON report (timing and thread fields aside) is byte-identical at 1, 3, 4 and 8 workers and under chaos scheduling, in both profiles.
- **`--compat=tlc`.** 70 models agree with TLC's single-worker result at every worker count. The one exception is by design: `DieHard/MCDieHardest` asserts `TLCGet("config").worker = 1`. Under `--compat=tlc` dive reports the physical worker count as TLC does ([D-011](#d-011-tlcgetconfigworker-reports-the-logical-worker-count)), so at `--workers 4` the assumption fails exactly as it does in TLC at `-workers 4`. In the default profile it reports 1 and the model agrees at every worker count.

---

## D-002: Randomness is always replayable

| | |
|---|---|
| **Kind** | Same answer, better path (reporting); different answer only when you fix a seed |
| **Default** | Fresh seed per run, as in TLC, but **the seed is always reported**; `--seed S` alone replays a run exactly, at any worker count |
| **TLC behavior** | TLC's seeding and TLC's draws, bit for bit, under `--compat=tlc` (given TLC's `-seed S` and `-fp N`) |
| **Status** | implemented in both profiles. `dive check` accepts `--seed`, always reports the seed and the fingerprint polynomial, and draws `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` from the run's streams: a start-up stream for constant definitions, `ASSUME`s and `Init`, and a stream per expanded state. `RandomSubsetSet` refuses by name. The trace validator refuses every random draw made by a check, because the seed and polynomial do not fix a draw's stream position independently of successor generation. |

**What TLC does.** `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` draw from an unseeded random source during model checking unless you pass `-seed`. A spec that uses them can produce a different state space on every run.

| Spec | Measured on the pinned TLC |
|---|---|
| Examples `SpanningTree/SpanTreeRandom` | 1,878 / 40,460 / 65,396 / 106,664 / 240,806 distinct states across runs with identical flags |
| `RandomElement(1..1000000)` in `Init` | 599,378, then 344,064, then 43,074. With `-seed 7`: 730,700 three times |

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
- counterexamples are rebuilt by ordered replay from a fresh start-up stream. When draws before `Init` make that impossible, dive reports the violation without a trace, as TLC does, says why, and never invents a replacement.

The operators choose with TLC's algorithms (which element, which subset); only the source of randomness differs. A dive run that cannot be reproduced from its reported inputs is a bug.

Under `--compat=tlc`, ordinary checking reproduces TLC's choices for a fixed `-seed` and `-fp`: Java's `java.util.Random`, TLC's per-state seeding and each operator's algorithm. Next-state draws follow TLC's variable order, and `LET` expressions are evaluated where TLC evaluates them. This covers the draws, not TLC's trace printing at every random stopping point. Randomness during checking runs inline, and the report gives the worker count used.

**Randomness with VIEW or SYMMETRY** refuses before binding, output or search, including unused source calls and configured native aliases. Pure VIEW without randomness and per-state randomness without VIEW work. Randomness during graph capture, temporal checking and checkpoint recovery also refuses.

**Start-up-fixed randomness with temporal properties.** A zero-arity
constant-level definition such as SpanTreeRandom's
`Edges == UNION {{{n, m} : m \in RandomElement(SUBSET (Nodes \ {n}))} : n \in Nodes}`
is evaluated once at start-up, from the reported seed, and its value is kept for
the whole run, as TLC keeps it, so dive checks temporal properties on that
fixed graph. A draw while checking, including through an operator with
parameters, refuses. So does a draw in a definition that a configured
`Op <- D` or `C <- D` targets, because TLC evaluates `D`'s body at every use of
`Op` or `C`; and so does a definition whose start-up value keeps a lazy set,
such as `S == {n \in Nat : n = RandomElement(0..1)}`, because each membership
test draws again. Under `--compat=tlc` the draws are TLC's:
`SpanningTree/SpanTreeRandom` at `-seed 0` gives TLC's 79,550 distinct and
1,357,252 generated states. The default profile draws from dive's own streams,
so its counts differ (22,040 distinct and 293,361 generated at seed 0, a
registered difference). A temporal violation of such a model refuses, because
the validator does not replay start-up draws.

**Configured random calls.** This model selects a native operator through its
configuration rather than a source call:

```tla
CONSTANT C(_)
VARIABLES n, x
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == x # 80
```

With `CONSTANT C <- RandomElement`, TLC at `-seed 7 -fp 0 -workers 1` reports an
invariant violation after two states, and so does dive: a configured
`RandomElement` uses the same start-up and per-state streams as a direct call
and reports an inline worker count. Configured `Randomization` targets without
VIEW work the same way. Transformed, graph and recovery random paths refuse.

TLC also shows a replay limitation on this `x # 80` model: exit 12, 2 generated
and distinct states and depth 2, but it prints only the initial `x=0` state,
because its initial-predecessor fast path reuses a state object whose random
stream was already consumed. With `Inv == n<3`, TLC prints all four states
`x=0,80,94,100`. Both dive profiles refuse a random violation on the first
successor of an initial state, keeping the prefix statistics and no
counterexample, instead of claiming that a truncated trace is valid.

**Reproduce.** `crates/dive-stdlib/tests/fixtures/random/` holds TLC runs at
seeds 0, 7, −3, 1234567890123, 2^63−1 and −2^63; dive reproduces all 728 values
TLC printed and the text of all 10 error cases. `RndTraceAssume` and
`RndLetRedraw` are the two examples above. `java.util.Random` and TLC's
`RandomGenerator` match bit for bit, the first six draws of `-seed 0` reproduce
the Examples result for `SpanTreeRandom`, and dive's streams replay identically
at 1 to 8 threads and in any processing order. The tests are `tlc_random.rs`,
`java_random.rs`, `examples_random.rs` and `dive_mode.rs` in
`crates/dive-stdlib/tests/`. `TmRandom` and `TmRandomStartup` in
`crates/dive-api/tests/fixtures/checker/tlc-module/` check that checking draws
match TLC's under `--compat=tlc`, for the per-state and start-up streams.

---

## D-003: Simulation is reproducible

| | |
|---|---|
| **Kind** | Different answer only when you fix a seed |
| **Default** | Seed reported; `--seed S` replays a sample at any worker count; `--traces N` is the total |
| **TLC behavior** | Under `--compat=tlc` |
| **Status** | implemented for safety simulation without effects, at any worker count, with dive's per-trace streams in both profiles; `--compat=tlc` does not reproduce TLC's worker selection stream, and its `num` stays a total rather than per worker. Random operators and effects in the spec refuse by name. |

**What TLC does.** Even with `-seed 0` and one worker, the number of states TLC generates in simulation varies between runs. On Examples `EWD840_anim` it ranged from about 267k to 271k.

TLC's simulation uses two kinds of generator, and neither is tied to the trace being generated:
- each worker chooses initial states, actions and successors with its own generator. That generator's seed is the next value of a generator seeded with `-seed` and advanced by `-aril`;
- each worker evaluates `RandomElement` and the `Randomization` operators with a generator seeded with `-seed` itself, and never reseeded, so every worker draws the same sequence of values. What a trace draws depends on which worker generated it and on what that worker generated before.

**What dive does.** The same seed gives the same simulation at any worker
count. Each trace has one stream keyed by seed and trace number, driving state
and action selection, so adding more traces leaves the earlier ones unchanged.
Workers hand finished traces over in trace order: the lowest-numbered trace
with a violation or an error decides the run, the statistics and `Print`
output are those of the traces up to it, and the kept traces are the lowest
numbers. `--workers 8` therefore reports exactly what `--workers 1` reports,
only sooner. `--traces N` counts traces in total; TLC's `num` counts them per
worker, so `-workers 4 -simulate num=10` runs 40 traces in TLC and 10 in dive,
also under `--compat=tlc`. Random operators in the spec refuse by name, as do temporal
properties and runtime effects; `crates/dive-sim/CONTRACT.md` lists every
limit. That makes simulation usable in tests and CI.

TLC's `SimulationWorker.getTrace` removes finite stuttering: at seed 0 and
aril 0, its third trace has five states although six were chosen. dive keeps
the uncompressed trace for validation and gives `tlc_states` separately for
TLC's display. A finite sample always reports `exhaustive: false` and
`model_proved: false`; a zero exit never claims the model was proved.

**Measured.** `crates/dive-sim/tests/fixtures/choice-oracle.json` holds the
pinned TLC's traces and generated counts for five seeds and two arils. dive
does not reproduce them in either profile, because it samples with its own
per-trace stream; every trace it samples for those rows validates
independently. Thirteen more probes compare error classes, exits and counts,
including TLC's 2112 for an action property checked before a constraint. The
receipts W1 and W4 (`crates/dive-sim/tests/fixtures/temporal/oracle.json`)
show TLC's counts changing with the worker count;
`crates/dive-api/tests/simulate_workers.rs` checks that dive's report is the
same at 1, 2, 4 and 8 workers. The validator accepts the
real violations, rejects a corrupted sampled edge and refuses a deadlock whose
absence of successors it cannot decide.

---

## D-004: Integers are unbounded

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Arbitrary-precision integers |
| **TLC behavior** | `--compat=tlc`: 32-bit integers with TLC's overflow error. The JSON report's `options.int` reads `"tlc"`, and `"big"` by default |
| **Status** | default, in both profiles |

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

`Cardinality(SUBSET (SUBSET (1..5)))` (the `ExplicitOuterPowersetCardinality`
model in `conformance/corpus/nested-powerset-primary/`) makes TLC fail during
Init with GENERAL (1000) and exit 75, wrapping the size overflow. The default profile computes
4,294,967,296 and completes with one distinct state, two generated states and
depth one. The TLC profile keeps the overflow and exit.

**What dive does.** dive computes with unbounded integers, as the language intends; small numbers use an inline 64-bit fast path. Under `--compat=tlc` every report above is reproduced word for word. When a run would have stopped with TLC's overflow error, dive's output says so, so the difference is never silent.

**Why it matters.** Four of the five other Rust or Go TLA+ checkers we measured widen integers silently, with no TLC mode and no sign of it in their output.

**Reproduce.** The fixtures `Ov*`, `Ar*` and `ErrCardSubset`/`ErrIntvSize` in `crates/dive-value/tests/fixtures/` hold TLC's transcripts, and dive's messages match them byte for byte. The `Overflow` model in `crates/dive-conformance/tests/fixtures/specs/` runs the first example end to end.

---

## D-005: Symmetry with liveness uses the full state space

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | `--symmetry-liveness=unreduced`: disable symmetry for the whole temporal run and report it |
| **TLC behavior** | `--symmetry-liveness=tlc` (selected by `--compat=tlc`) asks for TLC's reduced liveness checking, which dive refuses by name |
| **Status** | default (`unreduced`); `refuse` implemented; `tlc` refuses |

**What TLC does.** Symmetry reduction lets TLC explore only one representative of each group of states that differ only by a permutation of interchangeable values, such as process ids. Combined with liveness checking ("something good eventually happens"), symmetry can make TLC miss real violations. TLC knows this. It prints a warning ("Declaring symmetry during liveness checking is dangerous. It might cause TLC to miss violations…", `EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), but then carries on and can report success.

Measured on the pinned TLC, it goes wrong in both directions:

- **A missed violation.** `x` moves between three interchangeable model values `a`, `b`, `c` at every step, under weak fairness. `PROPERTY []<>(x = a)` is false: `a, b, c, b, c, …` never returns to `a`. Without `SYMMETRY`, TLC finds that counterexample. With `SYMMETRY Permutations({a, b, c})`, TLC prints the warning and reports **no error**, after exploring 1 state.

  ```tla
  Next == x' \in S \ {x}
  Spec == Init /\ [][Next]_x /\ WF_x(Next)
  InfOftenA == []<>(x = a)
  ```

- **A false violation.** A token alternates between two interchangeable processes under weak fairness, and the property "every process holds the token infinitely often", `\A p \in S : []<>(tok = p)`, is true. The spec and the property are both symmetric. With `SYMMETRY`, TLC reports `Temporal property Fair was violated`, with a counterexample of one state followed by "Stuttering". The fairness condition forbids that behavior.

**What dive does.** When a run has temporal properties and `SYMMETRY`, dive
ignores `SYMMETRY` for the entire run, including its safety checks, reports the
unreduced counts, and says that reduction was disabled. Safety-only runs keep
symmetry reduction. `--symmetry-liveness=refuse` refuses the combination
instead. `--compat=tlc` selects `tlc`, which refuses instead of reproducing
TLC's missed and false violations; it never silently switches to the
unreduced mode. `--compat=tlc --symmetry-liveness=unreduced` gives TLC value
semantics with sound checking and reports `tlc+overrides`. Compare that mode
with TLC run without symmetry. The declared symmetry and the whole-run choice
are reported separately. Reduced checking with `VIEW` is not supported. See
[symmetry with temporal properties](user/temporal.md#symmetry-with-temporal-properties).

**Reproduce.** `SymLiveMissed`, `SymLiveMissedNone`, `SymLiveSymProp` and
`SymLiveWarn` in `crates/dive-liveness/tests/fixtures/tlc-probes/`.

---

## D-006: `CHOOSE` does not depend on unrelated text in the module

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | A canonical order, independent of the module text |
| **TLC behavior** | `--compat=tlc`: TLC's order, including string interning order. The JSON report's `options.choose_order` reads `"tlc"`, and `"canonical"` by default |
| **Status** | default. Under `--compat=tlc`, names are recorded in TLC's encounter order; on 556 corpus models, no name is out of TLC's order. |

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
  - So `{"a", "b", "zebra", "Init", "c"}` prints in that order, and `[qfb |-> 1, qfa |-> {1}]` prints `qfb` first when `qfb` appears first in the module.
- **Sets** order by size first, then element by element: `{{}, {1}, {2}, {0, 5}, {1, 2}}`.
- **Tuples and other functions** order by length (domain size) first, then by domain, then by values: `{<<>>, <<1>>, <<2>>, <<0, 9>>, <<1, 2>>}`.
- **Kinds:**
  - untyped model values come before every other value;
  - `FALSE` comes before `TRUE`;
  - comparing other kinds is an error ([D-009](#d-009-comparisons-that-tlc-refuses-although-tla-answers-them)).
- **Equal elements keep the first form written.** `{1..3, {1, 2, 3}}` prints as `{1..3}`, while `{{1, 2, 3}, 1..3}` prints as `{{1, 2, 3}}`.

**What dive does.** dive uses a canonical order that does not depend on the module text: names in the lexicographic order of their characters (Unicode code points), and everything else structured as TLC orders it. Reordering or adding definitions never changes a result. Under `--compat=tlc`, the order and the printing match TLC's.

**JSON.** `Json!ToJson` uses the canonical object and set order by default.
Under `--compat=tlc` it refuses objects with two or more keys, and compound keys
holding multi-field records (also nested in tuples, sets or functions), because
dive's normalized functions do not keep TLC's construction history. Scalars,
sequences, finite sets and simple keys convert, and strings use Gson escaping
without HTML escaping. Symbolic or deferred sets, the Array and Object entry
points and JSON file operations refuse. Replay accepts JSON strings already
known at binding; a name created at run time in a TLC-profile counterexample
refuses. See [modules](user/modules.md).

**Reproduce.** `ChooseIntern` and `ChooseInternUnused` (this example), and
`PrintChoose`, `PrintSets` and `PrintFcns` (122 printed values, matched byte for
byte), in `crates/dive-value/tests/fixtures/`.

---

## D-007: Exhaustiveness claims can be unconditional

| | |
|---|---|
| **Kind** | Same answer, with a stronger guarantee |
| **Default** | Fingerprint mode, as in TLC |
| **Better option** | `--store exact`: full states, with an unconditional claim that every state was checked |
| **Status** | implemented for safety and temporal properties; it gives the same results as fingerprint mode. |

**What TLC does.** TLC stores 64-bit fingerprints of states, not the states themselves. A collision between two distinct states would silently skip part of the state space. TLC prints a probabilistic estimate of this at the end of a run, but the "no error found" verdict itself is probabilistic.

**What dive does.** `--store exact` stores full states and makes an
unconditional exhaustiveness claim. Every result says which mode produced it.

```sh
dive check Spec.tla --store exact --output json
```

With temporal properties, `--store exact` builds the behavior graph from full
state identities: two states whose fingerprints collide stay two vertices.
TLC's liveness graph is keyed by fingerprint, so a collision can merge states
and change the answer. The human report states the claim ("Exact full-state
storage." or "Fingerprint storage (64-bit); collision probability not
computed.").

**Spilling to disk.** For safety checks, in both profiles and stores:

- `--frontier-memory-bytes N` spills the ordered frontier (zero forces disk),
  keeping concrete traces and ordered counts and reporting I/O and cleanup
  failures. It does not bound the visited store, candidates, traces or graphs.
- `--predecessor-memory-bytes N` (zero goes straight to disk) moves the parent
  records used to rebuild counterexamples. It does not bound the seen set,
  frontier, candidates or rebuilt paths. Simulation, DFID and graph capture
  refuse it.

Storage and cleanup failures never count as completion. Temporal checks refuse
spilled or reclaimed storage by name, and graph spill is not implemented. See
[storage and recovery](user/storage-recovery.md).

**Checkpoints and recovery.** On a finite grid that finishes uninterrupted at
5,123,201 generated and 2,563,201 distinct states, TLC's recovery resets
generated and initial counters to its saved distinct count, so the resumed run
reports 3,703,060 generated. Its timed stop also reports success while 1,511
states are still queued. dive reports a checkpoint pause as an incomplete
search and resumes the saved frontier without regenerating Init. Recovery
resets counters as TLC does by default; `--checkpoint-counters retain` keeps
the original totals. Archives are trusted local state, not proof, and Java
checkpoint files are not readable.

- Safety checkpoints can resume at 1, 4 or 8 workers, or at a different width
  from the pause, keeping discovery, predecessor and frontier order.
- `VIEW` and safety symmetry recover through the same reduced keys as live
  search. The CpView, CpSymView and CpSymConcrete controls report 7/4/4, 5/3/3
  and 3/3/3 (generated/distinct/depth) on TLC and dive. CpSymConcrete keeps the
  concrete value p2 in the witness even though its canonical key is p1.
- Recovery works with the frontier and predecessor budgets, separately or
  together. Archive workspace counts against neither.
- Strings and names created at run time recover with the saved name table: for
  `n' = n + 1 /\ s' = ToString(n + 1)`, recovery reaches the same 4-state
  invariant violation as an uninterrupted check and TLC. TLC encounter order
  runs inline at one worker.
- Runtime effects, register reads, temporal recovery and replay of TLC name
  history refuse. An effectful local alias refuses before output or an archive.

---

## D-008: Results are data, and diagnostics are built for people and tools

| | |
|---|---|
| **Kind** | Same answers, as structured data; exit conventions are selectable ([D-023](#d-023-default-process-exits-are-distinct-conformance-observables)) |
| **Default** | Human-readable output; `--output json` gives a versioned schema |
| **TLC behavior** | TLC-style output conventions under `--compat=tlc` |
| **Status** | implemented: every command offers `--output json` with a versioned schema |

**What TLC does.** Results come as human-oriented text, or as the `-tool` message stream meant for the Toolbox. Counterexamples are available as JSON only through `-dumpTrace json`. SANY's error messages give a location, but no stable error codes and no suggested fix. SANY's command line exits 0 on semantic errors unless you pass `-error-codes`.

**What dive does.** Every diagnostic has:

- a stable code (for example `DIVE-S0010`);
- a precise source span;
- a message that names the construct;
- a likely fix, where one is known.

`--output json` gives verdicts, statistics and counterexamples as structured state sequences, along with diagnostics and an `options` block that records the active profile, options and seed. The library API (`dive-api`) gives the same results in-process, with no JVM. `dive analyze` also gives every semantic diagnostic its SANY error code (`sany_code`), so tools written against SANY's codes keep working.

**`dive check` exit codes.** TLC's process statuses are the default in both
profiles. `--exit-codes native` gives 0 success, 1 rejected input, 10 property
violation, 11 deadlock, 12 assumption and 13 evaluation error. Both keep 2
usage, 3 unsupported or resource limit, 4 internal error and 5 budget stop.
Native statuses with `--compat=tlc` report `tlc+overrides`. See
[process statuses](user/checking.md#process-statuses).

**The `dive tlc` adapter** selects TLC evaluation and one worker by default,
adds missing file suffixes and prints the usual check reports. It reads the
configuration before the module, as TLC does when both are invalid. Finite
`-simulate num=N` and `-generate num=N` give validated, non-exhaustive
sampling. `-lncheck final` with temporal properties, unbounded or file
sampling, `-tool` framing and other TLC protocols refuse by name. See
[TLC checking entrypoints](user/checking.md#tlc-checking-entrypoints).

**Finite temporal-prefix exit codes.** With `S = {}`, this property is false
already in the initial state:

```tla
CONSTANT S
VARIABLE x
Init == x = 0
Next == x' = x
P == \E i \in S : []<>(x = i)
```

TLC reports `Temporal property P was violated` (2116), prints one finite
initial state, and exits **12**, normally the safety status, with 2 generated,
1 distinct and depth 1, because `LiveCheck.printErrorTrace` stores invariant
code 2110 after printing 2116. A script that classifies by exit status alone
mislabels this violation. dive refuses this finite case by name; cyclic
temporal violations exit 13. TLC's raw output for it is in
`crates/dive-conformance/tests/fixtures/temporal-prefix/`.

**Coverage.** `--source-coverage` and `--coverage 0` (TLC `-coverage 0`)
refuse by name in both profiles. Both are off by default, as in TLC. See
[coverage](user/checking.md#coverage).

**PlusCal.** Scoped renaming, multiline macro substitution, branch labels,
symbolic process domains and procedure frames follow TLC's translator, and an
existing translation block is replaced in place. Expansion limits and
generated-operator name collisions refuse by name. A translation is accepted
only if the whole module passes syntax, name and level analysis: TLC's
translator can write a `PrintT` reference without `EXTENDS TLC`, which dive
rejects. Multiline expressions in sequential code translate with TLC's
substitution of changed variables, across LF and CRLF, tabs, blank and comment
lines and UTF-8 comments; junction indentation, binders and multiline
expressions inside macros, processes or procedures refuse. `dive translate
--ast` returns the original-stage `AST.tla` in memory (exit 0,
`artifact_available: true`, with `accepted` and `generated_module_accepted`
false), and `-writeAST` writes it and exits 255 as TLC does. `dive translate
--write` writes an accepted generated module to disk. Debug labeled compounds and flat procedure headers carry
TLC's exact CRCs; compound procedures, procedures owned by processes,
renaming and ordering data and historical fields are not supported. A checksum
match or a translation report says nothing about the model. [D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits)'s
weakness for fair algorithms is kept. See [PlusCal](user/pluscal.md) and the
[translation guide](pluscal-translation.md).

---

## D-009: Comparisons that TLC refuses although TLA+ answers them

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | **TLA+ answers** for `=` and `\in` between functions, records, tuples and sets. The error is kept only where TLA+ leaves the answer unspecified, such as `1 = "a"`. Finite sets are built canonically ([D-028](#d-028-canonical-finite-sets)); direct scalar type errors are unchanged. |
| **TLC behavior** | `--compat=tlc`. The JSON report's `options.comparisons` reads `"tlc"`, and `"tla"` by default |
| **Status** | default; TLC's comparisons are implemented under `--compat=tlc` |

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

The refusal also depends on *how* the function was built:

| Expression | TLC |
|---|---|
| `[qa \|-> 1, qb \|-> 2] = <<1>>` | error: a record literal refuses non-records |
| `("qa" :> 1 @@ "qb" :> 2) = <<1>>` | `FALSE`: the same function built with `:>` and `@@` compares sizes first |
| `[x \in {"qa", "qb"} \|-> 1] = <<1>>` | `FALSE` |

Whether TLC rejects a set also depends on which pairs its insertion sort compares, and so on the string order. `{[fx |-> "qzb", fy |-> 0], <<1>>, [fx |-> "qza", fy |-> 0]}` is a set in TLC (`RvCanonAsym`), but the same elements in another order are an error.

In TLA+, tuples and records are functions. Two functions with different domains are unequal, and a function is not a member of a set of functions with another domain. So each error above has a defined answer, `FALSE`. And `=` should not depend on which side a value is written on, or on how the function was built.

**What dive does.**

- **Under `--compat=tlc`.** dive reproduces all of the above, including the asymmetry and the dependence on how the function was built.
- **In the default profile.** Functions and sets compare extensionally, and
  membership in function, record and sequence sets is structural, so the
  defined mixed cases get symmetric answers. Where TLA+ leaves the answer
  unspecified, TLC's answer or error is kept. Both `{<<1>>, [a |-> 1, b |-> 1]}`
  and the reverse order are checked, including under a `VIEW` that projects the
  set away, and the resulting counterexamples validate.

This changes results where TLC stops with an evaluation error on a defined
comparison. Some heterogeneous set literals still refuse; see
[sets, functions and CHOOSE](user/checking.md#sets-functions-and-choose).

For example, with a configured model value `a`, `a \in {1,a}` is `TRUE` by
reflexivity: the default finishes with two states, while TLC hits a
typed-model comparison error before reaching that element and exits 75 after
one state (`ModelBadPrefix`). Eleven more comparison models in
`conformance/corpus/comparison-receiver-primary/`, with tuple and record
domains and nested tuple values, complete by default where TLC stops at
construction-dependent comparison errors; its `Next` division-by-zero control
agrees with TLC.

**Reproduce.** In `crates/dive-value/tests/fixtures/`: `AsymSetRecFirst`,
`AsymSetTupleFirst`, `AsymEqTupleRec`, `AsymEqRecTuple`, `ErrMemRcdTuple`,
`ErrMemProductRec`, `ErrMemEnumStr`, `PrintMember`, `RvFcnRcd1` to `RvFcnRcd8`
and `RvCanonAsym` hold TLC's transcripts.

---

## D-010: The order in which sets are explored

| | |
|---|---|
| **Kind** | Different answer, only at a violation |
| **Default** | **Canonical order**: sets are explored in the run's value order, which is canonical in the default profile ([D-006](#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)) |
| **TLC behavior** | `--compat=tlc`. The JSON report's `options.explore_order` reads `"tlc"`, and `"value"` by default |
| **Status** | default. In the default profile, every stop at a violation, deadlock or error notes that the result may differ under `--compat=tlc`; dive does not detect whether the order mattered in that run. |

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

**What dive does.** In the default profile, dive explores sets in the canonical order, as it does for `CHOOSE`, so the whole default profile is independent of the module text. Complete searches give the same states, totals and depth as TLC. Only *which* of several violations at the same depth is reported first, and the counts at that stop, can differ. `--compat=tlc` reproduces TLC's order.

**Measured.** These Examples models report the same violated property, depth
and counterexample length as TLC in the default profile, and only the counts at
the stop differ. Under `--compat=tlc` they agree with TLC exactly.

| Model | TLC (distinct / generated / queued) | dive default profile |
|---|---|---|
| `MissionariesAndCannibals` (`Solution`) | 61 / 225 / 11 | 61 / 226 / 11 |
| `acp/ACP_NB_WRONG_TLC` (`AC1`) | 11,852 / 59,286 / 1,248 | 12,645 / 64,357 / 970 |
| PlusCal `SBB` (`Immutability`, depth 15) | 1,617 / 3,126 | 1,581 / 3,042 |

From Rust, setting only the evaluator's `Options::exploration` to
`ExplorationOrder::Tlc` reproduces TLC's `SBB` counts with every other default
unchanged. Both PlusCal label variants of `SBB` were checked, and all six
15-state counterexamples validate.

**Reproduce.** `StringOrderViolation` in
`crates/dive-api/tests/fixtures/checker/tlc-probes/`.

---

## D-011: `TLCGet("config").worker` reports the logical worker count

| | |
|---|---|
| **Kind** | Different answer (only for specs that read it) |
| **Default** | `1`, the *logical* worker count, at every `--workers` setting |
| **TLC behavior** | Under `--compat=tlc`, the physical worker count, as TLC reports it. `--compat=tlc` also defaults to 1 worker, as TLC does |
| **Status** | implemented |

**What TLC does.** `TLCGet("config").worker` returns the number of worker threads. Specs read it to protect themselves from TLC's approximate multi-worker breadth-first search ([D-001](#d-001-parallel-checking-gives-the-same-answer-at-any-worker-count)). For example, Examples `DieHard/MCDieHardest` declares:

```tla
ASSUME /\ TLCGet("config").mode = "bfs"
       /\ TLCGet("config").worker = 1
```

so that TLC is guaranteed to find a shortest counterexample. Run with `-workers 4`, TLC fails that assumption, and the spec refuses to run.

**What dive does.** dive's parallel search gives TLC's single-worker result ([D-001](#d-001-parallel-checking-gives-the-same-answer-at-any-worker-count)), so by default dive reports the logical worker count, 1, at any `--workers` setting. Specs like `MCDieHardest` run at full parallel speed and get the guarantee they ask for. dive's own output still reports the physical thread count.

Under `--compat=tlc`, such a spec can behave differently at different worker
counts, as it does in TLC. `MCDieHardest` agrees with TLC at 1, 3, 4 and 8
workers in both profiles: under `--compat=tlc`, at 3, 4 and 8 workers, both TLC
and dive fail the assumption (`TLC_ASSUMPTION_FALSE`).

---

## D-012: Counterexamples are checked by an independent validator

| | |
|---|---|
| **Kind** | Same answer, with a stronger guarantee |
| **Default** | Every counterexample `dive check` reports is first replayed against the specification by a separate validator. `dive validate-trace` checks any trace on demand |
| **TLC behavior** | Nothing to switch off: validation never changes a verdict. Under `--compat=tlc` the validator evaluates with TLC's semantics |
| **Status** | implemented: `dive-trace`, `dive_api::trace` and `dive validate-trace` |

**What TLC does.** TLC prints the counterexample its own search found, and nothing checks it independently. A bug in TLC's successor generation, or in how it rebuilds the trace, would print a trace that the specification does not allow. TLC's `-dumpTrace json` output cannot be read back reliably, even by TLC: a set and a sequence are both JSON arrays, and a string and a model value are both JSON strings. For example, for `x = {<<1, 2>>}` and `x = <<<<1, 2>>>>`, TLC writes the same `[[1, 2]]`.

**What dive does.** A separate validator checks the trace *as data*. It evaluates `Init` on the first state, and the next-state relation as a predicate on each pair of states, without generating successors: the search code is not compiled into it. It also checks that the claimed invariant or property really fails at the end, that nothing failed earlier, and that every state respects the `CONSTRAINT`s. A trace that fails validation is an internal error (exit 4), never a verdict.

```sh
dive validate-trace DieHard.tla --trace trace.json --claim invariant:NotSolved --compat=tlc
valid: the 7-state trace validates
```

`dive validate-trace` reads dive's own trace format, whose values are exact, and TLC's `-dumpTrace json`, which it reads by trying the interpretations of the ambiguous parts. From Rust, `dive_api::trace::SpecModel` checks a program's own recorded states against a specification. See [trace validation](user/traces.md).

**Liveness.** A lasso validates when the trace lists the complete, nonempty
`properties`: the validator checks the closing edge, every temporal conjunct of
the behavior, and that exactly the claimed properties are false. A
non-stuttering occurrence on the cycle proves both weak and strong fairness;
occurrences only in the prefix, stutters and empty subscripts do not. Without
one, weak fairness needs a disabled cycle position and strong fairness needs
every cycle position disabled. Ten pinned liveness traces validate, among
them recurring fair actions, intermittent enabledness (`LvWFIntermittent`) and
a fair disabled-action stutter (`LvStutterEnd`), all in
`crates/dive-liveness/tests/fixtures/tlc-probes/`. TLC adds implicit stuttering even under
`ACTION_CONSTRAINT x' # x`, reporting a one-state stuttering violation at
`x = FALSE` (exit 13, 3 generated, 2 distinct, depth 2), and dive allows that
stutter too (`LvStutterActionConstraint`).

**Limits.** Deadlocks, fairness and `ENABLED` are confirmed only when a finite
successor cover is available; a missing cover, effects and `VIEW` or symmetry
closing evidence refuse by name, as do unnamed liveness words. Random draws
refuse, because the seed does not fix a draw's position in the stream. See
[where dive is more limited than TLC](#where-dive-is-currently-more-limited-than-tlc).

**Measured.** Of TLC's 15 safety counterexamples in the Examples results, 12
validate and 3 refuse by name (two on unimplemented evaluator operators, one
whose `ALIAS` hides the variables). Every broken copy (a state dropped, two
states swapped, a value changed, the trace cut short, the wrong invariant
claimed) is rejected.

---

## D-013: A deterministic short liveness counterexample

| | |
|---|---|
| **Kind** | Different answer (which counterexample) |
| **Default** | A shortest prefix and a deterministic short cycle; not necessarily the globally shortest lasso |
| **TLC behavior** | Not reproduced; `--lasso tlc` is a usage error |
| **Status** | default in both profiles |

**What TLC does.** TLC's lasso depends on hash-table and SCC visiting order.
`LvNoFair` and `LvTwoLoopsRev` have several violating loops. Changing only
TLC's fingerprint polynomial changed the counterexample on 6 of 21 measured
models; `MCDistributedReplicatedLog` gave five choices, of 4, 5 or 6 states and
with different named properties. That makes diagnosis harder.

**What dive does.** Both profiles report a short lasso that the independent
validator accepts, with every false configured property attributed and checked;
omitted or extra claims are rejected. Under `--compat=tlc`,
`dive_liveness::tlc_plan::classify` reproduces TLC's two liveness formula
refusals (errors 2213 and 2214). dive also reproduces TLC's negation,
simplification, DNF and grouping order for its diagnostics; this does not
change which counterexample `dive check` returns. For example, `TpMultiple`
combines weak fairness with three properties, and TLC produces two
order-of-solution groups and three possible-error models; dive reproduces their
predicate-bin order and index arrays while keeping the original property names.
TLC's configured action name `Live` and derived temporal name `Identity` are
recorded separately, as TLC records them. Of 84 supplied-formula controls, four
refuse by shape and one hits TLC's integer-overflow bound on DNF expansion.
Temporal formulas can quantify over finite constant domains and bind tuples,
and the configured property names stay attached even when a binder splits
roles. TLC's selection, timing and
SCC order are not reproduced. The TLC profile checks once after exhaustion,
and the default profile on the schedule of [D-014](#d-014-periodic-liveness-checking-has-a-replayable-schedule).
See [temporal checking](user/temporal.md#ordinary-checking-in-the-tlc-profile).

---

## D-014: Periodic liveness checking has a replayable schedule

| | |
|---|---|
| **Kind** | Different answer (counts at a violation) |
| **Default** | Checks at completed BFS levels, on a schedule set by the graph's size |
| **TLC behavior** | `--compat=tlc`: one check after exhaustion |
| **Status** | default |

**What TLC does.** TLC checks liveness periodically on a wall-clock schedule,
so where it stops depends on timing.

**What dive does.** A check is due after a completed BFS level once the graph
has at least 2^16 distinct states and at least twice as many as at the last
check. All prefix checks together cost about one final check, smaller models
are checked once at the end, and the schedule is the same at every worker
count. An exhausted frontier gets only the final check. A prefix check never
proves that a property holds, searches only the first configured property, and
reports a violation only after fresh validation, with the actual partial counts
and `capture_completed: false`. Reports name the schedule and its growth,
check, boundary and exhaustion facts. Capture limits are cumulative over the
run, while analysis bounds apply per property and per check, so repeated checks
add work: there is no total-run work or byte cap and no promised speedup.

A violation found on a prefix stops with fewer states than TLC's
`-lncheck final` run. As with TLC's own periodic checking, it can also preempt
an invariant violation or evaluation error beyond the checked prefix that a
final-only run would report instead. TLC's wall-clock schedule is not
reproduced, and neither is a `--lncheck default` timing emulation. Explicit
`--lncheck final --lasso short` refuses by name. See
[ordinary checking in the dive profile](user/temporal.md#ordinary-checking-in-the-dive-profile).

---

## D-015: Symmetry functions must be permutations

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Reject a symmetry declaration that contains a non-bijective map, naming it |
| **TLC behavior** | `--compat=tlc`: accept it and close the set under composition, as TLC does |
| **Status** | implemented for pure safety symmetry in both stores and at any worker count; unverified startup errors, other declaration shapes, lazy permutations and random seeding refuse by name |

**What TLC does.** A declaration containing `[a |-> b, b |-> b]` is not a
bijection. TLC accepts it and closes the set under composition, which permits
an unsound reduction:

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
The collapsing map merges the violating state into the initial state's key.

**What dive does.** The default refuses the non-bijection before search, and
`--compat=tlc` reproduces TLC's success and counts. Symmetric counterexamples
show concrete states and validate independently. dive matches TLC's warnings
for a too-small or unused symmetry set, and its early declaration errors:
`Sym == 1 \div 0` is GENERAL (1000), exit 255, with a nested 2179 message, and
`Sym == {1, TRUE}` wraps its normalization error in GENERAL. dive's
symmetry-closure code has a checked setting, which rejects non-bijective maps
including collisions with implicitly fixed model values, and a TLC setting,
which keeps TLC's closure of malformed maps and its insertion order; all 1,329
direct inputs to TLC's closure code match. With temporal
properties, symmetry follows [D-005](#d-005-symmetry-with-liveness-uses-the-full-state-space).

**Reproduce.** The `SyNonBiject`, `SyNonBijectNone`, `SyDivZero` and
`SyMixedElements` transcripts in `crates/dive-model/tests/fixtures/symmetry-group/`.

---

## D-016: The declared symmetry can be audited

**Status:** not implemented; it would be opt-in. The `SymAsym` and `SymAsymLate`
probes declare symmetry that the transition relation does not respect. TLC
trusts the declaration, so the reduced search can hide behaviors. A
`--check-symmetry` option would check initial states, successors, invariants,
constraints and `VIEW` under the declared generators and report the first
counterexample to the declaration. Ordinary runs, including `--compat=tlc`,
would not run this audit.

---

## D-017: Host access requires explicit grants

| | |
|---|---|
| **Kind** | Different answer (refuses where TLC reads the host) |
| **Default** | No host access; `--allow-io=env,files,exec` grants categories |
| **TLC behavior** | None: `--compat=tlc` also starts with no grants |
| **Status** | grants implemented; external inputs refuse by name |

TLC lets a specification read the environment and files (`IOUtils!IOEnv`,
`IOUtils!Deserialize`) without asking. In dive, every profile starts with no
grants. A specification that reaches `IOEnv` or `Deserialize` refuses without a
grant, naming the category it needs, and refuses with the grant too (exit 3),
because dive does not implement external inputs. For example, Examples
`TLC/TestGraphs` reads `IOEnv` at startup and `ewd998/EWD998ChanTrace` reads it
in its next-state relation. `dive validate-trace --inputs RUN.json` replays the
`external_inputs` records of an existing report offline. See
[host inputs](user/modules.md#host-inputs-and-explicit-grants).

---

## Under consideration

These are recorded candidates. They are not decided, and dive behaves as TLC
does for each of them.

### Empty symmetry sets crash TLC

```tla
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == {}
```

With `INIT Init`, `NEXT Next` and `SYMMETRY Sym`, the pinned TLC exits 255
with GENERAL and an `IllegalArgumentException`, instead of explaining that
there are no permutations. SANY accepts the module. TLC's closure code
allocates a set of capacity `size - 1`, so zero or one function throws when
called directly; normal binding often disables the singleton case first with
warning 2300. dive refuses the empty declaration by name (the `SyEmpty`
capture is in `crates/dive-model/tests/fixtures/symmetry-group/`).

### D-037: Simulation closes loops only on equal states

TLC's simulation closes a liveness loop when two states have the same
fingerprint. Under `VIEW` that is the view's fingerprint, and under
`SYMMETRY` the symmetric one. Two measured results:

- `x` toggles while `y` counts up to 3 with weak fairness, `VIEW x`, and the
  property `<>(y = 3)`. Every behavior reaches `y = 3`, yet TLC reports a
  violation: its loop goes back from `x = 0, y = 2` to the initial state,
  which only has the same view.
- `x` alternates between two symmetric model values `a` and `b` with weak
  fairness, and the property `<>(x \notin {a, b})` is false on every
  behavior. Without `SYMMETRY`, TLC reports the violation; with it, TLC's
  simulation reports none, because the cycle `a, b, a` looks like stuttering
  at `a`, which weak fairness rules out.

dive's simulation will treat `VIEW` and `SYMMETRY` as having no effect on
simulation and close a loop only when a state equals an earlier one. Every
reported loop is then a real behavior that the trace validator checks.

### D-038: Simulation reports how much of a property it checked

TLC's simulation checks a liveness property only on traces that return to an
earlier state or stop. When every trace is cut by `-depth` first, for example
`<>(x = 10)` on `x' = x + 1` at depth 5 or a three-state cycle at depth 2,
TLC reports success exactly as when it checked every trace. dive's simulation
will report, for each property, how many traces gave a complete behavior on
which it was checked and how many could not decide it. It never reports that
a property holds.

### D-039: Extending traces cut by the depth

A trace that reaches `--depth` while a temporal property is still undecided on
it will continue, by up to `--depth` more steps, until it returns to an
earlier state or stops. TLC always ends the trace at `-depth`. Every violation
found this way is a real behavior and is validated, and every extra step is
checked like any other. `--liveness-extend N` sets the number of extra steps;
`--liveness-extend 0`, or `--compat=tlc`, ends every trace at the depth as TLC
does.

### D-040: Sampling from unbounded sets

`x \in Nat` in `Init`, or `\E n \in Nat : x' = x + n` in `Next`, makes
TLC's `-simulate` and `-generate` fail with "not enumerable". dive's
simulation will draw one member of such a set instead: from `Nat` with a
geometric distribution of mean 8 (`--sample-mean` changes it), from `Int` with
a geometric magnitude and a random sign, and from `Seq(S)` with a geometric
length. The draws follow the run's seed, so `--seed` reproduces them, and the
report notes name the distribution and every set that was sampled. Each drawn
value is a member of its set, so every trace is a real behavior; values that
were not drawn are not checked. `--sample-unbounded=off`, or `--compat=tlc`,
reports the same error as TLC.

### Other candidates

- **Which liveness counterexample is reported.** TLC's liveness counterexample depends on the order of its internal hash tables, so it depends on the fingerprint polynomial (`-fp`), which TLC picks at random unless you set it. On the Examples spec `MCDistributedReplicatedLog`, six polynomials gave five different counterexamples, of 4, 5 or 6 states, naming either `InSync` or `AllExtending and InSync` as violated. TLC's choice is also not the shortest available: for `[]<>(x = 0)` on a counter `0, 1, 2` with no fairness, TLC shows three states and then stuttering, where stuttering after two states already violates the property. dive reports a counterexample with the shortest possible prefix, the same one at any polynomial and any worker count; see [D-013](#d-013-a-deterministic-short-liveness-counterexample). The verdict and the state counts are the same either way.
- **Liveness under a state constraint.** TLC checks liveness on the graph that `CONSTRAINT` leaves, while `ENABLED` (and so fairness) still sees the successors the constraint removed. A behavior that leaves the model is not considered at all. For `x' = x + 1` under weak fairness with `CONSTRAINT x < 3`, TLC reports that `<>(x = 5)` holds, although no state inside the model has `x = 5` (`LvConstraint`). With `ACTION_CONSTRAINT` the cut graph can also produce a counterexample that the unconstrained spec does not have (`LvActionConstraint`). dive reproduces TLC exactly. A warning could point out when a liveness verdict depends on a constraint.
- **SANY parser artifacts.** SANY rejects some inputs that look like valid TLA+, apparently because of how its JavaCC parser is implemented:
  - `{a, b \in S}`;
  - `{f(a) \in b : a \in S}`, which is rejected with `\in` but accepted with `∈`;
  - a keyword as a record field anywhere except the first position;
  - `3 <4> 5`, where `<4>` lexes as a proof step;
  - `F(+)`, where `(+)` lexes as `⊕`.

  dive matches SANY exactly. An opt-in `--syntax=lenient` could accept these. The cases are in `crates/dive-syntax/tests/sany-cases/`.
- **SANY semantic-analysis artifacts.** `dive analyze` accepts and rejects exactly what
  SANY does. A few of SANY's verdicts look like artifacts of its implementation:
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

  An opt-in lenient or strict mode could differ. The cases and SANY's verdicts
  are in `crates/dive-sema/tests/sany-cases/`.
- **Line breaking of printed values.** TLC wraps long values at 80 columns by re-parsing their printed form. It gives up, and prints the whole value on one line, when the value contains a model value with `_` in its name, a negative interval such as `-1..1000`, or a string with an escaped quote (`PrintWide` in `crates/dive-value/tests/fixtures/`). dive's printer matches TLC exactly. An option could wrap these too; it would change only human-readable output.
- **`\div` on negative numbers in a spec that extends only `Naturals`.** TLC then uses `Naturals`' own Java division, which rounds toward zero when the quotient truncates to 0: `(0 - 1) \div 2` and `1 \div (0 - 2)` are `0`, although `(0 - 1) % 2` is `1` and `(0 - 7) \div 2` is `-4`. With `Integers` loaded, `(0 - 1) \div 2` is `-1`, as `a = b * (a \div b) + (a % b)` requires. `-2147483648 \div -1` also wraps under `Naturals` instead of overflowing. Under `--compat=tlc` dive matches TLC exactly; with dive's default unbounded integers, `\div` is floored division under both modules (`RvNatDiv` in `crates/dive-value/tests/fixtures/`). A spec that relies on negative numbers should extend `Integers`.
- **"States generated" counts that depend on how a set is represented.** TLC counts every successor its enumerator produces, duplicates included. For `x' \in S` it walks `S` without normalizing it when `S` is a lazily represented set, so an element in both halves of a lazy `\cup` is generated twice:

  ```tla
  VARIABLE r
  Init == r = [a |-> 0]
  Next == r' \in [a : {1}] \cup [a : {1, 2}]
  ```

  The pinned TLC reports 10 states generated and 3 distinct: each of the 3 states generates `[a |-> 1]` twice and `[a |-> 2]` once. Writing the same set as `[a : {1, 2}]`, or as a set enumeration, gives 7. The count also depends on history: once TLC has compared, sized or fingerprinted the same set object, it caches a normalized copy and enumerates that instead. dive matches TLC wherever the two enumerations agree, and refuses the others (see [where dive is more limited](#where-dive-is-currently-more-limited-than-tlc)). An option could count distinct successors only (`NxInLazyCup` in `crates/dive-eval/tests/fixtures/next/`).
- **`-nowarning` changes the number of states generated.** Once an action has assigned every variable, TLC evaluates the rest of it as a plain Boolean instead of enumerating it, but only while warnings are on (`TLCGlobals.warn`, the default). With `-nowarning`, the same action enumerates the rest, so `x' = 1 /\ (TRUE \/ TRUE)` generates two successors instead of one. On `NxFastPath` the pinned TLC reports 7 states generated by default and 37 with `-nowarning`, for the same 4 distinct states. Initial states always take the shortcut. dive reproduces both (`Options::warnings`).
- **The order in which `\A` explores its bindings in an action.** For `\A i \in {1, 2, 3} : A(i)` in a next-state relation, TLC enumerates `A(1)` first and then `A(3)` before `A(2)`: it conses the remaining bindings onto its continuation list, last first (`NxForallOrder`). This decides which successor comes first when the `A(i)` assign variables. dive reproduces it.
- **Java overrides that disagree with the CommunityModules' TLA+ definitions.** TLC runs the Java override, never the definition, and dive does the same. Measured on the pinned TLC and CommunityModules `202610040242` (in `crates/dive-eval/tests/fixtures/specs/`):
  - `Bitwise!shiftR(n, pos)` is Java's `n >>> pos`, which uses only the low five bits of `pos`: `shiftR(16, 32)` is `16` and `shiftR(16, 33)` is `8`, where the definition gives `0` (`CmBitwise`). On negative numbers, outside the definitions' domain, `x ^^ y`, `Not` and `shiftR` follow Java's 32-bit two's complement (`(0 - 5) ^^ 3` is `-8`, `shiftR(-16, 2)` is `1073741820`).
  - `SequencesExt!LongestCommonPrefix` on strings assumes the first string of the normalized set is the shortest. TLC orders strings by when it first met them, not by length, so `LongestCommonPrefix({"q7q", "q7"})` fails with `String index out of range: 2` instead of returning `"q7"` (`ErCmLcpStringOrder`).
  - `BagsExt!FoldBag` folds in the order TLC happens to hold the bag in, which for a bag built with `:>` and `@@` is the order it was written. Dive folds in normalized key order by default ([bag fold order](#d-031-bag-fold-order)). TLC exploration order refuses when source order cannot be recovered.
  - `SetToBag` of a non-set reports its error as `BagToSet`'s (`ErCmSetToBagNonSet`).

  dive reproduces these in both profiles. An option could follow the definitions instead.
- **Errors inside `TLCGet`, `TLCEval` and other evaluating overrides are wrapped.** An error raised while TLC evaluates `TLCGet`, `TLCEval`, `ReplaceFirstSubSeq` or `ReplaceAllSubSeqs` (including in their arguments, and in the TLA+ definition TLC falls back to) is reported as `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` with the Java method's signature, around the actual error: `TLCGet(7)` with no register 7 is reported that way instead of as `TLCGet(7) was undefined.` (`ErCmTlcGetUndefined`, `ErCmTlcGetArgError`, `ErCmTlcEvalError`, `ErCmReplaceFallbackError`). dive reproduces the wrapping. An option could report the inner error alone.
- **String length in UTF-16 units.** TLC's `Len`, `SubSeq` and `Tail` on strings count UTF-16 code units, not characters (tlaplus#1076).
- **Quirks of TLC's `.cfg` reader.** TLC's model-configuration reader accepts some files that it will later fail on, and it reads some values in unexpected ways. Measured on the pinned TLC:
  - `CONSTANT N = -1` is not a negative number. TLC reads `-` as a model value, and then fails on the next line with "expecting = or <-" (`CFG_EXPECTED_SYMBOL`, exit 151). A cfg file has no way to write a negative integer.
  - `N = 2147483648`, or `N = \h1F`, stops TLC with an unexpected `NumberFormatException` (`GENERAL`, exit 255) instead of a configuration error.
  - `INVARIANT Inv1, Inv2` is accepted, with the comma read as an invariant name. TLC then fails with "The invariant , … is not defined".
  - `INIT NEXT` (a forgotten name) makes `NEXT` the initial predicate.
  - `{1,}}` is read as a set containing the model value `}`.
  - String values keep their escapes verbatim, so `"a\nb"` in a cfg is four characters, unlike the same literal in a module.

  dive matches TLC exactly, and adds warnings (`DIVE-C0100` to `DIVE-C0102`) where TLC accepts input it can never resolve. An opt-in `--cfg=strict` could reject these files. The cases are in `crates/dive-config/tests/fixtures/`. Earlier TLC versions did worse: a lexical error silently truncated the file, so TLC could report success while ignoring an `INVARIANT`. The pinned TLC rejects such files, and so does dive.

## Where dive is currently more limited than TLC

dive refuses by name where it cannot match TLC. These are the refusals.

- **Extremely deep nesting** (`DIVE-S0040`). dive refuses expressions nested more than 256 levels deep, and syntax trees more than 2,048 levels high (for example a chain of thousands of `+`). SANY accepts these. No module in the corpus comes close.
- **Very deeply nested values** (`DIVE-V0030`). dive refuses to print, to convert to JSON, or to compare, test for equality or test for membership with TLC's rules, a value nested more than 500 levels deep, such as `<<<<<<...>>>>>>`. Its own equality, ordering, hashing and fingerprints handle any depth, and so does dropping a value. TLC works recursively up to the limit of its JVM stack (`-Xss`). The limit keeps dive safe on a 2 MiB worker stack.
- **Very large sets.** dive enumerates at most 2^31−1 elements of a set (`DIVE-V0020`). TLC's own limit is the same, from Java's `int` sizes. Larger sets that TLC handles without enumerating, such as `0..2147483646` in a state, are handled the same way in dive.
- **Very deeply nested cfg sets** (`DIVE-C0030`). dive refuses a constant value in a `.cfg` file whose sets are nested more than 1,024 deep. TLC reads them recursively, and stops with a stack overflow at a depth that depends on its `-Xss` setting. No cfg file in the corpus comes close.
- **PlusCal re-translation during analysis.** When a module contains both a PlusCal
  algorithm and its translation markers, SANY re-translates the algorithm to check it
  against the translation.
  - Two outcomes of that check reject the module: an unterminated string in the algorithm,
    and `END TRANSLATION` without `BEGIN TRANSLATION`. `dive analyze` matches both.
  - SANY would also reject a module whose re-translation throws an unexpected exception.
    `dive analyze` does not re-translate (`dive translate` does), so it could accept
    such a module. No corpus module or mutant has shown this.
- **Evaluation refusals.** The evaluator refuses, naming the feature, instead of guessing:
  - a Java-overridden operator dive does not implement: `JavaTime`, Json file operations and representation-dependent Array/Object entry points, most of `TLCExt` (`AssertError`, `TLCCache`, `TLCFP`, ...), `Randomization!RandomSubsetSet`, and the CommunityModules operators `FiniteSetsExt!kSubset`, `Combinatorics`, `Statistics!ChiSquare`, `VectorClocks!CausalOrder`, `GraphViz`, SVG layout and element serialization. `SVG!PointOnLine` works on literal stored records, as described in [modules](user/modules.md); other SVG argument forms refuse. TLC never uses these operators' TLA+ definitions, so dive does not either;
  - cases of implemented overrides whose TLC result depends on something dive does not track: TLC-order `BagsExt!FoldBag` on a bag of two or more elements, `Bags!BagOfAll` with an operator that has side effects, and the error a few `Bags`, `Functions` and `IsInjective`/`AntiFunction` cases raise when values TLC cannot compare meet in an order dive does not know;
  - `TLCGet` keys that read TLC's internal description of the run (`"spec"`, `"action"`, `"revision"`) or its JVM (`"-D..."`), the live counters (`"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"stats"`, `"duration"`) while states are being checked (they are provided while assumptions are checked), and `TLCSet("pause", TRUE)`;
  - the random operators when the evaluator runs without a model checker providing a random stream (for example `ASSUME`s evaluated on their own);
  - a function that TLC keeps unevaluated (`[x \in S |-> e]` over an infinite domain, or a recursive function) stored inside another value, when computing it could fail or has side effects;
  - enumerating `S \X T` or `[a : S]` with an infinite component, a set too large to count when it is enumerated lazily, and `IsFiniteSet` of a lazily evaluated set;
  - applying a function of 32 or more arguments to a value TLC cannot compare with its domain.

  An evaluation nested deeper than the depth limit is a resource limit, as TLC's stack overflow is.
- **Successor-generation refusals.** The successor generator refuses, naming the case, instead of guessing the successors TLC would generate:
  - `x' \in S` where `S` is a lazily represented set (for example `[a : S] \cup [a : T]`, or a set filter or `UNION` built from one) whose TLC enumeration lists an element twice, or in an order other than the normalized one. TLC's answer depends on whether it has already compared, sized or fingerprinted that same set object (see [under consideration](#under-consideration));
  - applying, as an action, a lazily evaluated function that has pending `EXCEPT`s or an argument outside its domain;
  - a built-in operator passed as an operator argument and applied as an action (`Op(\/)`);
  - `x' \subseteq S` where `S` is not a set.
- **`dive check` refusals.** dive checks invariants, deadlock, temporal properties with fairness, and `SYMMETRY`. With a temporal property, `SYMMETRY` is checked on the unreduced state space in the default profile ([D-005](#d-005-symmetry-with-liveness-uses-the-full-state-space)). TLC's reduced liveness checking (`--symmetry-liveness=tlc`) is refused, and it is the `--compat=tlc` default, so under `--compat=tlc` a temporal property with `SYMMETRY` is refused unless `--symmetry-liveness=unreduced` is passed. dive also refuses, naming the feature:
  - `POSTCONDITION`;
  - `--chaos`, `--action-composition=tlc`, and action composition (`\cdot`) with conjuncts after it or variables assigned before it in the default mode ([D-022](#d-022-action-composition-works-by-default-with-its-tla-meaning));
  - with temporal properties: the explicit bounded final mode (`--lncheck final --lasso short`);
  - anything the binding stage or the evaluator refuses (for example `IOUtils`, Json file operations and the CommunityModules operators listed above);
  - evaluation errors whose exact TLC error class depends on where a value came from, which dive does not record. dive reports their messages but marks the deciding class as unverified.

  `ALIAS` is not applied to printed counterexamples: dive prints the states themselves, and says so. `dive simulate` renders the `ALIAS` beside each trace's states (the trace's `alias` array), but refuses an `ALIAS` that names no definition, and leaves a trace without a rendering, with a note, when evaluating the `ALIAS` fails, prints, draws or reads other runtime state. `dive simulate` does not evaluate a `VIEW`, so it does not report an error that evaluating the `VIEW` on a sampled state would raise. `dive simulate` runs with one worker; parallel simulation workers are refused.
- **Trace validation.** The validator refuses, naming the reason, instead of trusting the checker:
  - deadlock endings whose absence of successors the finite-cover check cannot decide. A concrete successor refutes the ending, and exhausting a complete finite cover validates it;
  - liveness claims that lack a complete named-property list, or need fairness or `ENABLED` facts the evaluator cannot decide on the trace's states, effects, or `VIEW` or symmetry closing evidence;
  - the final claim of an assertion failure or evaluation error outside the [supported error endings](user/traces.md#endings). The steps are checked;
  - a TLC `-dumpTrace json` document written under an `ALIAS` whose records do not contain the variables, and one too ambiguous to search exhaustively when no interpretation tried validates;
  - traces whose states leave variables unassigned (partial implementation traces), which need a search;
  - Rust typed states with names whose text and token do not match the model's interner (`DIVE-V0041`), opaque deferred values whose names cannot be inspected without running caller code (`DIVE-V0042`), or sets, records and functions sorted in the other profile's value order (`DIVE-V0043`). Build values with the model's interner and value order, or decode a trace document against that model;
  - whatever the evaluator refuses, and reads of the live counters, which depend on the search, not on the trace. `TLCGet("level")` is the level of the state being checked (a primed `TLCGet("level")`, which TLC evaluates at the successor's level, refuses), and `TLCGet("config")` is provided when the run's `-seed` and `-fp` are known. Register reads and writes, including `TLCGet("all")` and `TLCGet("all:named")`, refuse during evaluation, because their values can depend on evaluations outside the trace. Untaken calls are fine, and validation leaves caller registers unchanged;
  - every random draw made while checking the trace, even with the run's `-seed` and `-fp`: only the successor generator determines its stream position.

  A step label that names an operator with parameters is noted and not checked.
- **Simulation with a liveness `PROPERTY`.** `dive simulate` refuses a configuration that names a temporal `PROPERTY` (`simulation temporal properties: <names>`), including the fairness conjunct of a specification named as a `PROPERTY`, where TLC checks it on the simulated traces. Use `dive check` for the property, or remove it from the simulation configuration. Examples: `ewd687a/EWD687a_anim` (property `DT2`) and `ewd998/SmokeEWD998` (the `WF_vars(DetectTermination)` conjunct of `PROPERTY TDSpec`).
- **`VIEW` combined with liveness.** A model that declares a `VIEW` and checks a temporal property is refused by name (`graph capture with VIEW requires quotient edge and closing-state evidence`). In these models the `VIEW` is what makes the state space finite, so the search runs on the reduced graph. A liveness verdict there is sound only with an argument that the `VIEW` preserves the property, plus a way to rebuild a concrete counterexample; dive has neither. Safety checking with a `VIEW` is unaffected. Examples: `FiniteMonotonic/MCDistributedReplicatedLog`, `ewd998/EWD998ChanID`.
- **TLC's exact liveness counterexample.** TLC picks its lasso through hash-table and SCC visiting order, which depends on the fingerprint polynomial. dive reports a deterministic short lasso that the validator accepts, with the same verdict and state-space facts. Each report names every property proved false on its own counterexample, so different valid lassos can name different sets. `--lasso tlc` is a usage error ([D-013](#d-013-a-deterministic-short-liveness-counterexample)).
- **Configurations embedded in a `.tla` file.** TLC can read a configuration from a `---- CONFIG Name ----` section of a `.tla` file. `dive parse --config X.tla` refuses this as `unsupported`.
- **Binding refusals.** Binding a specification to its configuration refuses, naming the feature:
  - a constant operator given values in the `.cfg` (`F(1) = 2`), which TLC holds as a table of values (`OpRcdValue`);
  - unknown or changed Java classes next to a module: TLC loads them as module
    overrides, while dive cannot run arbitrary Java. The exact Hanoi `Bits.class`
    has a native port ([D-024](#d-024-the-hanoi-bits-override-gives-the-one-worker-result-at-any-width));
  - TLC's undocumented `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE` sections. A `_PERIODIC` entry is bound first, with TLC's errors for an undefined name (2229) or an operator with parameters (2228), and then refused: dive does not run the hook on TLC's wall-clock timer ([D-014](#d-014-periodic-liveness-checking-has-a-replayable-schedule));
  - a definition TLC evaluates at start-up that dive's evaluator refuses, when the result decides how the model is bound (it is named in the configuration, it is a conjunct of the behavior specification, or it prints or draws), or when the refused construct splits the next-state relation;
  - the few messages whose text depends on Java objects dive does not have (a Java override used as a configuration name, a state variable used as a conjunct of `SPECIFICATION`).

---

Independent replay refuses every random draw it meets while checking a supplied
trace. Seed and fingerprint settings describe how the model binds; they do not
allow random replay or a valid result.

## Temporal checking limitations

Temporal formulas can quantify over bounded function sets, record sets,
Cartesian products and subsets, and over closed pure filters and maps of them:

```tla
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Funcs == [{1, 2} -> BOOLEAN]
Live == \A f \in Funcs : []<>(x = f[1])
Domain == {f \in [1..2 -> BOOLEAN] : f[1] = FALSE}
LiveFiltered == \A f \in Domain : []<>(x = f[1])
```

TLC holds `Live` with 3 generated states, 2 distinct states and depth 2;
replacing `Next` with `UNCHANGED x` gives a liveness violation (exit 13) with 2
generated states, 1 distinct state and depth 1. `LiveFiltered` behaves the same
way, and with `Init == x = TRUE` and `Next == UNCHANGED x` it is violated with
the same counts. dive matches these in both profiles and stores, and every
violation validates. A filter captures the state in which its value was
created, and primed predicates use the successor state.

Domain size, construction work and Cartesian bindings are bounded separately,
using cardinality upper bounds charged before any predicate runs. Infinite or
opaque domains, oversized constructors, effectful or unresolved predicates,
non-filter deferred set operations and recursive local functions as domains
refuse by name. The eight paired function, record, product and subset models,
with TLC's captures, are in `crates/dive-api/tests/fixtures/lazy-domains/`.

Pure expression arguments such as `Identity([]<>x)` keep their caller context,
including a captured `LET` value or a substituted module body. A temporal
argument passed through an exported `INSTANCE` parameter refuses:

```tla
\* Inside the instantiated Leaf:
Identity(p) == p
Claim(p) == Identity(p)
\* At the root, after two INSTANCE substitutions:
P == I!Claim([]<>x)
```

TLC reports 2213, exit 255, at the inner formal for this (`TfInstanceHolds`
and `TfInstanceFails` in `crates/dive-api/tests/fixtures/temporal-forwarding/specs/`
pass SANY). A zero-arity
`Claim == []<>v` in Leaf with `Identity(I!Claim)` at the root works. Opaque
captured operators, recursive local-function domains and more complex `ENABLED`
scopes refuse. Operators passed by a Rust caller refuse unless their purity is
proved, and unknown or missing capture metadata and unchecked expression ids
refuse with typed errors; an evaluator built with an invalid redirect target
can still panic before reaching them.

TLC's liveness evaluator skips an action body on a subscript stutter, while
ordinary action evaluation checks the body first. dive follows TLC's liveness
order, also on replay. The division-by-zero example and 36 controls are in
`crates/dive-liveness/tests/fixtures/probes/temporal-action-order/`.

### Runtime names in TLC order

A model such as `s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)`
observes the order in which strings are created at run time. TLC also creates
its configuration strings before assumptions: with `x \in 0..15` and the
invariant in `crates/dive-api/tests/fixtures/checker/runtime-names/DynamicStringOrder.tla`,
seed 7 stops at 23 generated and distinct states, and seeds 0 and 14 at 30.

Under TLC value or exploration order, reachable string constructors (including
tuple uses of operators that can also build strings, record-name constructors,
wrappers and native overrides) make the check run inline at one worker, with
the reason reported. The default canonical order stays parallel. Finite safety
simulation refuses these models in the TLC profile; default sampling works.

### Independent runtime-name trace validation

TLC's encounter order can depend on strings created while exploring states
outside a counterexample, and reading the counterexample itself creates
strings, so the order cannot be inferred from the trace.

In `crates/dive-trace/tests/fixtures/runtime-names/ReplayNames.tla`, TLC with
seed 0, fingerprint 0 and one worker completes safely with 3 generated states,
2 distinct states and depth 2. A forged trace `(x=6, s="unset") -> (x=6, s="7")`
claims an invariant violation; loading `"7"` before building `"6"` would change
what `CHOOSE` selects.

So TLC-order validation follows one rule. Names created when dive binds the
model (SANY's encounter order, constants and configuration, and the checker's
start-up names) have TLC's order. If decoding the trace or evaluating any check
creates a name after binding, the result is **unsupported: runtime string
encounter order**, whether the trace would otherwise be valid or not. This
applies to dive's documents, TLC's JSON documents, the Rust API, `ENABLED`
decisions, supplied lasso formulas and independent `ENABLED` predicates. On
`ReplayNames`, both the forged `"7"` trace and the real `"6"` behavior refuse
under `--compat=tlc`; the default profile accepts the real behavior and rejects
the forged step. Literal strings are fine, and so are models whose string
operators (`Tail`, `\o`, `SubSeq`, `ToString`) only build values from start-up
names, such as `MCCheckpointCoordinationFailure`. The checker's seed and
fingerprint strings are always known in the right order.

### Bounded parallel temporal capture

Temporal capture uses the requested workers for supported models and keeps
every accepted edge, including duplicates and already seen states, in the
inline order, so reports are identical at 1, 4 and 8 workers. At most eight
states are expanded per wave. A separate bound admits at most 65,536 successors
of one state by default; reaching it is a resource refusal with ordered partial
evidence. See [the bound](temporal-api.md#bounded-parallel-graph-retention).
`VIEW`, symmetry, randomness and stateful effects refuse during capture.

## D-018: A DFID zero exit may mean only a depth-bounded check

| | |
|---|---|
| **Kind** | Reporting; the specification's meaning is unchanged |
| **Default** | Report incomplete termination at an exhausted DFID bound |
| **TLC behavior** | `--compat=tlc` keeps TLC's exit status, with the incomplete scope reported |
| **Status** | implemented for pure safety models |

```tla
VARIABLE x
Init == x = 0
Next == x' = x + 1
```

This model has infinitely many reachable states. TLC's `-dfid 2` exits zero
after 2 states generated and 2 distinct, and `-dfid 4` exits zero after 7 and
4. Neither prints the completed-check message 2193. The help calls the argument
an initial depth, but it is a maximum level. `-dfid 0` and `-dfid 1` run only
initialization and exit 255.

`dive dfid` reports the level reached, partial counts and
`termination: depth-bound`, with `exhaustive: false` and `model_proved: false`.
A zero exit in the TLC profile is never a proof. See [DFID](user/dfid.md).

TLC rejects `-dfid` with more than one worker, or with temporal properties, as
a general error and exits 255. `dive dfid` refuses both by name,
`DFID multiple workers` and `DFID temporal properties and fairness`, with exit
3, in both profiles.

Even with seed 7, aril 3 and fingerprint index 0, TLC's `DfidBranch` produces
both `0,1,3` and `0,2,4` counterexamples: each `DFIDWorker` seeds a new Java
generator from its own fresh `nextLong()`, not from the command-line seed.
`dive dfid` reports every worker seed, so default runs replay, and
`--worker-seeds` labels a TLC-profile request as mixed. dive does not
reproduce TLC's fresh seed distribution.

## D-019: DFID can truncate a counterexample after switching initial roots

```tla
EXTENDS Naturals
VARIABLE x
Init == x \in {0, 1}
Next == IF x = 0 THEN x' = 0 ELSE x' = 2
Inv == x < 2
```

**What TLC does.** `DfidRootsMixed` with `-dfid 6 -workers 1 -fp 0 -seed 7
-aril 3` reports invariant code 2110 and exit 12. When root 1 is explored
first, the printed witness is `1,2`, with 3 generated and 3 distinct states.
When harmless root 0 is explored first, TLC can instead print only `1`, with 4
generated and 3 distinct states. State 1 satisfies `Inv`, so the truncated
trace does not show the reported violation. `DFIDWorker` checks the next root
before resetting its traversal depth, so its invariant printer walks the
previous root's stack depth.

**What dive does.** `dive dfid` keeps the concrete path, returns `1,2`, and
reports the violation only after independent validation. `--compat=tlc` keeps
TLC's search and counts but never labels a malformed trace valid, and there is
no mode that prints TLC's truncated witness. The observations are in
`crates/dive-api/tests/fixtures/checker/dfid/`, where `oracle-controls.json`
lists the clean TLC runs.

## PlusCal limitations

The [translation guide](pluscal-translation.md) lists the supported constructs,
options and refusals. These are the differences from TLC's translator:

- TLC's grammar scanner accepts `(1]`, `[1)`, `<<1}` and `[1, end]`, because it
  counts delimiter nesting instead of parsing expressions. dive keeps those
  source slices and validates them when it translates.
- The minimal `process P = 1 variables x=self` translates in TLC with exit 0
  but leaves `self` unbound, and SANY rejects the output. dive refuses it in
  both profiles.
- A process with missing labels always needs `--label`, unlike a sequential
  algorithm, where TLC's translator adds the labels by default.
- Against TLC's translator, dive's PlusCal parser was compared on 222 inputs
  and markers: 200 give the same AST field for field, 13 are rejected by both,
  four differ at the macro-expansion stage, one times out in TLC's translator,
  and four are documentation markers that are not standalone inputs.
- Errors in public Rust contexts (an invalid evaluator redirect, a malformed
  lazy expression id) refuse with `DIVE-EVAL-C001` to `C005` before evaluation,
  and trace and predicate APIs reject metadata from another model. Opaque
  deferred Rust bindings refuse instead of running user code. The cases are
  in `crates/dive-api/tests/fixtures/public-context/`.

### Translator success does not ensure a valid TLA module

```tla
---- MODULE TranslatorName ----
(* --algorithm A
variable Next = 0;
begin L: Next := 1; E: skip; end algorithm; *)
====
```

TLC's translator exits **0** and prints a successful translation. Its output
then declares variable `Next` and operator `Next`, and SANY rejects the module.
Changing `Next` to `vars` has the same outcome, as do two `define` cases and a
shadowing `with` binder.

dive refuses generated-name collisions and binder shadowing it cannot resolve
before returning a module, and accepts a translation only if the whole module
passes syntax, name and level analysis, in both profiles. It does not rename
to avoid the collision.

## D-020: fair-algorithm checksums do not detect source edits

This source has the same algorithm checksum after changing `x=0` to `x=9`:

```tla
---- MODULE FairCRC ----
EXTENDS Integers
(* --fair algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)
====
```

TLC's translator emits pcal checksum **`7c28162a`** for initial values 0, 1 and
9 in both C and P syntax.
Their TLA-body CRCs are **`64f9cea5`, `7faa8ba4`, `a530a3ac`**.
Editing only the initializer in the translated file from 0 to 9
still gives **`NO_DIVERGENCE`** in all four C and P validator runs. This is a
missed source edit: `trans.performTranslation` and `Validator` hash `"fair"`
alone because of Java ternary-expression precedence.

**Status: TLC defect; no corrected checksum adopted.** dive writes TLC's
algorithm CRC for the procedure-free shapes it supports, including this
constant one, so the checksum does not prove the source is unchanged. When an
exact algorithm CRC is unavailable, rewriting disables that value and keeps the body CRC. `--checksums full` and `--checksums tla` both select this
best-effort policy in either profile. See [PlusCal](user/pluscal.md) and the
[block rewriting](pluscal-translation.md) contract.

## D-021: PlusCal checksums need the generation options

```tla
---- MODULE OptionCRC ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variable g=0;
process P \in {1,2} begin L:g:=1; E:skip; end process;
end algorithm; *)
====
```

TLC's `pcal.trans -wf OptionCRC.tla` keeps only two hexadecimal CRCs in the
translation header and does not record `-wf`. Here the pcal CRC is `247b4187`.
A fresh Validator without options reports `PCAL_DIVERGENCE_EXISTS` with no
edit; the same Validator with `-wf` reports `NO_DIVERGENCE`. `-sf` and inserted
or custom labels show the same false change. An explicitly `fair+` process
translated with `-nof` validates both ways, because the source keeps the
declared fairness even when the generated formula omits it. The Validator
reparses with its current options and the source's `PlusCal options (...)`
declarations, so declarations in the file validate without options.

**Status: TLC behavior; no corrected default.** For supported
procedure-free process shapes, dive derives exact CRCs from the options you pass
and reports `translation_options` on every replacement. The CLI rejects
repeated or conflicting fairness flags before loading. Supported source
declarations are honored as in TLC; unknown, malformed or conflicting ones
refuse by name. Keep the options with the artifact, and validate with the same
ones: a Validator run without them proves nothing about edits. Under
`--compat=tlc` the same CRC and options context applies. The fair-algorithm
weakness ([D-020](#d-020-fair-algorithm-checksums-do-not-detect-source-edits))
can also hide changes. See [source options](pluscal-translation.md).

A declaration inside a previous translation block is still read by TLC's
translator. dive refuses before masking the old block, instead of producing
output without the requested fairness
(`crates/dive-api/tests/fixtures/pluscal-source-option-masking/`).

### Bounded pure native operator arguments

Temporal checking accepts `Len`, `Head` and `Cardinality` as operator arguments
in pure constant and state predicates: TLC and dive both report exit 13 for
`Use(F(_)) == []<>F(x); Live == Use(Head)` with `x=<<FALSE>>`. Known user
operators passed as formals keep their caller scope and are inspected even when
unused. Configured, opaque, effectful, native, recursive and higher-order
targets refuse. Captured concrete values, such as
`<<[tag |-> "ok", vals |-> {a,b}], <<1,2>>>>` and configured string, model and
set constants, are supported within the existing 50,000 and 128 bounds; opaque
or cached providers, invalid namespaces and effects refuse. Rust callers must
supply sets, records and functions in the evaluator's stored value order.

### D-021 retained source-option errors and partial application

TLC's translator also continues after some source-option errors. In C and P
process examples, `PlusCal options (-wf, -sf)` prints an `Unrecoverable error`,
exits **0**, writes a translation and keeps the first weak-fairness setting.
`PlusCal options (-unknown)` also exits 0 and writes a translation after
reporting the unknown option, and an unterminated declaration can consume a
later `)` from the algorithm and apply a setting. dive refuses malformed,
unknown or conflicting source options before generating anything, instead of
applying them partly. A source declaration that overrides CLI fairness follows
TLC. A zero exit from a translator never proves the model or the artifact
([D-008](#d-008-results-are-data-and-diagnostics-are-built-for-people-and-tools)).

## D-022: Action composition works by default, with its TLA+ meaning

| | |
|---|---|
| **Kind** | Different answer (only for specs that use `\cdot`) |
| **Default** | `--action-composition=tla`: `A \cdot B` is evaluated, in successor generation, in expressions and under `ENABLED`, as TLA+ defines it |
| **TLC behavior** | `--action-composition=off`, selected by `--compat=tlc`: every use is TLC's error. `--action-composition=tlc` (also spelled `-Dtlc2.tool.impl.Tool.cdot=true`) asks for TLC with that switch; dive accepts it only for TLC-profile checks whose properties are all safety, and refuses it otherwise |
| **Status** | default for a complete enumerable first stage followed by a deterministic tail that assigns each variable once from unprimed expressions; other shapes refuse by name |

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

**What dive does.** By default, dive evaluates `A \cdot B` with its TLA+ meaning: `s -> u` is a step iff some state `t` has `A` from `s` to `t` and `B` from `t` to `u`. Only the composed successors count as generated, so the first spec above is reported as deadlocked. Where the composition sits inside a conjunction (conjuncts after it, or variables already assigned before it), and for incomplete intermediate states, effects, nested or recursive composition and tails dive cannot prove deterministic, dive refuses by name instead of approximating. Duplicate final candidates still count. `TLCGet("-Dtlc2.tool.impl.Tool.cdot")` is `"true"` whenever composition is enabled. An explicit composition option wins over the profile in any flag order, and `options.action_composition` reports the setting. See [action composition](user/compatibility.md#action-composition).

Under `--compat=tlc` composition is off, as in TLC; add `--action-composition=tla` to keep it with the profile's other switches. `--action-composition=tlc` is accepted only in the TLC profile when every property binds to initial predicates, invariants and implied actions; it then keeps TLC's intermediate generated counts and runs sequentially, but a counterexample that needs composed replay refuses ([temporal checking](user/temporal.md#ordinary-checking-in-the-tlc-profile)). DFID and simulation count only emitted successors. For `CdDeadlock`, each reports deadlock (2114) with one generated state, where TLC exits 11.

**Replay and fairness.** In `tla` mode, replay accepts pure finite complete
intermediates with at most one complete tail successor, and applies a
nonstuttering test to the original-to-final edge. For
`A == (x'=1) \cdot (x'=0)` at `x=0`, the only composed edge is `0 -> 0`:
`ENABLED A` is true, `ENABLED <<A>>_x` is false, and `WF_x(A)` and `SF_x(A)`
hold on the stuttering word. TLC's experimental implementation reports both
queries true and both fairness properties false (exit 13, one-state stuttering
counterexample). Replay refuses `off` and `tlc` modes, effects, nested or unproved
compositions, partial states and nondeterministic tails. See
[trace validation](user/traces.md#language-notes).

**Measured.** In `crates/dive-eval/tests/fixtures/cdot/`:

| Probe | TLC with the switch (generated / distinct) | dive default |
|---|---|---|
| `CdNext` (top-level composition) | success, 65 / 18 | deadlock: a state with `y = 3` and `x < 4` has `A` steps but no `B` step |
| `CdDeadlock` (the first spec above) | success, 2 / 1 | deadlock, 1 / 1 |
| `CdConj` (the second spec above) | deadlock, 1 / 1 | refused by name |
| `CdEval` (action property) | `Prop` violated, 5 / 5 | the same |
| `CdEnabled` (`ENABLED` in an invariant) | `Inv` violated, 2 / 2 | the same |
| `CdOff` (no switch) | error 1000 | the same under `--compat=tlc` |

The `Reduction` example on the [compatibility page](user/compatibility.md#action-composition)
succeeds with 3 distinct states, 7 generated states and depth 2, where TLC with
the switch counts 13 generated states, including six intermediate ones.
`MCCRDT` and `MCReplicatedLog` get past the composition in both profiles.
Checked for safety without their `PROPERTIES`, at one worker,
`--compat=tlc --action-composition=tlc` matches TLC's counts exactly, and the
default profile's generated counts leave out TLC's 450,000 and 5,808
intermediate states:

| Model | TLC (generated / distinct / depth) | `--compat=tlc --action-composition=tlc` | dive default |
|---|---|---|---|
| `MCCRDT` | 1,350,001 / 25,000 / 16 | 1,350,001 / 25,000 / 16 | 900,001 / 25,000 / 16 |
| `MCReplicatedLog` | 17,425 / 1,363 / 13 | 17,425 / 1,363 / 13 | 11,617 / 1,363 / 13 |

The finite replay controls are in `crates/dive-trace/tests/fixtures/composition/`.

## D-023: Default process exits are distinct conformance observables

| | |
|---|---|
| **Kind** | Same answer; selectable exit convention |
| **Default** | TLC's process statuses, in both profiles |
| **Native option** | `--exit-codes native`: property violations exit 10 and rejected inputs 1 |
| **Status** | implemented |

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --exit-codes native --workers 1 --seed 7 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --workers 1 --seed 7 --fp 0 --output json
```

The same invariant violation exits 10 with the native convention and 12 with
the default TLC convention; both commands use the default evaluation. The exit
status is part of every comparison with TLC, and a finite temporal violation
that TLC reports with 2116 and exit 12 keeps that classification. See
[process statuses](user/checking.md#process-statuses).

## D-024: The Hanoi Bits override gives the one-worker result at any width

| | |
|---|---|
| **Kind** | Same answer, better path |
| **Default** | The exact Examples Hanoi `Bits.class` is replaced by a native signed 32-bit AND, and every worker count gives the one-worker result |
| **TLC behavior** | `-workers 1` |
| **Status** | implemented |

TLC at 4 workers reports 243 distinct states, 907 generated and depth 33 on
Hanoi, while its one-worker result is 233, 768 and depth 32, with the same
`NotSolved` violation (2110, exit 12). dive gives the one-worker result at
every width, with a validated 32-state counterexample. A changed, unreadable
or unknown class refuses by name. See [modules](user/modules.md).

## D-026: Run budgets stop a run with a structured result

TLC runs until it finishes or its JVM runs out of memory. A runaway liveness
check can occupy a machine for hours, and running out of heap prints
`SYSTEM_OUT_OF_MEMORY` and exits 1 or 255 with nothing to act on. dive bounds
every `dive check` by two budgets:

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
get TLC's unbounded behavior, pass `--max-memory none`.

`dive simulate` and `dive dfid` accept the same time and memory flags, with the
same defaults. They report `result: stopped`, the phase and completed progress,
exit 5 and no verdict. Their clock starts after the model loads, and one budget
covers all traces or iterations, including validation; when it runs out,
remaining traces are skipped. See [simulation](user/simulation.md#run-budgets)
and [DFID](user/dfid.md#run-budgets).

`--max-states N|none` and `--max-depth N|none` bound discovered states and BFS
depth (initial states have depth one). Both default to `none`, as in TLC;
TLC's simulation `-depth` does not bound exhaustive search this way. Finite
settings under `--compat=tlc` report `tlc+overrides`. A new state past either
ceiling is not admitted, and the run reports resource `states` or `depth`, the
progress reached, `statistics.exhaustive: false` and exit 5. A finite graph
exactly at the ceiling can still finish normally. With temporal properties the
ceilings stop graph capture the same way, with no counterexample and no holding
result, while a violation proven at a completed level before the ceiling is
still reported. TLC has no such ceiling for liveness checking. See
[state and depth budgets](user/checking.md#state-and-depth-budgets).

## D-028: Canonical finite sets

dive builds finite set literals and mapped sets as mathematical sets:
construction order, repeated elements and TLC's earlier normalization of the
same object do not change their identity. Both orders of
`{<<1>>, [a |-> 1, b |-> 1]}` are accepted in the default profile. This extends
[D-009](#d-009-comparisons-that-tlc-refuses-although-tla-answers-them).
Undefined direct scalar comparisons, such as `1 = "a"`, still report an error.

Canonical enumeration can also reduce generated counts. `NxInLazyCup`
(`r' \in [a : {1}] \cup [a : {1, 2}]`) generates 10 states in TLC and 7 in
dive; both find the same 3 distinct states at depth 2. This is separate from
exploration order ([D-010](#d-010-the-order-in-which-sets-are-explored)).

Symbolic powersets, function sets and sequence sets keep lazy membership and
streaming, so huge or infinite domains are not enumerated. Using a symbolic
definition as an element, function key or stored state can need an identity
dive has not established; such a check refuses, and symbolic structure is
never treated as inequality. The trace validator also refuses it, including in
traces that claim an evaluation error.

### Default outcomes and real errors

The validator builds sets the same way. Mixed-set normalization no longer
hides a mathematical result: `graph-demand/DemandedEdges` is a validated
initial `Inv` violation, and the finite graph, product and projection controls
in the [registry](divergences.md#d-028-canonical-finite-set-construction)
complete with tested counts. Malformed graph fields, non-function arguments to
`AntiFunction` and empty arguments to `LongestCommonPrefix` still report their
errors. dive also reaches the non-set operand error in `UnknownFirstProjection`
and `PrefixUnknownFirst`, and refuses the infinite enumeration in
`PrefixInfiniteFirst`.

Finite bag results use the same canonical construction for their materialized
keys. For instance, a message bag can have a record carrying an integer and
a record carrying a configuration as separate keys. Adding the same message
again increments its count. Direct comparisons during accumulation still use
the selected comparison semantics.

### Compat gaps

Building a set can refuse before an operation TLC would never perform:
`graph-demand/SelfDemand`, `graph-empty-product/LazyEmptyProduct`,
`product-operations/FiniteRawAxisHistory` and
`set-conversion-demand/NormalizedProjectionSkipRaw` refuse with the mixed-shape
comparison gap. `--compat=tlc` keeps TLC's integer and name order but uses the
same canonical sets, and refuses mixed comparison shapes by name instead of
emulating TLC's construction history. `SeqProductSkipRaw`,
`MappedEmptySubsets`, `SeqProductFiniteSkipRaw`, `SeqProductFiniteDemandRaw`
and `MappedMixedSubsets` in `crates/dive-api/tests/fixtures/core-receiver-consumers/`,
and `OrderShortCircuit`, `SizeShortCircuit` and `ExcludedRawState`, are sets
TLC can leave unnormalized that dive refuses under `--compat=tlc`. These gaps
do not change the trace format or allow an unvalidated counterexample.

## D-029: Predicate-witness counts

dive evaluates an applied recursive Boolean function in `Next` as a predicate
when its compiled plan, including indirect calls, has no effects or unknown
call targets. When several existential witnesses prove it true, they do not
each create another copy of the same successor:

```tla
Next == LET f[n \in 0..1] == IF n=0 THEN TRUE ELSE \E k \in {1,2}: f[n-1]
        IN f[1] /\ UNCHANGED x
```

With `Init == x=0`, TLC generates three states (Init plus two identical
successors) and dive two, with the same one distinct state, depth one and
successful verdict. Examples `byzpaxos/VoteProof.cfg` succeeds with 6,962
distinct states and depth 14 in both, generating 27,727 states in dive against
TLC's 318,877. Verdicts, distinct counts, depth, properties, error classes,
exits, queue counts and counterexample validity are unchanged.

In the `OrError.cfg` control, the predicate is already true, so dive does not
evaluate a later `1 \div 0` alternative: it succeeds with 1 distinct and 2
generated states, depth 1, exit 0, where TLC reports division by zero (2179,
exit 255).

`--compat=tlc` uses the same evaluation and does not rebuild TLC's duplicate
witnesses; see [compat gaps](#compat-gaps). Plans with effects (registers,
randomness or output) or unproved calls refuse. Recursive evaluation, including
arguments, polls budgets and refuses beyond 64 active calls instead of
exhausting the stack. Escaping recursive functions, primed applications,
unproved captures, nested recursion and other unsupported contexts refuse by
name.

## D-030: Finite symbolic safety evidence

The [finite symbolic checker](user/symbolic.md) has its own opt-in proof
and search entry point. For example, with `--prove-inductive` it can prove
the receipt-set bound by checking Init and each action. Its report then
contains the proof outcome. TLC and Core ordinarily enumerate reachable
states and report exploration counts.

An exhaustive symbolic search reports generated and distinct counts at its
fixed point. When an invariant fails, it reports an independently validated
shortest counterexample. The failing state can differ from TLC's first state
in the same breadth-first layer. The report omits explicit prefix counts.
Use `dive check` for Core's usual report and checking order.

<a id="d-030-bag-fold-order"></a>

## D-031. Bag fold order

`FoldBag(op, base, bag)` visits keys in canonical order under the default
exploration order, once per occurrence. For example, a decimal-digit fold over
`(3 :> 1) @@ (1 :> 2)` gives 113. Pinned TLC visits 3 first and gives 311.
The order can change the value of a noncommutative fold, a side effect, or
which error is reached first. With `--explore-order=tlc` the checker refuses
when TLC’s historical bag order cannot be recovered.

## D-036: Simulation checks implied initial properties

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Checked on every initial state a simulation keeps, reported as 2108 |
| **TLC behavior** | Not reproduced; documented gap under `--compat=tlc` |
| **Status** | implemented for `dive simulate` |

With `PROPERTY x = 1` on a spec whose initial state has `x = 0`, TLC's
`-simulate` exits 0: it never checks a state-level property in simulation,
while model checking reports error 2108. `dive simulate` checks it on every
initial state, after the invariants and before the constraints as
`dive check` does, and reports the same 2108 with a one-state trace that the
trace validator checks. It does so in both profiles, because skipping a
configured property would report a sample as clean when it is not. The
receipts are `crates/dive-sim/tests/fixtures/temporal/oracle.json` U12 and
its model-checking control B-U12.

## D-041: Constructed initial states in simulation

| | |
|---|---|
| **Kind** | Opt-in option |
| **Default** | Off: Init is enumerated once and every initial state is checked, as in TLC |
| **TLC behavior** | The default, in both profiles |
| **Status** | implemented for `dive simulate --constructive-init` |

TLC's `-simulate` and `-generate` enumerate every initial state before the
first trace, so a spec whose Init has a billion states never starts (G2
enumerates a million). `dive simulate --constructive-init` builds each
trace's initial state with randomized Init choices instead, the way
`--generate` builds successors, and checks it as an enumerated one is
checked. Init is never enumerated, so such a spec starts at once. The cost is
that an initial state that violates an invariant is found only if some trace
constructs it, where enumeration finds it on the first run. When no
alternative of Init gives an admitted state, the result is the same 2118 or
2256 error as with enumeration, because the construction searches every
alternative before giving up.
