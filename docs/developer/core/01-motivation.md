# Motivation: why a new core, and where its speed comes from

Status: current as of `main` at `625622c1` (2026-10-26).

## 1. Measurements

### 1.1 Headline

Measurements below were taken on an Apple M3 (8 cores) with MCLamportMutex
(2,729,079 generated and 724,274 distinct states, depth 61; the counts are
identical in every run). Settings: default profile, `--fp 0 --seed 0`. The host
had about 2.6 cores busy with other processes, so timings are indicative.

| | TLC | dive |
|---|---|---|
| 1 worker | 12.0 s | 19.4 s (0.62×) |
| 8 workers | 4.7 s | 10.2 s (0.46×) |
| Scaling from 1 to 8 workers | 2.55× | 1.9× |

A `dive-bench` run on the same host and commit, over 7 models with 2 repetitions,
gave these speeds relative to TLC (geometric mean):

| Profile | 1 worker | 8 workers |
|---|---|---|
| TLC | 0.67× | 0.52× |
| Default | 0.72× | 0.64× |

Some models are forced onto one worker in the TLC profile. TLCSailfish1 takes
53 s at 8 requested workers, against TLC's 12 s.

The x86 campaign of 2026-10-21 (`docs/developer/performance.md`) measured
0.32× TLC at 1 worker and 0.15× at 8.

ADR 0006 requires all of the following:
- at least 2× TLC (geometric mean) at 1 and 8 workers;
- at least 1× on every benchmark;
- at least 3.5× scaling;
- startup of 10 ms or less.

dive meets only the startup target, at 8 ms against TLC's 340 ms. Its memory
use is far lower than TLC's: 4–300 MB against 1.7–3 GB.

### 1.2 Where the time goes

The figures below come from macOS `sample` on a profiling build (`main` at
`92a2e32d`, MCLamportMutex). They are inclusive shares on the worker thread at
1 worker:

| Category | Share |
|---|---|
| Successor generation | 70% |
| Expression evaluator (inside successor generation) | 39% |
| Drop glue and `Arc` release | 18% |
| TLC FP64 fingerprinting | 14% |
| `malloc` and `free` (mimalloc) | 13% |
| Forcing and caching thunks | 6% |

Memory management (allocation, reference counting, drop and free; the
categories overlap) is about 25–30%.

At 8 workers, 38% of worker busy time is spent waiting on one process-wide
mutex. About 98% of those waits come from `Core::eval_std`
(`crates/dive-eval/src/std.rs:55`), which calls `check_native_access`
(`crates/dive-eval/src/host.rs:540`) on every standard-operator call. Skipping
the check in a throwaway build cut 8-worker time from 10.2 s to 7.9 s, with
identical counts; this is issue #495. Even then, user CPU time doubles from 1
to 8 workers for the same work (19 s to 39 s). The likely cause is contention
on shared atomic reference counts.

ADR 0104 (`docs/decisions/0104-evaluator-performance-plan.md`) recorded
inclusive shares on other models:

| Operation | Share |
|---|---|
| Set membership | 20–27% |
| Default equality | 9–15% |
| `EXCEPT` | up to 14% |
| Forcing lazy values | up to 53% |
| INSTANCE substitution (EWD998PCal) | 52.6% |
| Invariant checking | 44–55% |
| Allocation and drop | about 20% |
| Dispatch | 7–16% |

ADR 0104 also records about eleven prototypes. Each changed instruction counts
by 1–2% at most, or regressed; one continuation design cost +73%.

## 2. Why the current design cannot get there

The current evaluator reproduces TLC's structure, not only its semantics:

- TLC's `LazyValue` is `Arc<Thunk>`, which has two `Mutex`es and a cache
  validated against state references (`crates/dive-eval/src/env.rs:525`).
- TLC's `Context` chain is `Arc<Frame>`, with `Arc<SubstLink>` for INSTANCE
  substitutions that are looked up at runtime (`env.rs:381`, `env.rs:393`).
- Values are reference-counted heap objects created on every evaluation
  (`crates/dive-value/src/value.rs:30`).
- Successor generation copies the state on write, through `Arc::make_mut`
  (`crates/dive-eval/src/next.rs`).
- The interpreter walks the IR directly (`crates/dive-eval/src/eval.rs:336`).
  It has some elements of ADR 0014 §1's plan:
  - slots resolved by the IR;
  - typed state-variable reads;
  - caching of unprimed INSTANCE substitutions.

  It does not have the specialized plan, and its values and environments keep
  TLC's object structure. TLC requires re-evaluating primed substitutions in a
  null context, and the new core keeps that (`02-semantic-contract.md`).

TLC can afford this design because the JVM's JIT inlines across virtual calls
and its generational collector makes allocation almost free. A Rust copy pays
an allocation, atomic increments and decrements, a drop walk and a free for
every object. At several workers it also pays for cache-line contention on
shared counts. The flat profile (ADR 0104: no exclusive function above 7%) and
the failed micro-prototypes show that the cost is spread over the whole design.
No local change removes it.

## 3. Where a new core's speed comes from

The new core changes what is computed per generated state and per check. Three
facts make this possible:

- A successor usually differs from its parent in one or two variables.
- Most generated states are duplicates. On MCLamportMutex, 74% of generated
  successors were already seen.
- The number of distinct values a variable takes is far smaller than the
  number of distinct states.

`tlc-baseline.md` records, from the pinned TLC source, what TLC does per state.

| # | Strategy | What it replaces | Cost |
|---|---|---|---|
| S1 | Incremental state hash: a combination of per-variable hashes cached on interned values, updated only for assigned variables | A walk of the whole state per successor | O(changed variables) |
| S2 | Additive value hashes over (key, value) pairs, so `EXCEPT` updates the hash in place | Rehashing the whole value | O(1) per update |
| S3 | Seen-set probe from parent plus delta before the successor is built; only winners are built and interned | Building every successor | Duplicates cost one hash and one probe |
| S4 | Hash-consed, structurally shared compound values; persistent vectors for large functions | Per-state copies and array copies on `EXCEPT` | O(log n) updates; memory proportional to distinct components |
| S5 | Equality by handle, membership by hash index, symbolic membership checks for `a..b`, `[D -> S]`, `SUBSET S` and record sets | Deep comparison and binary search | O(1) on interned values |
| S6 | Memoized pure evaluation of invariant conjuncts, guards and right-hand sides, keyed on interned input handles (`06-memoization.md`) | Full re-evaluation on every state | One evaluation per distinct input tuple |
| S7 | Per-worker transient arenas reset per expansion; states as arrays of 8-byte handles | Allocation, reference counting and free | No allocation in the steady state |
| S8 | Static resolution: INSTANCE substitution inlined by name, constants folded, slots resolved, thunks only where the demand-order analysis requires them | Dynamic substitution and lazy values everywhere | Removes the substitution and forcing shares |
| S9 | Parallelism without contention: lock-free seen-set claims, no process-global locks, no shared reference counts, per-worker arenas | Locks, shared counts and allocator contention | Target ≥6× scaling at 8 workers |

The gains multiply:

- S1–S3 reduce the work for every generated state, most of which are duplicates.
- S5 and S6 reduce the work for every check.
- S7 and S8 reduce the constant factor of the evaluation that remains.
- S9 multiplies all of them by the number of cores.

How much each strategy helps depends on the specification. M0 therefore
measures every strategy with an ablation switch (`09-performance-method.md`)
instead of relying on these estimates.

## 4. What the evidence does not yet show

These are the open questions M0 must answer:

- Whether hash-consing holds up under symmetry and under very large state
  spaces (R5 in `10-plan.md`).
- Whether S6's cache hit rates are high on the benchmark set.
- Whether the laziness-heavy models (GameOfLife, EWD998PCal) gain comparably.
- The current x86 gap at a current commit.
