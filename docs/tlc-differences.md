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
| **Status** | planned. The default switches once both modes pass the conformance harness. |

**What TLC does.** TLA+ defines integers as the mathematical integers, but TLC stores them in 32 bits. Anything above 2,147,483,647 stops the run:

```tla
VARIABLE x
Init == x = 2147483646
Next == x' = x + 1
```

TLC reports an error ("can't handle a number this big") as soon as `x` would exceed 2^31−1. Integer literals above 2^31−1 are also rejected.

**What dive does.** dive computes with unbounded integers, as the language intends. Small numbers still use a fast path. When a run would have stopped with TLC's overflow error, dive's output says so, so the difference is never silent.

**Why it matters.** Four of the five other Rust or Go TLA+ checkers we measured widen integers silently, with no TLC mode and no indication in their output. dive does better by default and is TLC-exact on request.

**Evidence.** Probe P1, harness fixture `Overflow`, ADR 0010 (owner decision of 2026-10-08).

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
| **Status** | planned (`dive-value`) |

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

TLC also orders non-empty sets by size before content, and tuples by length before content.

**What dive does.** dive uses a canonical order that does not depend on the module text. Reordering or adding definitions never changes a result. This is one of the charter's metamorphic guarantees.

**Evidence.** Probes P2 and P7 in `docs/survey/competitive.md`, ADR 0010 (owner decision of 2026-10-08).

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
| **Status** | `dive parse` implemented; `check` and the rest planned |

**What TLC does.** Results come as human-oriented text, or as the `-tool` message stream meant for the Toolbox. Counterexamples are available as JSON only through `-dumpTrace json`. SANY's error messages give a location, but no stable error codes and no suggested fix. SANY's command line exits 0 on semantic errors unless you pass `-error-codes`.

**What dive does.** Every diagnostic has:

- a stable code (for example `DIVE-S0010`);
- a precise source span;
- a message that names the construct;
- a likely fix, where we know one.

`--output json` gives verdicts, statistics and counterexamples as structured state sequences, along with diagnostics and an `options` block that records the active profile, options and seed. Coding agents and Rust callers can act on this programmatically. The library API (`dive-api`) gives the same results in-process, with no JVM.

**Evidence.** ADR 0009, `dive parse --output json`, and probes M1 and M2 (diagnostic comparison in `docs/survey/competitive.md`).

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
- **String length in UTF-16 units.** TLC's `Len`, `SubSeq` and `Tail` on strings count UTF-16 code units, not characters (tlaplus#1076). See `docs/survey/oracle-history.md`.

## Where dive is currently more limited than TLC

dive refuses rather than approximates (soundness contract rule 3). These are honest refusals, and we track them here so that nobody mistakes them for features.

- **Extremely deep nesting** (`DIVE-S0040`). dive refuses expressions nested more than 256 levels deep, and syntax trees more than 2,048 levels high (for example a chain of thousands of `+`). SANY accepts these. No module in the corpus comes close. Removing the limit is a tracked follow-up.

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
