# Memoized pure evaluation

Status: design. There is no code yet. This document covers strategy S6 from
`01-motivation.md`.

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
- Each site has a set-associative table (2-way, initially 2^14 entries,
  resized within a per-worker memory budget).
- Each entry holds the full key, for verification, and the result.
- On a miss, the site is evaluated and its entry inserted.

**Adaptivity.** After 4,096 lookups, a site whose hit rate is below a threshold
(initially 30%) turns itself off. At that hit rate, hashing the key costs more
than evaluating the body. The engine reports per-site statistics with
`--stats`.

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
2. **No cutoff sensitivity.** It is not evaluated under `ENABLED` and does not
   depend on its cutoff semantics or on lazy cache reuse across contexts. The
   `lazy-enabled-cache` corpus shows the result can depend on context; see
   `02-semantic-contract.md`.
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

- **Stale reuse (B2).** TLC's `LazyValue` cache reuse is sometimes stale on
  purpose: `isSubset` ignores hidden `INSTANCE` variables. A sound memo would
  give a different answer at such sites. Condition 2 therefore excludes every
  body reachable under `ENABLED` or behind an `INSTANCE` with hidden variables
  (B2, B3). Those sites keep real thunks with TLC's cache key, as
  `04-plan-compiler.md` §4 requires.
- **Cached errors (B7).** TLC never caches a failing thunk; it re-evaluates in
  the caller's context. A memo entry may record an error only because its key
  covers every input of the body. A different context is therefore a different
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

M0 implements both mechanisms for invariant conjuncts and action guards on the
spike models, with one ablation switch each. It records:
- hit rates per site;
- the share of conjunct evaluations skipped by the parent-delta rule;
- wall-time contribution, measured with each switch off.
