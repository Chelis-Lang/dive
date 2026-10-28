# Memoized pure evaluation

Status: M0 spike implemented in `crates/dive-core/src/memo.rs` and in the
parent-delta rule of `crates/dive-explore`; results in `m0-results.md`. M1
decision (§6): memo tables off by default, the parent-delta skip on. This document covers strategy S6 from `01-motivation.md`.

## 1. Idea

TLC evaluates every invariant in full on every new distinct state, and every
action guard on every state it expands (`tlc-baseline.md`). Most of that work
repeats.

- **Few changes per step.** A successor usually changes one or two variables.
- **Few input combinations.** A conjunct such as `pc \in [Procs -> Labels]`
  depends on one variable, which takes far fewer distinct values than there are
  states.

ADR 0104 measured invariant checking at 44–55% of instructions on two
benchmark models.

The new core caches results of pure compiled bodies, keyed by the handles of
their inputs. With hash-consed values (`03-value-model.md`), equal inputs have
equal handles, so a key is a short tuple of integers.

Memoization never changes which expressions TLC would evaluate or in which
order. It only replaces a repeated evaluation by its known result. Enumeration,
and therefore generated counts, are unaffected.

## 2. Two mechanisms

### 2.1 Parent-delta skip, for invariants and state constraints

Every new state has a parent that already passed every invariant; otherwise
the run would have stopped there. So a pure invariant conjunct whose read set
does not intersect the variables the step changed is true, and cannot raise an
error.

- It is skipped with no lookup at all.
- **Order is preserved.** Checking order is unchanged. A skipped conjunct is
  true, so the first violated invariant, and the first error, are the same as
  under full evaluation.
- **Initial states** have no parent and are checked in full.
- **Excluded conjuncts.** Impure and cutoff-sensitive conjuncts (section 4)
  are never skipped.

### 2.2 Keyed result cache, for any pure body

A memo site is a compiled body that the compiler marks as worth caching:
- an invariant conjunct;
- an action guard that reads only unprimed variables;
- the right-hand side of an assignment;
- a large pure subexpression inside a quantifier.

Each site has a key, a result and a cache.

**Key.** The site id, plus the handles of every input in its read set:
- unprimed state variables;
- free local variables bound by enclosing quantifiers or operator parameters;
- for invariants checked on a new state, that state's variables;
- the lazy-view handles of those inputs, where the site's read set can observe
  views (`03-value-model.md` §8).

Inputs must be persistent handles. A site whose inputs include a transient
value is evaluated normally. States in the frontier are interned, so unprimed
inputs are always persistent.

**Result.** One of:
- a persistent handle;
- a boolean;
- an error, with its error-class code and site. A cached error is raised at the
  same point the evaluation would have raised it.

**Cache.** Per worker, with no sharing and therefore no contention.
- Each site has a set-associative table (2-way). It starts with 64 sets.
  It doubles, rehashing its entries, once it has evicted more entries than
  a quarter of its capacity since it last grew, up to 8,192 sets.
- **Memory bound (built).** A worker's tables together stay under a byte
  cap: a sixty-fourth of the run's memory ceiling (`--max-memory`) shared
  by the workers, clamped to between 1 MiB and 64 MiB (`Opts::memo_cap`,
  `core_engine.rs` `memo_cap`). A table that would exceed the cap stops
  growing, and one that cannot be allocated stays empty. Eviction only
  drops a cached result, which the next lookup recomputes, so neither the
  cap nor the eviction changes a result. The unit test
  `tables_grow_within_the_cap_and_never_answer_wrongly` covers caps of 0
  to 64 MiB.
- **Measured** on M3 (load 8–20, so cycles, medians of 2–3; memo on;
  `DIVE_CORE_OPT=+memo` on main `e1cbdbfb` against this change; counts
  identical):

  | Model | 8w RSS before / after (MB) | 8w cycles before / after | 1w cycles before / after |
  |---|---|---|---|
  | MCLamportMutex | 308 / 185 | 30.4 G / 29.2 G | 20.0 G / 20.6 G |
  | MCbtree | 221 / 121 | 10.3 G / 9.7 G | 7.4 G / 7.4 G |
  | MCDisruptor_MPMC | 100 / 33 | 3.0 G / 2.9 G | 1.5 G / 1.5 G |
  | SimpleRegular | 164 / 127 | 7.6 G / 7.5 G | 4.8 G / 4.8 G |

  With the tables off, the 8-worker RSS is 130, 107, 29 and 109 MB.
- Each entry holds the full key, for verification, and the result.
- On a miss, the site is evaluated and its entry inserted.

**Adaptivity.** After 4,096 lookups, a site whose hit rate is below a threshold
(initially 30%) turns itself off. At that hit rate, hashing the key costs more
than evaluating the body. The engine reports per-site statistics with
`--stats`.

### 2.3 Step predicates: thunks and memo sites on the pair

A step predicate (an implied action `[][A]_v`, an action constraint) is
evaluated on a complete pair: while it is evaluated, its primed variables
are as fixed as its unprimed ones. The compiler knows it is compiling one
(`Compiler::step`), and caches more there (`core/engine-step-thunks`):

- **Thunks.** Each zero-arity top-level definition, and each substitution
  used by name (the role's own `INSTANCE` substitutions, and a substitution
  used primed), is a thunk evaluated at most once per evaluation of the
  predicate, primed or not (`Compiler::step_cached`, keyed by
  `Cached::Def` or `Cached::Subst` with the mode). Outside step predicates
  a definition is cached only unprimed and a role substitution not at all:
  the generator's primed variables are still being assigned. A body with a
  bound variable (a free slot) or an effect is not cached. TLC evaluates
  such a body at each use; the value, or the first error, is the same. The
  thunk counts as by-name for the function-application rule
  (`Compiler::def_slots`, `04-plan-compiler.md` §4.1).
- **Memo sites** (`Opts::step_memo`, on by default; `-step-memo` turns
  them off). Each such thunk's body is also a memo site keyed on the
  handles of the variables it reads, unprimed from the source and primed
  from the successor (`MemoSite::primed`). So `token`, a function of
  `network`, is evaluated once per distinct value of `network`, not once
  per step. `-memo` leaves these sites on; `-step-memo` turns only them
  off.
  - A successor's value is transient. The key uses an *exact* persistent
    copy of it (`Heap::intern_exact`): every node keeps its kind and view
    bits, so equal keys are equal representations, and the result cannot
    depend on anything the key does not hold. The successor itself is left
    as it is. Its values are mostly already in the store (a seen state's
    values were interned when it was claimed), so the copies cost little
    memory: 2,500 nodes on EWD998PCal.
  - A result is kept as an exact persistent copy too. A lazy key or result
    is not kept: the site is evaluated as if it had none.
  - A body that only assembles leaves and thunks into a tuple, record or set
    (a `vars` tuple), or that is already a thunk or a site, gets no site:
    keeping a copy of each costs more than rebuilding it. On EWD998PCal a
    site on `EWD998!vars` kept 300,000 tuples, 60 MB.
  - The adaptivity rule of §2.2 applies: a site under 30% hits after 4,096
    lookups turns itself off (`EWD998!Next`, a boolean of the whole pair).
- **Measured** (M3, 1 worker, indicative, load 6–7). EWD998PCal with its
  refinement `PROPERTY` (`EWD998!Spec` through `INSTANCE`, with `token` and
  `pending` substituted by definitions): 13.96 s user and 21 MB before;
  thunks alone 11.47 s; thunks and sites 5.5 s and 34 MB. The safety part
  alone takes 2.0 s; the reference engine takes 53.9 s and 113 MB. The
  sites `token` and `pending`, unprimed and primed, hit 99.7%. Counts are
  identical in every run. The ablation over the corpus models with step
  predicates is in the PR.

Fixtures `StepCache` (holds) and `StepCacheBad` (violated, with a cached
definition read at both ends of the step) in `core-m1`, through
`StepAbs`. Keying the sites on the unprimed variables only makes both
fail.

## 3. Read-set analysis

The compiler computes read sets during the pass that records reads and purity
(`04-plan-compiler.md` §3, pass 6):

- every state variable read, primed or unprimed;
- every free local;
- every constant that is not folded.

Calls to operators that are not inlined contribute their callees' read sets.
Recursive operators contribute a fixed point.

A read set that mentions a whole variable is conservative. A body that reads
only `pc[i]` still keys on all of `pc`, plus `i`. Finer keys that read a field
or an index are a later refinement, measured before adoption.

## 4. Soundness conditions

A body is eligible only if all of the following hold.

1. **No effects.** It does not reach `TLCGet`, `TLCSet`, random draws,
   `Print`, `PrintT`, assertions with output, or I/O. Assertion failures are
   errors and may be cached, because they are deterministic.
2. **No cutoff sensitivity.** It does not depend on `ENABLED` cutoff
   semantics (B3).
   - **Under `--compat=tlc`.** Neither is it reachable at a site where TLC's
     lazy cache reuse is stale (B2, B4); the `lazy-enabled-cache` corpus shows
     the result can then depend on context.
   - **In the default profile.** Those sites evaluate soundly
     (`02-semantic-contract.md` §3a), so they may be memoized like any other
     pure body.
3. **No normalization side effects.** Its result does not depend on the
   normalization state of an input whose lazy view is unnormalized, and it
   does not change such a state. In TLC, normalizing a set can change the
   outcome of a later membership test. Bodies that perform membership on such
   inputs are not memoized while those inputs are unnormalized.
4. **No provisional names.** Under `--compat=tlc`, it compares no runtime name
   whose token is still provisional (`07-engine.md` §6).
5. **Determinism.** Its result depends only on its inputs, the profile and the
   constants. Every TLA+ operator is deterministic given TLC's order,
   including `CHOOSE`, so this holds once 1–4 hold.

How these conditions relate to TLC's own caching
(`02-semantic-contract.md` §3(b)):

- **Stale reuse (B2, B4).** TLC's `LazyValue` cache reuse is sometimes stale:
  `isSubset` ignores hidden `INSTANCE` variables.
  - **Under `--compat=tlc`.** Condition 2 excludes those sites. They keep real
    thunks with TLC's cache key, as `04-plan-compiler.md` §4 requires.
  - **In the default profile.** They are evaluated soundly and may be memoized
    (`02-semantic-contract.md` §3a).
- **Cached errors (B7).** TLC never caches a failing thunk; it re-evaluates in
  the caller's context. Caching errors is allowed in both profiles. A memo
  entry may record an error only because its key covers every input of the
  body. A different context is therefore a different
  key, and an equal key re-evaluates to the same error. Shadow comparison must
  confirm this on the error fixtures (`ErInv*`, `ErrMemNormalized`).

A memoized result must equal the result evaluation would produce at that
point, including errors and their codes. This is checked by:
- shadow comparison with memoization switched on against the reference engine;
- an ablation run with memoization switched off, comparing every observable
  (`08-correctness.md`).

## 5. Interaction with other parts

- **Successor generation.** Guard memoization short-circuits a disjunct whose
  guard is false. The successor enumeration and its counts are unchanged.
- **Parallel workers.** Each worker has its own caches. Hit rates are lower
  than with one shared cache, but there is no contention. A shared, read-mostly
  cache is a later option, measured first.
- **Compaction.** Compacting the persistent store remaps handles, so it clears
  all caches (`03-value-model.md` §10).
- **Errors and replay.** When an error is reported, the call-stack replay
  re-evaluates with memoization off, so the replay's observable effect on
  counts is unchanged (`02-semantic-contract.md`).

## 6. Next step

M0 implements both mechanisms:
- keyed memo tables for invariant and constraint conjuncts and action guards,
  keyed on the handles of the variables and free slots they read. Each table is
  per worker, 2-way and has 8,192 sets. A site turns itself off when its hit
  rate is under 30% after 4,096 lookups;
- the parent-delta skip of invariant and constraint conjuncts whose read set
  does not meet the variables a step changed.

`DIVE_CORE_OPT=-memo` and `-parent-skip` turn each off. `m0-results.md` gives
hit rates, the share skipped and the slowdown when each is off.

**M1 decision (stream 2), revised: the memo tables are on by default**
(`DIVE_CORE_OPT=-memo` turns them off). The first M1 decision turned them
off on wall-clock medians at load 10–22 (below). The quiet x86 A/B
reverses it, and so do the M3 instruction counts.
- **Measurement of record: quiet x86** (lead; c7i.metal-24xl, SMT and turbo
  off, CPUs 0–15; main `fdaf57a2`; core with `DIVE_CORE_OPT=+memo` against
  `-memo`; wall seconds, median of 5; every count equals its golden):

  | Model | 1w off | 1w on | Speedup | 8w off | 8w on | Speedup | RSS 8w off / on (MB) |
  |---|---|---|---|---|---|---|---|
  | CoffeeCan1000BeansSafety | 0.97 | 0.97 | 1.00 | 0.67 | 0.66 | 1.01 | 212 / 208 |
  | EWD998PCal | 27.78 | 27.91 | 1.00 | 4.86 | 4.76 | 1.02 | 160 / 230 |
  | GameOfLife | 8.12 | 8.12 | 1.00 | 1.64 | 1.65 | 1.00 | 151 / 147 |
  | MCBakery | 8.47 | 6.11 | 1.39 | 6.61 | 4.66 | 1.42 | 242 / 330 |
  | MCDisruptor_MPMC | 1.47 | 0.87 | 1.69 | 0.44 | 0.34 | 1.28 | 84 / 172 |
  | MCLamportMutex | 12.23 | 9.54 | 1.28 | 2.50 | 2.57 | 0.97 | 184 / 419 |
  | MCbtree | 5.11 | 4.35 | 1.17 | 3.67 | 1.07 | 3.43 | 167 / 333 |
  | SimpleRegular | 2.95 | 2.72 | 1.08 | 0.72 | 0.72 | 1.00 | 176 / 258 |
  | TLCSailfish1 | 44.18 | 43.21 | 1.02 | 7.86 | 7.81 | 1.01 | 182 / 217 |

  Geomean speedup with the tables on: 1.16 at 1 worker and 1.23 at 8. The
  M0 ablation covered four models and missed this.
- **Next: bound the tables.** They roughly double the 8-worker RSS of
  several models (MCLamportMutex 184 to 419 MB). The per-worker tables need
  a size cap that follows the run's memory budget, with an eviction that
  never changes a result.
- **Instructions retired, M3, at `0abc2844`, 1 worker, off / on:**

  | Model | Instructions off / on | Cycles off / on | RSS off / on (MB) |
  |---|---|---|---|
  | MCLamportMutex | 114.2 G / 81.5 G | 24.5 G / 24.5 G | 54 / 86 |
  | SimpleRegular | 26.6 G / 24.9 G | 5.3 G / 4.8 G | 40 / 47 |
  | GameOfLife | 113.5 G / 113.6 G | 23.7 G / 24.3 G | 87 / 84 |
  | CoffeeCan1000BeansSafety | 9.2 G / 9.5 G | 2.6 G / 2.6 G | 145 / 146 |

  The counts are identical in every run.
- **x86 bench (lead, `f1ed932a`).** One-worker MCLamportMutex went from
  10.47 s at M0 to 12.53 s. The bisection by instruction count puts 13.4 G
  of the 24.5 G it gained since M0 on the memo-off commit (`0bff25aa`).
- **Cost.** The tables add their memory: about 32 MB on MCLamportMutex.
  Finer keys (below) would cut that.

The first decision's evidence, kept for the record:
- **x86 (M0 ablation, `m0-results.md` §3).** `-memo` gives 0.96 on CoffeeCan,
  0.97 on SimpleRegular, 1.04 on MCLamportMutex and 1.00 on GameOfLife. No
  model gains more than noise.
- **M3, M1 branch.** Medians of 3 alternating runs, user seconds, at load
  10–22, so indicative:

  | Model | memo on | memo off | RSS on / off (MB) |
  |---|---|---|---|
  | CoffeeCan1000BeansSafety | 0.71 | 0.58 | 213 / 213 |
  | MCLamportMutex | 9.42 | 7.65 | 76 / 50 |
  | GameOfLife | 7.16 | 6.49 | 83 / 83 |
  | EWD998PCal (safety) | 2.14 | 2.14 | 30 / 23 |

  The counts are identical in every run.
- **Reading (superseded).** Where states share sub-values, hash-consing and the
  parent-delta skip already remove the work the tables would save. The tables
  then cost a lookup per site visit and their memory: 26 MB on
  MCLamportMutex, 8 MB on EWD998PCal.
- **Next.** Keys on sub-values (`f[p]` instead of all of `f`), once the
  read-set analysis can name them. Then a guard on one process's slot would
  hit across states that differ elsewhere. Also tables sized from each
  site's observed key diversity, which would cut their memory. Revisit with
  the M2 benchmark campaign (`09-performance-method.md`), measuring
  instructions as well as time.

The parent-delta skip stays on: it costs nothing measurable and is exact.
