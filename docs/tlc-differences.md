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
| **Status** | planned (M5 architecture requirement) |

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

**Evidence.** Probe P8 in `docs/survey/competitive.md`, ADR 0004, ADR 0010, and the harness worker-invariance reports in `docs/conformance.md`.

---

## D-002: Randomness is always replayable

| | |
|---|---|
| **Kind** | Same answer, better path (reporting); different answer only when you fix a seed |
| **Default** | Fresh seed per run, as in TLC, but **the seed is always reported**; `--seed S` replays a run exactly |
| **TLC behaviour** | TLC's seeding, under `--compat=tlc` |
| **Status** | planned |

**What TLC does.** `RandomElement`, `RandomSubset` and the `Randomization` module draw from an unseeded random source during model checking unless you pass `-seed`. A spec that uses them can produce a different state space on every run.

| Spec | What we measured on the pinned TLC |
|---|---|
| Examples `SpanningTree/SpanTreeRandom` | 1,878 / 40,460 / 65,396 / 106,664 / 240,806 distinct states across runs with identical flags |
| `RandomElement(1..1000000)` (probe P9) | 599,378, then 344,064, then 43,074 |

The seed is not shown in the normal results, so a surprising run cannot easily be reproduced.

**What dive does.** dive also draws a fresh seed by default, and always reports it in both human and JSON output. `--seed S` reproduces the run exactly, at any worker count. A dive run that cannot be reproduced from its reported inputs is a bug.

**Evidence.** Harness review finding M5, probe P9, ADR 0010 §4.

---

## D-003: Simulation is reproducible

| | |
|---|---|
| **Kind** | Different answer only when you fix a seed |
| **Default** | Seed reported; `--seed S` gives bit-identical simulation at any worker count |
| **TLC behaviour** | Under `--compat=tlc` |
| **Status** | planned |

**What TLC does.** Even with `-seed 0` and one worker, the number of states TLC generates in simulation varies between runs. On Examples `EWD840_anim` it ranged from about 267k to 271k.

**What dive does.** The same seed gives the same simulation, at any worker count. This makes simulation usable in tests and in CI.

**Evidence.** Harness notes in `crates/dive-conformance/README.md` and the registry in `docs/divergences.md`.

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

## D-005: Symmetry with liveness checking is refused

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | Refuse the combination, with an explicit diagnostic |
| **TLC behaviour** | `--symmetry-liveness=tlc` (included in `--compat=tlc`): warn and proceed |
| **Status** | planned (with `dive-liveness`) |

**What TLC does.** Symmetry reduction lets TLC explore only one representative of each group of states that differ only by a permutation of interchangeable values, such as process ids. Combined with liveness checking ("something good eventually happens"), symmetry can make TLC miss real violations. TLC knows this. It prints a warning ("Declaring symmetry during liveness checking is dangerous. It might cause TLC to miss violations…", `EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`), but then carries on and can report success.

**What dive does.** By default dive refuses to give a verdict for this combination. It explains why, and suggests how to proceed: drop the symmetry for the liveness run, or opt into TLC's behaviour knowingly. "No error found" should never be an answer that the tool itself suspects may be wrong.

**Evidence.** ADR 0010 (owner decision of 2026-10-08), probe P4 (another checker raised a false liveness alarm under symmetry).

---

## D-006: `CHOOSE` does not depend on unrelated text in the module

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | A canonical order, independent of the module text |
| **TLC behaviour** | `--choose-order=tlc` (included in `--compat=tlc`): TLC's order, including string interning order |
| **Status** | implemented in the value layer (`dive-value`, ADR 0012), in both orders. `--choose-order=tlc` also needs the parser to record names in TLC's encounter order; that part is pending. |

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
| **Status** | planned (M5) |

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
| **Status** | `dive parse` and `dive analyze` implemented; `check` and the rest planned |

**What TLC does.** Results come as human-oriented text, or as the `-tool` message stream meant for the Toolbox. Counterexamples are available as JSON only through `-dumpTrace json`. SANY's error messages give a location, but no stable error codes and no suggested fix. SANY's command line exits 0 on semantic errors unless you pass `-error-codes`.

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

## D-009: Comparisons that TLC refuses although TLA+ answers them (proposal)

| | |
|---|---|
| **Kind** | Different answer |
| **Default** | TLC's comparability rules in both profiles. They are exact under `--compat=tlc`; the default profile differs in one corner, described below. The proposal below is **not adopted**. |
| **Proposed option** | `--comparisons=tla`: answer `=` and `\in` between functions, records, tuples and sets as TLA+ defines them |
| **TLC behaviour** | `--comparisons=tlc`, part of `--compat=tlc` |
| **Status** | proposed (open question 12). TLC's behaviour is implemented and matched in `dive-value`. |

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

## Under consideration

These are recorded candidates. They are not decided, and nothing in dive behaves differently for them yet.

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
- **Configurations embedded in a `.tla` file.** TLC can read a configuration from a `---- CONFIG Name ----` section of a `.tla` file. `dive parse --config X.tla` refuses this as `unsupported`.

---

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
