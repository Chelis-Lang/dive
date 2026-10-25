# 0104: Evaluator performance plan (perf phase B)

- **Status:** proposed
- **Date:** 2026-10-19

## Context

ADR 0006 targets a geometric-mean speed-up of at least 2x over TLC, and at
least 1x on every benchmark model. Before ADR 0102 dive ran at about 0.22x TLC
at one worker; ADR 0102 (fat LTO, mimalloc) removed 15–17% of instructions.
ADR 0014 §1 planned a specialized evaluation plan (resolved slots, typed
variable reads, fast paths). Inspection on 2026-10-21 found that the shared IR
already has resolved local slots and typed variable IDs; `Core::eval_inner`
interprets borrowed IR without cloning each node. A separate evaluator-private
dispatch plan remains an unproven optimization, rather than missing resolution.

This ADR records the original measurements and proposed changes. Its numeric
cost tables describe the pinned historical build below. Later implementation
checks and complete measurements qualify the remaining opportunities; historical
shares and estimated savings do not establish a current optimization.

### Method

- Build: the `profiling` profile (release settings of ADR 0102 plus debug
  info), from `main` at `cfec92ba`.
- Runs: `dive check --seed 0 --fp 0 --workers 1`, in the default profile unless
  stated, under `valgrind --tool=callgrind` (profiles) or
  `--tool=cachegrind --cache-sim=no` (counts), valgrind 3.27. The machine is a
  shared 32-thread host under load 25–30, so instruction counts are the primary
  metric. They are deterministic: repeated runs of one binary agree to better
  than 0.001%.
- Models: three `dive-bench` models at their pinned configurations, and four
  reduced configurations for A/B comparisons that finish in minutes under
  valgrind: SimpleRegular with `N = 6`, MCbtree with `MaxKey = 3`, GameOfLife
  with `N = 3`, and EWD998PCal with `counter[i] < 2` in `StateConstraint`.
- Caveat: with fat LTO and one codegen unit, an unrelated change can move a
  model's count by 1–2.5% through inlining and layout (measured below). A
  change is therefore judged on every model in both profiles, and needs at
  least 5% to count as a structural win.

### Where the time goes

Instructions per distinct state, at the pinned configurations: SimpleRegular
397 k (110.3 G in total), MCbtree 1.28 M (478.3 G), GameOfLife 5.4 M
(353.4 G). The reduced EWD998PCal spends 2.7 M per state (56.3 G).

Inclusive cost by phase, as a share of all instructions (callgrind; phases
overlap where one calls another):

| Phase | SimpleRegular | MCbtree | GameOfLife | EWD998PCal (reduced) |
|---|---|---|---|---|
| Successor generation (`Evaluator::next_states`) | 49.8% | 44.6% | 99.9% | 99.5% |
| Invariants and implied actions (`engine::holds`) | 44.3% | 54.7% | 0% | 87.7% |
| State key and fingerprint (`state_key_of`) | 8.1% | 2.2% | 0.2% | 0.9% |
| Fingerprint-set insert | < 0.1% | < 0.1% | < 0.1% | < 0.1% |
| Freeing values (`drop_glue::<Value>`) | 10.9% | 9.8% | 9.5% | 9.9% |
| Freeing `Env` / `Sv` / `St` | 1.0 / 0.6 / 0.9% | 2.3 / 2.4 / 0.7% | 7.8 / 8.3 / 0.4% | 2.9 / 3.3 / 0.9% |
| Allocator (`mi_malloc_aligned`, `mi_free`) | 8.9% | 10.7% | 11.1% | 11.5% |
| Set membership (`Core::member`) | 19.6% | 27.0% | < 0.1% | 1.1% |
| Default TLA+ equality (`tla_cmp::equals`) | 8.7% | 14.9% | 5.5% | 1.6% |
| Set normalization (`normalize_set`) | 1.7% | 8.2% | 7.4% | 4.4% |
| `EXCEPT` (`Core::eval_except`) | 14.4% | 2.3% | 0% | 12.8% |
| Forcing lazy values (`Core::force`) | 0.5% | 26.9% | 45.3% | 52.8% |
| `INSTANCE` substitutions (`Core::eval_subst_ref`) | 0% | 0% | 0% | 52.6% |

The EWD998PCal run uses the reduced configuration (callgrind on the pinned one
would take hours); its implied action is checked inside the successor sink, so
`holds` nests inside generation there.

In that historical profile, exclusive cost is flat: no function exceeds 7%.
The interpreter's own
dispatch is the largest exclusive item: the bodies of `Core::eval` and
`Core::eval_inner`, their prologues and the moves of `Result<Ev>` take 7–8% of
SimpleRegular, about 12% of MCbtree and 12–16% of GameOfLife. Most of the rest
is constructing, comparing and freeing values and activation records.

Successor storage is not a bottleneck: the fingerprint set's insert is
negligible, and fingerprinting is at most 8%.

## Decision

Optimize in this order. Each change must keep every verdict, distinct and
generated count, depth, fingerprint and successor order (ADR 0086), so none is
an observable divergence and none needs a `--compat=tlc` setting. Each lands as
its own PR with before/after instruction counts on the benchmark models in both
profiles, and the small conformance tier unchanged against the committed
baselines.

1. **Free values without allocating.** `Drop` for sets and functions collected
   every child into a heap work list, so freeing even `{0, 1}` allocated.
   Children that are scalars or shared are now released in place, nested
   values are freed by bounded native recursion, and only deeper nesting uses
   the work list. Measured: 3.3–6.9% fewer instructions on the reduced models
   in both profiles (geometric mean 5.1%); on the pinned configurations,
   5.8% for SimpleRegular and 4.2% for GameOfLife (PR #329).

2. **Activation records.** The original profile motivated smaller slots,
   inline frames and borrowed reads. Inspection at `d1eb36e3` on 2026-10-21
   confirms that `Sv` is already 24 bytes, frames retain four inline slots,
   local reads borrow their slot, and IR dispatch borrows its expression.
   These are implemented premises, not further savings to claim. Frame
   allocation and persistent context ownership remain measurable costs;
   `next::Gen::eval` still clones `Env` and `St`, but lazy captures and
   rebinding require persistence. Profile those costs before changing that
   ownership model. The historical 3–8% estimate does not establish a new
   optimization's expected saving.

3. **Specialized dispatch from ADR 0014 §1.** Local slots and typed state
   reads already exist in the shared IR. Measure evaluator-private dispatch
   before adding another plan representation, and consider small specialized
   nodes for integer arithmetic and comparison, constant-key
   function and record access, and boolean junctions that return `bool`.
   Target: the 7–16% of dispatch overhead above, plus variable lookup
   (`state_variable`, about 0.9%). Expected saving 5–10%; it also unblocks a
   closure-compiled backend, which ADR 0014 admits only at 1.3x over the
   interpreter.

4. **Membership and equality fast paths for type invariants.** Invariants such
   as `x \in [D -> SUBSET S]` and record-set membership dominate
   SimpleRegular (member 19.6%) and MCbtree (27.0%), and default equality
   another 9–15%. Planned: membership of an enumerated function in a function
   set by domain identity and per-value membership without building the
   domain set; equality that stops at shared or hash-unequal values before a
   structural walk. Every case must keep TLC's errors (incomparable values and
   unnormalized scans, fixture `ErrMemNormalized`). Expected saving 5–10% on
   invariant-heavy models.

5. **Normalization state as TLC keeps it.** TLC normalizes a set in place when
   it enumerates it in order (`CHOOSE`, `elements()`), and a difference or
   intersection then inherits that state. At `d1eb36e3`, dive's
   `Set::elements` already marks/caches normalization, and difference and
   intersection already inherit the left operand's normalization state.
   The historical 3–7% estimate therefore does not identify missing
   transitions. Profile remaining repeated normalization, and add a pinned
   oracle probe before changing any transition: normalization remains
   observable through TLC's errors.

6. **`INSTANCE` substitutions.** In the reduced EWD998PCal, 52.6% of all
   instructions are under `Core::eval_subst_ref`: each reference to an
   instantiated variable re-evaluates its substitution while the refinement's
   implied action is checked. TLC binds substitutions as `LazyValue`s in the
   instance's context and reuses a forced value under the
   `LazyValue.getCachedValue` rule (ADR 0014 §2). First confirm with a probe
   how often TLC re-evaluates them here; then reuse a forced substitution
   exactly where TLC would. This is the slowest benchmark model, so the
   expected gain there is large (up to half its instructions), and it bears
   directly on ADR 0006's "at least 1x on every model".

A fresh frozen-main profile at `c542d442` and a bounded hash-demand check
qualify the historical cost estimates; see the
[performance guide](../developer/performance.md#frozen-main-callgrind-attribution-2026-10-24)
and [compact record](../survey/data/current-main-profile-2026-10-25.json).
Hot exclusive owners and nested inclusive arcs do not themselves establish
removable work. No defensible 5% hash-deferral case was established; that
hypothesis was rejected before implementation.

### Substitution-cache probe, 2026-10-21

A two-state paired probe against pinned TLC commit `94d0c507` checks
`INSTANCE Inner WITH a <- Print(<<"force",x>>,x)` with `Inner` defining
`Repeated == a=a` and `PrimedRepeated == a'=a'`. From `x=0`, the next action
sets `x'=IF x<1 THEN x+1 ELSE x`; each property is the corresponding action
under `[][ ... ]_x`. Both TLC and dive complete with two distinct states,
three generated states and depth two. Both print twice for `Repeated`
(one substitution evaluation per edge) and four times for `PrimedRepeated`
(two per edge). The compact reproducer and qualification are in
[the performance guide](../developer/performance.md#substitution-cache-probe).

`Core::force` already caches the unprimed thunk using the state/control/cache
domain rule. `Tool.java`'s LazyValue branch explicitly bypasses the cache when
`s1 == null`; `Core::eval_subst_ref` and `Core::force` match that primed path.
Thus item 6's 52.6% inclusive cost does not establish missing memoization:
primed work may be required by the pinned oracle. No speedup is claimed, and
caching beyond the existing rule is not authorized by this evidence. Profile
cache misses separately before attempting a narrower substitution optimization;
investigate activation/context costs and specialized dispatch without
duplicating the existing resolution. The first dispatch and operand-storage
experiments did not meet the acceptance bar; see the performance guide.

Measured and not pursued:

- Boxing cold `ValueError` details reduced hot result layouts but increased
  instructions by 0.301% geometrically across the complete eight-cell matrix.
  Four-byte fingerprint extension and a scalar traversal shortcut increased
  them by 0.473%. All arms completed with identical counts and depth; both
  runtime candidates were rejected. A later inline settled-leaf experiment
  increased instructions by 2.263% geometrically, with all eight cells worse;
  it was also rejected. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links the compact paired measurements, exact inputs and startup tradeoffs.

- Native unary/binary operand arrays and a compact dispatch prototype: the
  four reduced GameOfLife/SimpleRegular profile comparisons saved only
  0.83–1.76% of instructions. Counts, verdict and depth matched, but neither
  experiment met the 5% bar; no runtime change was landed. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  records the factorized comparison and its limited population.

- `EXCEPT` replacing the value in place, without renormalizing the unchanged
  domain: 2.7% on SimpleRegular, 1.1% on EWD998PCal, nothing on MCbtree or
  GameOfLife. Below the bar on its own; reconsider inside item 3.
- An O(1) hash update for `EXCEPT` through a stored sum of entry hashes: the
  extra 8 bytes per function cost 1.3–2.5% on MCbtree and GameOfLife, more
  than it saved.
- Reusing uniquely owned inline activation frames through a bounded thread-local
  pool increased instructions by 0.795% geometrically across all eight reduced
  model/profile cells. All sixteen arms completed with identical counts and
  depth; every cell was worse, and default-profile MCbtree exceeded the 1%
  regression ceiling. The runtime prototype was rejected and its conditional
  full gate was skipped. The [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links the compact measurement and exact producer/input identities.
- Reading settled leaf operands at consuming operations, an inline buffer for
  two operands and a successful bounded integer arithmetic path lowered
  instructions by 1.787% geometrically across the eight reduced model/profile
  cells. All sixteen arms preserved verdicts, counts and depth, and all cells
  improved, but the complete screen missed the 5% admission bar. It was rejected
  without a full gate or landed runtime change. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links exact source, input and producer identities and all sixteen profile hashes.
- A private predicate-region plan with an explicit continuation loop and Boolean
  result channel increased instructions by 10.234% geometrically across all eight
  reduced model/profile cells (+7.741% to +12.958%). All sixteen arms preserved
  verdicts, counts and depth. The complete screen rejected the runtime candidate
  and skipped its conditional full gate; no runtime change was landed. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links actual source/binary identities and all sixteen original profile hashes.
- Recursive Boolean-result consumers with an unchanged generic-dispatch fallback
  increased instructions by 0.501% geometrically across all eight reduced
  model/profile cells (+0.230% to +0.811%). All sixteen arms preserved verdicts,
  counts and depth. The complete screen rejected the candidate and skipped its
  conditional full gate; no runtime change was landed. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links actual source/binary identities and all sixteen original profile hashes.
- Private action-context continuations with borrowed action segments lowered
  instructions by 2.414% geometrically across the eight reduced model/profile
  cells, below the 5% admission bar. GameOfLife and EWD998PCal regressed in both
  profiles (four cells, +1.569% to +2.150%), exceeding the individual 1% ceiling.
  All sixteen arms completed uncensored with identical verdicts, counts and
  depth. The completed screen was rejected; no full gate ran and no runtime
  change was delivered. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links exact source, producer, input and original profile identities. This
  negative result does not establish wall-clock, throughput or scaling behavior.

- A private invocation-session evaluator with explicit call/force, substitution,
  quantifier-row and raw-logic continuations increased whole-CLI instructions by
  73.271% geometrically across all eight reduced model/profile cells (+55.216%
  to +90.641%). All sixteen arms completed normally with identical verdicts and
  all five counters. Both the 5% geometric reduction bar and 1% per-cell ceiling
  failed; the runtime candidate remains unmerged without a runtime full gate,
  repeat screen, retuning or threshold waiver. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links the compact record and original source, binary and profile hashes.

- Ready eager explicit-set payloads and checked same-order domain-key views
  reduced whole-CLI instructions geometrically by 2.018% across all eight
  reduced model/profile cells (0.251% to 5.071% fewer instructions per cell).
  All sixteen arms completed normally with identical verdicts and all five
  counters. The 1% per-cell regression ceiling passed, but the fixed 5%
  geometric reduction bar failed. The complete screen was rejected; no
  conditional runtime full gate, runtime PR, repeat screen, retuning or
  threshold waiver follows. The
  [performance guide](../developer/performance.md#discarded-evaluator-prototypes)
  links the compact record and exact source, binary, input and profile hashes.

- Scoped resolved-call ownership over the original recursive interpreter
  increased whole-CLI instructions geometrically by 2.021% across all eight
  reduced model/profile cells (+0.479% to +3.648%). All sixteen arms completed
  normally with identical success verdicts and all five counters. Six cells
  exceeded the 1% regression ceiling, and the fixed 5% geometric reduction bar
  also failed. The complete screen was rejected without a conditional runtime
  full gate, runtime PR, repeat, retuning or threshold waiver. The
  [performance guide](../developer/performance.md#resolved-recursive-call-ownership-2026-10-25)
  links the compact source, binary, input, profile and observation evidence.

- Pre-sizing the fingerprint traversal's work list: within ±0.3%, noise.
- A lazy function hash: it would make hashing recursive and break the
  documented stack safety for deeply nested values (`tests/limits.rs`).
- Calling `mi_malloc` directly for small alignments: the `mimalloc` crate
  routes every allocation through `mi_malloc_aligned` (about 1–2% of
  exclusive cost on its slower paths), but a custom `GlobalAlloc` needs
  `unsafe`, which the workspace forbids without an owner-approved ADR.

A later frozen-main caller profile at `1a8343b9` retains complete source-PC and
caller-context ledgers for all eight observations. The mixed allocator and
ownership share does not prove removable work. A query of existing EXCEPT
contexts did not justify a 5% candidate; selected exclusive costs and overlapping
immediate arcs remain separate. See the [caller-attribution record](../survey/data/instruction-caller-attribution-2026-10-25.json).

A later private eight-cell function-membership census recorded no qualified repeat
successes. Provider-excluded MCbtree work remains unknown; GameOfLife had only
first successes. This does not establish a safe hit rate or saved instructions,
and no membership cache is admitted. See the
[census record](../survey/data/function-membership-census-2026-10-25.json).

## Consequences

- The plan is ordered by measured share; re-profile after each item, because
  the shares move.
- Items 2 and 3 change `dive-eval` internals only. Item 4 touches
  `dive-value` and `tla_cmp`, and item 5 the normalization rules, which are
  observable through TLC's errors; both need oracle probes first.
- The reduced configurations above are A/B tools, not benchmarks. ADR 0006's
  claims rest on `dive-bench` runs of the pinned models against TLC.
- Items 2 and 6 may be reordered once item 6's probe shows how much of
  EWD998PCal's substitution work TLC itself repeats.
