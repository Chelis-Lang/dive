# Value model

Status: M0 spike on `main` in `crates/dive-core` (`handle`, `strings`,
`store`, `arena`, `node`, `heap`); results in `m0-results.md`. M1 (stream 1):
the value interface is frozen in §11, node data is inline in the store
(step 2), the arena rolls back rejected candidates (step 3), and lazy values
exist on the value side (step 4), integers have any size (step 5), and
strings can compare by TLC token (step 6), and intern hits take no lock
(step 8); the remaining M1 steps are in §12. This document covers strategies S1, S2, S4, S5 and S7 from
`01-motivation.md`.

## 1. Goals

The value model has four goals:

- **No heap allocation in the steady state of successor generation.** There is
  also no reference counting at all.
- **Cheap equality and membership.** Equality of retained values is a handle
  comparison. Membership in an enumerated set is a hash lookup.
- **Incremental hashing.** A successor's state hash, and the hash of a function
  updated by `EXCEPT`, cost time proportional to what changed.
- **Exact semantics.** The model has room for TLC's observable distinctions:
  - symbolic set domains for scalable membership and bounded materialization;
  - intern-token string order under `--compat=tlc`;
  - TLC 32-bit integers under `--int=tlc`, and big integers otherwise.

  See `02-semantic-contract.md`.

## 2. Handles

A value is a `Handle`, a `u64` newtype. Its low three bits are a tag.

| Tag | Meaning | Payload |
|---|---|---|
| 0 | Small integer | 61-bit signed integer |
| 1 | Boolean | 1 bit |
| 2 | String | string id (u32) into the string table |
| 3 | Model value | id (u32) into the string table; the TLC model-value type is the name's first character when its second is `_` (`compare.rs`) |
| 4 | Persistent compound | shard (8 bits), size class (3 bits) and index in the persistent store |
| 5 | Transient compound | epoch (16 bits) and index (32 bits) in the owning worker's arena |
| 6 | Reserved | — (big integers are compound nodes of kind `BigInt`, §11.1) |
| 7 | Reserved | — |

Handles are plain integers, so arenas and stores are vectors indexed by
handles. All of this is safe Rust (`unsafe_code = "forbid"`). Decoding a handle
is a shift and a mask.

The string table is global and append-only. Ids are assigned on first
encounter. Section 7 covers the order rules.

Each heap keeps an append-only copy of name texts. String comparisons and
model-value type classification extend it on an id miss, taking the shared
table's read lock once for the missing suffix. Model-value checks then read
the local copy in both profiles. Classification retains TLC's UTF-16 rule:
the name has more than two code units and its second unit is `_`; its type is
the first unit. Arena reset does not invalidate immutable names or their ids.

## 3. Regions

### 3.1 Persistent store

The persistent store holds every value reachable from a retained state, plus
all constants.

- **Shards.** It is split into 256 shards, selected by the top bits of the
  interning hash: the value hash for canonical nodes, a private structural key
  for retained construction histories and their parents (§8). Each shard has
  two parts:
  - **Node storage.** One append-only column per size class: nodes with at
    most 4 handles, at most 8, and more. A column is a chunked vector of
    write-once `OnceLock` slots (chunk `k` holds `2^(4+k)` slots). The first
    two classes hold the node header and its handles inline in the slot; the
    last boxes its handles. Reads need no lock: a read is one chunk-pointer
    load and one slot load. The class is part of the persistent handle.
  - **Deduplication index.** An open-addressing table of atomic `u64`
    entries (the low 32 bits of the interning hash, and the class and index). An
    intern first probes it without a lock, through the interning heap's
    cached copy of the shard's table (`InternCache`), and returns on a hit.
    A miss takes the per-shard `Mutex` (on its own cache line), probes
    again and inserts. An entry is written once, after its node, with
    release ordering. Growing replaces the table and bumps the shard's
    generation; a heap refreshes its copy when the generation moves, and an
    old table stays valid for whoever still holds it, because nodes never
    move. All of this is safe Rust (#550, step 8).
- **Hash-consing.** A canonical compound is interned once. Interning looks up
  the private index key, compares candidates structurally, and inserts if none
  matches. Equal canonical values share a handle; retained representations can
  differ by their actual construction origin or phase (§8).
- **Intern after claim.** A successor's values are interned only after its
  state fingerprint wins the seen-set claim (`07-engine.md`). Duplicate
  successors, 74% of them on MCLamportMutex, never touch the store. Interning
  is therefore rare relative to evaluation. Most interns are hits (about
  93% on SimpleRegular), and hits take no lock; only inserts take the shard
  mutex.
- **Node contents.** Every node records its kind, its element or entry handles,
  its hash (section 5), and a lazily computed FP64 (section 6).
- **Permanent region.** Constants and config values are interned at startup
  into a region that is never compacted.

`Store::memory_usage` reports a lower bound on requested retained storage from
actual slab allocations, boxed handle slices, current index tables, retired
tables still held by cached readers, retirement-record vector capacity and
unique string payload/vector slots. Slab spare capacity counts; a node-count
times header-size estimate does not. Weak retirement records preserve no table
payload and are collected when reporting after the last reader drops. Multiple
readers of the same table or text do not multiply its payload. Known index
rehash temporary-vector allocation traffic and its largest individual capacity
are separate from retained bytes and do not measure a concurrent peak.

The core's `store.memory` metrics expose these components before explorer worker
teardown. They exclude allocator rounding/metadata, Arc control blocks, opaque
standard-library string-index bucket overhead/spare capacity and external printer/name or host
objects. The string index reports logical capacity and live key/value object
bytes separately; logical capacity is not a physical allocation-byte count.
Worker arenas/caches, graph buffers and concurrent growth scratch need their own
accounting. The snapshot is not a whole-run memory bound or peak measurement;
sampled process RSS remains necessary and cannot enforce allocation-time caps.
This accounting prerequisite changes no capture limits or capacity claim.

### 3.2 Transient arena

Each worker owns an arena: a vector of nodes plus an epoch counter.

- Values computed while expanding one state live there: intermediate sets,
  records, function values, thunks and activation frames.
- The arena is reset, and its epoch incremented, after each state expansion.
  The vector keeps its capacity, so allocation reaches a steady state of zero.
- A transient handle carries the epoch it was created in. In debug builds,
  dereferencing a handle whose epoch differs from the arena's current epoch is
  an internal error. Stale indices are therefore caught by tests, not silently
  misread.
- Transient nodes may reference persistent nodes and other live transient
  nodes. Persistent nodes never reference transient nodes.
- **Rollback.** `mark` and `rollback` free the nodes allocated after a mark,
  within one expansion. Later allocations get a new epoch, and every node
  records its epoch, so a handle to a freed node no longer resolves. The
  filter `{x \in S : P}` and the generator's streamed `x' \in {y \in S : P}`
  free each rejected candidate, and the filter also frees the predicate's
  transients. Both skip the rollback when a thunk was forced to a transient
  meanwhile, since its slot would keep the freed value. Both also enumerate a
  symbolic domain through its cursor rather than materializing it first, so
  CoffeeCan's fold holds the 501,500 survivors rather than the 1,002,001
  candidates.

### 3.3 Interning a transient value (deep close)

Interning a transient value walks it bottom-up: each child is interned before
its parent. The result contains only persistent handles.

Values that can escape into states are deep-closed when they are interned:
- lazy functions;
- recursive-function values;
- `SetPred` and other symbolic sets.

They are materialized to the extensional form that TLC's own fingerprinting
forces when such a value is stored in a state. TLC normalizes and enumerates
these values to fingerprint a state, so materializing them at that point does
not add observable behaviour. The evidence is in `02-semantic-contract.md`.

## 4. Compound representations

**Sets.**
- A canonical sorted array of element handles, in profile order (section 7).
- Above a size threshold (initially 16), a side index from handle to position
  is built on first membership query. It is stored on the node and computed
  once.

**Functions.** A domain handle (an interned set) plus a value array aligned
with the sorted domain.
- **Interval domain.** Application is `values[x - a]` for an integer interval
  domain `a..b`.
- **Other domains.** Application looks up the domain's side index.
- **Large functions and sequences.** Flat arrays at every size. Persistent
  32-way trees (an `EXCEPT` copying one path, with chunks shared across
  states) were planned above 64 entries but are deferred; see §4.1.

**Records.**
- The sorted field names are interned as a shape. A record is a shape id plus
  a value array.
- The compiler resolves field access by a constant name to an index when the
  shape is known statically. Otherwise it reads the index from the shape's
  table.
- TLC distinguishes a record literal (`RecordValue`) from a string-domain
  function (`FcnRcdValue`), and a tuple (`TupleValue`) from a `1..n` function.
  That distinction is a view bit, `flags::RECORD` or `flags::TUPLE` (§8),
  as `dive-value`'s literal flag is (`crates/dive-value/src/value.rs:752`).

**Tuples and sequences** are functions with domain `1..n`; a tuple literal
carries `flags::TUPLE`.

**Big integers** are nodes holding a `BigInt`, used only outside the 61-bit
inline range in the default profile.

### 4.1 Measured: persistent function trees deferred (negative result)

Trees were to land only if they showed a gain on MCbtree and EWD998PCal (M3).
They do not, for three reasons.

- **Neither model runs on the core yet.** MCbtree needs model values ("the
  value override of NIL"), and EWD998PCal needs `Bags!SetToBag` (even with a
  safety-only configuration); both are refused. The reference engine takes
  18.0 s and 8.7 s on them (1 worker, M3, safety only for EWD998PCal).
- **Their functions are small.** MCbtree's largest are `childOf` and `valOf`
  over `Nodes \X Keys`, 8 × 4 = 32 entries. EWD998PCal's are over `0..N-1`
  with `N = 3`. The four spike models peak at 16 entries (GameOfLife's
  grid). A 32-way tree at 32 entries or fewer is a single leaf, so `EXCEPT`
  would copy the same array.
- **A flat `EXCEPT` is cheap at those sizes.** On M3, at load 8–12, the
  costs of `fcn_with`, without and with interning the result, were:

  | Entries | `EXCEPT` (ns) | `EXCEPT` and intern (ns) |
  |---|---|---|
  | 8 | 54 | 110 |
  | 32 | 62 | 194 |
  | 64 | 77 | 259 |
  | 256 | 148 | 628 |
  | 1024 | 583 | 1,917 |

  Those numbers come from a throwaway benchmark over a tuple, with the index
  cycling and the store hit on most interns; the benchmark is not kept.
  Generating one MCLamportMutex state costs about 2,300 ns, so below 64
  entries a tree could save at most a few percent of that.

**Revisit when** a core-supported model in the corpus holds a function or
sequence above 256 entries that its actions update with `EXCEPT` or
`Append`. Measure that model first.

## 5. Hashing

Hashes are 64-bit. Each node caches its hash, so a hash is computed once per
node.

The rules below define the hash so that updates are incremental:

- **Scalars:** `mix64(tag constant XOR payload)`, as in
  `crates/dive-value/src/value.rs:55-66`.
- **Set:** `fin(K_SET, len, Σ h(e))`. The sum uses wrapping addition, and `fin`
  applies a final mix.
  - Adding an element that is not already present adds its hash and
    increments `len`.
  - Removing a present element subtracts its hash and decrements `len`.
- **Function and record:** `fin(K_FCN, len, Σ mix2(h(k), h(v)))`.
  - The node stores the raw sum, so an `EXCEPT` at `k` updates in O(1):
    subtract `mix2(h(k), h(old))`, add `mix2(h(k), h(new))`.
  - The existing function hash is already of this form
    (`crates/dive-value/src/value.rs:787`).
- **State:** `fin(K_STATE, n, Σ_i mix2(i, h(var_i)))`. A successor's hash is
  the parent's raw sum, corrected for each assigned variable whose handle
  differs. That costs O(changed variables) (S1).
  - **VIEW.** Under a VIEW, the state key is instead the hash of the evaluated
    VIEW value.
  - **Symmetry.** Under symmetry, the state key is the hash of the canonical
    representative (`07-engine.md`).

The state hash is the seen-set key in both profiles (ADR 0117). Its collision
probability is reported the way the current fingerprint set reports it. The
exact store (`--store exact`) remains available.

A transient value has a hash too, computed as it is built and with the same
rules. A successor's hash can therefore be computed before any of its values
are interned (S3).

## 6. TLC FP64

TLC's FP64 fingerprint is observable only through three uses (see
`02-semantic-contract.md`):
- per-state random seeding;
- the `TLCFP` operator;
- VIEW random streams.

When a specification can reach one of those, the plan compiler marks the run
as needing FP64. Each node then computes its FP64 lazily and caches it, using
the existing FP64 implementation in `crates/dive-value/src/fp64.rs`, which is
ported as a function over handles. A state's FP64 follows TLC variable order
(`dive_ir::Ir::tlc_vars`). Runs that cannot reach these uses never compute FP64.

### 6.1 Random draws (B6)

`TLC!RandomElement` draws from the run's random streams
(`02-semantic-contract.md` B6, D-002). The core draws exactly what the
reference engine draws, so that a run gives the reference engine's counts
and traces in both profiles: TLC's draws bit for bit under `--compat=tlc`,
and D-002's replayable streams in the default profile. `--seed` replays
the run, and the seed is always reported.

- **Streams.** `dive_core::random` defines the trait `Draws`: a startup
  stream, and a stream for expanding a state. `crates/dive-api/src/core_engine.rs`
  implements it with the reference engine's parts:
  - `dive_stdlib::random::Streams` for the run's seed and mode;
  - `dive_store::state::StateFingerprinter::tlc` for the state key. This
    is TLC's FP64 state fingerprint, over the state converted to reference
    values (`print::to_value`).

  This adds no layering edge: `dive-core` defines the trait, and `dive-api`
  already depends on `dive-stdlib` and `dive-store`. Converting and
  fingerprinting each expanded state costs time, but only in plans that
  draw (`Plan::effects`). Computing FP64 over handles (§6) would remove
  that cost later. `crates/dive-api/src/core_random.rs` holds the
  implementation.
- **Where draws come from.**
  - Constants and `ASSUME`s draw from the startup stream, on the reference
    binder's host. If they drew, a draw in `Init` is refused: it would
    continue that stream, and TLC's trace rebuild
    starts `Init` from a fresh one (D-002, fixture `RndTraceAssume`).
  - `Init` draws from the startup stream.
  - Expanding a state starts that state's stream (TLC reseeds with
    `fingerprint ^ seed`). The next-state relation draws from it, action
    by action, in TLC's order.
- **Checks.** TLC's checks on a successor draw from the same stream as the
  expansion. A draw in a check (an invariant, a constraint, an action
  constraint or an implied action) is refused by name. The trace rebuild
  replays the actions without the checks, so it would see the stream at
  another position.
- **Rebuilding traces.** The rebuild uses the same streams: the startup
  stream for `Init`, and each predecessor's own stream after that. TLC's
  call-stack replay after a failing check (`ending.rs`) is refused in a
  plan that draws. A check that fails on a successor of an initial state
  is refused with the reference engine's feature name and the stop's
  statistics: TLC rebuilds that trace from the initial state's object,
  whose stream is consumed, and may drop its last state
  (`docs/tlc-differences.md`, D-002).
- **Evaluation counts.** A draw is an effect (`Info::effect`). It is never
  folded, memoized or cached. Inside a function constructor, a filter, a
  set map or an operator argument's body, TLC evaluates lazily, so a draw
  there is refused. A `LET`-bound definition or an operator argument with
  an effect is TLC's `LazyValue`: forced once within one expression, but
  evaluated again in each conjunct of `Init` or an action that uses it
  (fixture `RndLetRedraw`). The core forces its thunk once, so a use in a
  second generator expression is refused (`Compiler::effect_thunk_use`).
  This applies to `Print` and `PrintT` too.
- **VIEW and SYMMETRY.** A specification with randomness and a `VIEW` or
  `SYMMETRY` is refused before binding, as the reference engine refuses it
  (`dive_model::transformed_randomness`, fixture `RdView`).
- **Workers.** A plan with an effect runs one worker, and the report says
  why. Draws themselves are per state, and could run in parallel later.
  `--core-sample` is refused in a plan that draws.
- **Argument shape.** TLC draws one value per component from a lazy
  `[S -> T]`, `[a : S]` or `S \X T`, but one index from an enumerated set.
  So:
  - the argument keeps the core's symbolic sets symbolic;
  - an enumerated set of functions is refused when the plan may build
    such lazy sets (`Store::lazy_sets`), because TLC may hold it lazily.

  Intervals and `SUBSET` draw an index into the normalized order either
  way, so they need no such rule.
- **Not done.** `RandomSubset`, `RandomSetOfSubsets` and
  `TLCGet("seed")`.
- **Tests.** `crates/dive-api/tests/core_random.rs` runs the `Rd*`
  fixtures (`tests/fixtures/core-lib`) and the reference engine's `Rnd*`
  fixtures at four seeds in both profiles against the reference engine,
  checks that `--seed` replays at any worker count, and checks the `Rd*`
  fixtures' counts and printed lines against the pinned TLC at `-seed 0`
  under `--compat=tlc`. No new divergence: the default profile's draws are
  D-002's.

### 6.2 Registers and the level (B6)

`TLC!TLCSet` and `TLC!TLCGet` read and write the run's registers, and
`TLCGet("level")` reads the level of the state TLC evaluates against.

- **Registers.** `dive_core::random::Registers` is the interface;
  `crates/dive-api/src/core_random.rs` implements it with the reference
  evaluator's own registers (`dive_eval::Evaluator::tlc_register`). They
  hold what the constant definitions and `ASSUME`s wrote, and the
  reference engine's code computes every result and error, so the core
  gives TLC's values under `--compat=tlc` too. Arguments and results are
  converted as reports are (`Conversion::Report`). Supported keys:
  integer registers, `"s:..."` names, `"all"` and `"all:named"`, and
  `"config"`, which the core's host provides after binding as the
  reference engine does (`dive_check::engine::run_config`). The `ASSUME`s
  read the start-up counters; from `Init` on the counters are not
  provided.
- **Level.** `Machine::level` is set by the engine: 0 in `Init`, the
  checked state's level in invariants, constraints and implied initial
  predicates, the expanded state's level in the next-state relation,
  action constraints and implied actions. The trace rebuild sets each
  predecessor's level.
- **Order.** Both are effects (`Info::effect`), so a plan that uses them
  runs one worker, in TLC's order, and they are never folded or cached.
- **The argument of `TLCGet`** must be a literal: TLC reports an error
  raised while evaluating the argument inside `TLCGet`'s own wrapper.
- **Refused by name.**
  - a run with registers that does not end in success, because TLC
    writes and reads them again while it rebuilds a trace or replays an
    error;
  - TLC's call-stack replay after a failing check, in a plan with
    registers or the level;
  - `TLCSet("exit")`, `TLCSet("pause")`, `TLCSet("-D...")`, and `TLCGet`
    of the counters during the search, `"stats"` and other keys;
  - `--core-sample` with registers.
- **Tests.** `crates/dive-api/tests/core_registers.rs` runs the `Rg*`
  fixtures in both profiles at 1 and 4 workers against the reference
  engine and the pinned TLC (counts and printed lines), and the `Tm*`
  fixtures of `dive-check` against the reference engine or a named
  refusal.

## 7. Order

Sets are stored in a canonical sorted order, and the order depends on the
profile.

- **Default profile.** Canonical value order (D-010; ADR 0012). Strings
  compare by text.
- **`--compat=tlc`.** TLC order:
  - strings compare by intern token, which reproduces `UniqueString.compareTo`;
  - other kinds follow `compareTo`.

  Tokens come from the string table, which is fed in SANY and TLC encounter
  order at startup, as `crates/dive-model/src/intern.rs` does today. Strings
  created at runtime get tokens in encounter order. With several workers that
  order is nondeterministic, so `07-engine.md` §6 covers how runtime names are
  ordered.
- **Normalization errors.** These use TLC tokens in both profiles (ADR 0012;
  `02-semantic-contract.md`).

Comparing two values in order is a structural comparison. It is needed only
when sets are built or merged, never for equality.

## 8. Canonical construction and symbolic views

Finite literals and mapped sets use canonical construction in both profiles
(ADR 0153, D-028). Elements are sorted and deduplicated by mathematical value.
There is no per-object raw/normalized phase, constructor origin, receiver
ownership proof or sparse history row in ordinary execution. Default mixed
structured sets have order-independent identity; nominal compat keeps inexpensive
checked-comparison refusals rather than reproducing TLC's mutable set history.

Union merges two already canonical explicit operands in order instead of
concatenating and sorting their elements again. Symbolic operands keep their
existing path. The complete result still passes mathematical identity and
profile-specific class checks, and the merge polls the existing work owner.

Identity consumers require a recursively materialized mathematical value. This
includes singleton set elements, function keys and values, equality, membership
in an explicit set, and stored state. Symbolic children cannot supply inequality
from their structural representation. Unproved identities return `unsupported`,
including during independent counterexample validation. These checks retain
cooperative interruption polling and bounded materialization; removal of history
machinery does not admit unknown `Seq`, product or deferred-provider identities.

Symbolic sets remain useful operator domains. `Seq` keeps its range symbolic and
asks membership only for actual sequence elements, so the empty sequence does
not enumerate a huge or infinite range. `SUBSET` membership enumerates the
concrete query and leaves an unused base unenumerated. Cartesian-product
cardinality and finiteness use bounded factor algorithms rather than expanding
the whole product. They check operand shapes, depth, work budget and size bounds;
unknown demanded shapes refuse. Sequence finiteness checks range finiteness and
emptiness, including empty products and nested sequence ranges. Holding and
opposite-invariant controls prevent false violations and false success.

The existing default compound-membership index is derived lookup data for
canonical explicit sets. It starts on the second eligible lookup for sets with
at least 16 members, uses closed value hashes and verifies candidates with the
canonical comparator. Warm lookups check the query and reuse the immutable
elements admitted when the index was built, avoiding another full set traversal.
The root must be non-lazy; index fallback retains the complete materialized-tree
check, including its depth bound. Misses retain class/error checks. Scalar queries, finite
fuel, lazy or unproved values and TLC comparisons retain ordinary search. The
`member-index` ablation changes the lookup strategy without changing results.

Each worker's index is reserved inside its existing memo cap, at most 4 MiB or
one-sixteenth of that cap. Allocation is lazy and fallible. Complete set handles
identify records; weak Store and Arena identities prevent reuse after
replacement. Local records bind to reset/rollback revisions, while persistent
records survive arena rollback. Exhaustion or allocation failure returns to
ordinary search. Construction and probing poll the same work owner.

Identity is extensional; genuine representation metadata stays separate:

| Flag | Meaning |
| --- | --- |
| `UNNORM` | legacy `SetEnum` symbolic storage; finite normalization yields a canonical set |
| `RECORD` | a function built by a record literal |
| `TUPLE` | an interval function built by a tuple literal |
| `NESTED_VIEW` | a structural parent containing child representation metadata |

Hashing, equality and order ignore these bits. `intern` drops them for canonical
identities; `intern_const` and `intern_exact` preserve available literal metadata
and symbolic forms for evaluation, printing and captured originals. `EXCEPT`
keeps genuine record and tuple bits. Metadata never proves mathematical identity.
The `SetEnum` kind remains a finite symbolic representation for existing callers;
Core source literals use canonical sets, and direct unnormalized membership has
a named refusal rather than an emulated TLC search.

Candidate DAG records retain their existing fields, with the legacy origin word
fixed at zero. Decoding rejects nonzero origins or history flags. The retained
report fields for the constructor-origin index stay zero; no trace version or
report schema changes. Store snapshot and exchange compatibility code is separate
from ordinary execution and does not grant an identity proof.

## 9. Integers

- **Default profile** (`IntMode::Big`, D-004). Integers are inline while they
  fit 61 bits; arithmetic checks for overflow and promotes to a `BigInt` node
  (`int.rs`).
- **`--int=tlc`** (`IntMode::Tlc`). TLC's 32-bit semantics: operands and
  results are Java `int`s, and arithmetic outside the TLC range raises TLC's
  overflow error with TLC's message (`Overflow when computing 2147483647+1`).
  `Naturals.Divide` (`IntOp::DivNaturals`) is available for a specification
  that loads `Naturals` but not `Integers`.

The semantics, messages included, are those of the reference engine's
`dive_value::Int` (`docs/provenance.md`). The evaluator keeps an inline fast
path for `+`, `-`, `*`, `\div` and `%` whose result is certain to be inline
(and, in TLC mode, a Java `int`), and calls `Heap::int_arith` for everything
else. The mode is set per heap with `Heap::set_int_mode`; the default is
`Big`.

## 10. Memory bound

The persistent store grows with the number of distinct values that are ever
retained. With a fingerprint seen set, values referenced only by states that
are already expanded become unreachable.

Most specifications have far fewer distinct component values than distinct
states, so this is acceptable as the first implementation. It is measured as
intern hit rate and store size in M0.

Opt-in completed-level reclamation is available for owned pure Core safety BFS
and recovery-only runs (`--store-compact-bytes`, ADR 0149). Its measured-Store-byte
setting triggers work, rather than imposing a hard cap. At a joined level boundary it:
1. copies the values reachable from the frontier and the permanent region into
   fresh shards;
2. remaps the frontier handles;
3. clears the memoization caches.

The compiled startup keeps its issued coordinates; live original/canonical
frontier rows and registered raw/normalized origin companions are remapped into
a fresh owner. Machines and spilled frontier codecs are replaced, releasing old
Arena/cache/Store owners. This route detaches resident exact keys into full
mathematical bytes. Names, mandatory live roots, fixed startup, seen keys,
mapping/replay workspaces and old/new overlap remain separate retention.
Checkpoint pause, temporal capture, effects and sampling refuse this option.
Accepted discovery residency and temporal paging remain unimplemented.

The worker's bounded explicit-set comparison certificates also contain
persistent handles and are cleared with those caches. They retain an exact
aggregate-shape witness, without changing stored nodes or interning values
for lookup (`06-memoization.md` §2.2a). Arena reset and rollback leave these
persistent handles valid; compaction and machine/store replacement do not.

### 10.1 Measured: the MCLamportMutex bisection

A reported rise in RSS from 85 MB (main at #523) to 98 MB (main at #514) was
noise, not a regression. The test used release builds of each merge in the
range, run at 1 worker, three times interleaved, at load 7–15 on M3:

| Main at | Max RSS, 3 runs (MB) | Peak footprint, median (MB) |
|---|---|---|
| #523 `b836629e` | 85.5, 90.5, 92.0 | 85.6 |
| #510 `57e9e679` | 95.6, 96.4, 96.9 | 81.7 |
| #520 `6ed14712` | 79.0, 94.7, 97.4 | 81.6 |
| #514 `a87f1bea` | 87.5, 87.9, 96.0 | 81.7 |
| #524 `431d6f95` | 94.9, 95.2, 97.6 | 81.8 |

The store's counts are identical at every merge (182,053 nodes, 546,115
handles). The footprint fell by 3.9 MB at #510 (the inline columns) and is
flat after it, while one binary's max RSS spans 79–98 MB. Memory is
therefore compared by footprint (`09-performance-method.md` §4).

## 11. Interface (M1)

This section is the value interface that the evaluator (`eval`), the plan
compiler (`compile`) and successor generation (`generate`, `dive-explore`)
code against. Streams 2 and 3 use only what is listed here. A change to it is
made by stream 1 and recorded here first. Items marked *(step n)* are
specified but not yet on `main`: they land with step n of §12, and their
signatures are frozen.

### 11.1 Handle kinds and encoding

A `Handle` is a `Copy` `u64`. The low three bits are the tag; the encoding is
private to `handle.rs`, and other modules use only the methods named here.

| Tag | Kind | Payload | Constructors and accessors |
|---|---|---|---|
| 0 | Integer, 61-bit two's complement | value | `Handle::int(i64) -> Option`, `as_int` |
| 1 | Boolean | 1 bit | `Handle::TRUE`, `FALSE`, `bool`, `as_bool` |
| 2 | String | id (u32) in the string table | `Heap::string(&str)`, `as_str_id` |
| 3 | Model value | name's id (u32) in the string table; the TLC type is read from the name | `Handle::model(id)`, `as_model_id` |
| 4 | Persistent compound | shard (8 bits), size class (3 bits), index | issued by the store only; `is_persistent` |
| 5 | Transient compound | epoch (29 bits), index (32 bits) | issued by the arena only; `is_transient` |
| 6 | *(reserved)* | — | — |
| 7 | Empty marker | none | `Handle::EMPTY`, `is_empty` |

Rules:

- **Equality.** Equal handles are equal values. Two *persistent* handles of
  closed values (§11.3) are equal values exactly when the handles are equal,
  in the default (hash-consing) store. For anything else use `Heap::eq`.
- **Integers outside 61 bits** are compound nodes of kind `BigInt`, never
  inline. A canonical integer is inline whenever it fits, so an inline and a
  big handle are never equal. Where a big integer is possible, use
  `Heap::is_int`, `Heap::big` and `Heap::int_cmp` rather than `as_int`.
- **`Handle::bits`** is for hash keys only; it carries no order.
- **Lazy constants.** A lazy value interned by `intern_const` (for example
  `[x \in Nat |-> x + 1]`) is stored structurally, in a class of its own, so
  that `Handle::is_lazy_persistent` tells it from the handle. Two such
  handles are never compared; `Heap::eq` records an internal error.
- **Size classes** are a store detail (step 2). Code outside the store
  never decodes a persistent handle.

### 11.2 Node kinds and views

`Heap::view(h) -> View` reads any compound from either region. A `View`
borrows the heap; copy what you need out of it before calling any `&mut Heap`
method.

| `Kind` | `lo` | `data` |
|---|---|---|
| `Set` | 0 | sorted distinct elements (run order, §7) |
| `IntFcn` | domain lower bound (1 for tuples, sequences and the empty function) | values |
| `Fcn` | 0 | `n` sorted keys, then `n` values |
| `Interval` | lower bound | `[hi]` |
| `Subset`, `SeqSet` | 0 | `[S]` |
| `FcnSet` | 0 | `[D, R]` |
| `RecSet` | 0 | `n` sorted field names, then `n` sets |
| `Product` | 0 | the factors |
| `Nat`, `Int` | 0 | empty |
| `Lambda` | closure id | `[D, captures...]`: `[x \in D \|-> e]` before it is materialized |
| `SetPred` | closure id | `[S, captures...]`: `{x \in S : P}` before it is enumerated |
| `Union` | 0 | `[A, B]`: a lazy `A \cup B` |
| `BigInt` | sign (`1` or `-1`) | the magnitude's 32-bit digits, least significant first, as inline integers; read only through `Heap::big` |
| `SetEnum` | 0 | legacy symbolic enumeration; normalization produces canonical identity (§8) |

`View` has `kind`, `lo`, `hash`, `sum`, `flags` and `data`. Use
`len`, `keys` and `values` rather than indexing `data` by kind. Every kind
except `Set`, `IntFcn`, `Fcn`, `BigInt`, `Lambda` and `SetPred` is
*symbolic* (`Kind::is_symbolic`): a set the heap can enumerate on its own.
`Lambda` and `SetPred` are *lazy* (`Kind::is_lazy`): only the evaluator can
force them.

**Canonical form.** Builders always produce the canonical form: a function
whose keys are consecutive integers is an `IntFcn`; anything else is a `Fcn`
with sorted keys. So structural equality of normalized, closed values is
equality of node contents, and the evaluator never needs to compare
representations.

### 11.3 Normalized, closed and lazy values

- **Normalized**: not a finite symbolic set. `Heap::norm` normalizes a handle
  (it materializes a finite symbolic set and leaves everything else alone).
- **Lazy**: the value contains a `Lambda` or `SetPred` node, which
  only the evaluator can force (the heap has no access to the plan).
  `Heap::is_lazy(h)` tells. A lazy node is never a set element or a
  function key: the evaluator closes a value before it uses it as either,
  because TLC normalizes sets (and so forces their elements) on every
  comparison.
- **Closed**: normalized at the top level and not lazy anywhere. `hash`,
  `eq`, `cmp`, the set builders and `intern` require closed inputs (sets:
  closed elements; functions: closed keys). Given a lazy input they record an
  internal error (`Heap::poisoned`), never a wrong answer.

### 11.4 Heap operations

`Heap` is one worker's handle on the shared `Store` plus its own `Arena`. Each
worker (and the compiler, while folding) owns exactly one `Heap`; it is not
`Sync`. The operations each stream may use:

| Operation | Purpose | eval | compile | generate / explore |
|---|---|---|---|---|
| `view`, `kind` | read a compound | yes | yes | yes |
| `hash` | value hash of a closed value; the state key is built from it | yes | yes | yes |
| `eq`, `cmp` | equality and run order of closed values | yes | yes | yes |
| `is_finite`, `cardinality` | set size questions | yes | yes | yes |
| `cursor` + `Cursor::next`, `elements` | enumerate a finite set in run order | yes | yes | yes |
| `norm`, `materialize` | normalize, or force a set's enumeration | yes | yes | yes |
| `materialize_cached` | membership in a persistent symbolic set, once per worker | yes | no | no |
| `set_from`, `set_sorted`, `empty_set` | build a set (unsorted with duplicates, or sorted distinct) | yes | yes | yes |
| `int_fcn`, `tuple`, `fcn_sorted`, `fcn_from_pairs` | build a function, record or tuple | yes | yes | yes |
| `fcn_with` | `EXCEPT` at an entry index, O(1) hash update | yes | no | yes |
| `symbolic`, `interval` | build a symbolic set | yes | yes | no |
| `string` | intern a string's text | yes | yes | no |
| `lambda`, `set_pred`, `union` | build a lazy value | yes | yes | no |
| `closure(h) -> Option<Closure>` | the closure id, domain and captures of a `Lambda` or `SetPred` | yes | no | no |
| `rebuild(h, data)` | the same kind with new contents, canonicalized and rehashed (used to close a value bottom-up) | yes | no | yes |
| `is_lazy` | whether the value must be closed first | yes | yes | yes |
| `is_int`, `big`, `int_from_i64`, `int_from_big`, `int_cmp`, `int_arith`, `int_neg`, `set_int_mode` | integers of any size, and arithmetic in the run's `IntMode` | yes | yes | mode: engine setup |
| `mark`, `rollback` *(step 3)* | discard transients built since a mark | yes | yes | no |
| `intern` | deep-close a closed value into the store | no | no | engine, after the claim |
| `intern_const` | intern a constant, keeping a top-level symbolic set symbolic and a lazy value lazy | no | yes | no |
| `intern_view` | intern a state value and, for a top-level symbolic set, its view (§8) | no | no | engine, after the claim |
| `reset` | end of an expansion: free every transient | no | after each fold | engine, per expansion |
| `poisoned` | an invalid handle read was detected, or a lazy value was hashed (a bug) | check | check | check after each successor |

Persistent handles are coordinates within their issuing Store, not globally
branded identities. Keep every handle with that Store. An unissued coordinate
can be detected, but a foreign word whose bits also name a locally issued
coordinate cannot be distinguished by `poisoned` or an origin-table lookup.
Machine/store replacement invalidates those handles (§10); an origin check
does not make arbitrary cross-Store handle injection a supported operation.

Store operations outside the heap are limited to `Store::new(hashcons)`,
`Store::with_string_order(hashcons, order)`, `Store::stats` and
`Store::strings` (with `Strings::preload` at startup, before compilation). Nothing outside `heap.rs` reads store or
arena nodes directly.

### 11.5 Ownership and lifetime rules

1. **Arena epoch.** Every transient handle belongs to one worker's arena and
   one epoch. `reset` frees all of them and starts a new epoch. A transient
   handle must not be kept across a `reset`, stored in another worker's
   structures, or put in the persistent store, memo tables or the frontier.
   A stale read is detected in every build and poisons the heap; the engine
   reports it as an internal error.
2. **Rollback** *(step 3)*. `mark()` returns a `Mark`; `rollback(mark)` frees
   every transient allocated after it. A handle allocated after the mark must
   not be used after the rollback; this includes values held in frame slots
   or thunk caches. Each rollback starts a new epoch for later allocations,
   so a stale use is detected like rule 1. Use it only around work whose
   result is a scalar or a value built before the mark (a filter predicate,
   a rejected candidate).
3. **Intern after claim.** The engine interns a successor's values only after
   its state key wins the seen-set claim (or, without the early probe,
   before the probe). Nothing else interns state values, except that a step
   predicate's memo sites key on exact copies of a successor's values
   (`Heap::intern_exact`, `06-memoization.md` §2.3), leaving the successor
   as it is. The compiler interns constants with `intern_const`.
4. **Deep close on intern.** `intern` walks a value children first, interning
   each child, and materializes finite symbolic sets (TLC fingerprints a state
   by normalizing it, so this adds no observable behaviour). A lazy value
   is closed by the evaluator first: for each `Lambda` or `SetPred`
   it evaluates the materialization, then rebuilds the parents with
   `rebuild`. `intern` given a lazy value is an internal error.
5. **Persistent values are immutable and never reference transients.** They
   live until their Store is dropped. The owned safety compactor (§10) remaps
   all live roots before replacing that owner.
6. **Views and identity** (§8). A value's identity handle is its normalized,
   closed form. A lazy view (a symbolic class, or TLC's `isNorm` state) is
   kept beside it, never in it: `hash`, `eq`, `cmp` and `intern` ignore
   `View::flags` (except to refuse a lazy value), and the store hash-conses
   a view-flagged node apart from identities (§8, view bits). The engine keeps a state slot's view handle in a
   side table when the plan asks for it.
7. **Sharing.** `Store` is `Sync` and shared through `Arc`. Inserting takes a
   per-shard lock; canonical node reads and intern hits take none. Construction
   snapshot checks use the shared origin-table access in §8. A persistent handle
   may be passed to any worker once it has been returned by `intern`.

## 12. Next step

M0 built the following. Measurements are in `m0-results.md`.
- 64-bit tagged handles: inline 61-bit integers, booleans and string ids; a
  persistent tag (shard and index); a transient tag (epoch and index).
- The string table, and the persistent store: 256 shards, a `Mutex` index per
  shard, and append-only chunked `OnceLock` node storage that is read without
  locks.
- The arena with an epoch check on every dereference. A stale handle poisons
  the heap and becomes an internal error.
- Sets, functions, records and tuples as sorted plain arrays. Integer-domain
  functions are stored densely.
- Symbolic `a..b`, `Nat`, `Int`, `SUBSET`, `[D -> R]`, record sets, `Seq(S)`
  and products. Persistent finite symbolic sets are enumerated at most once
  per worker.
- Section 5 hashing: an incremental `EXCEPT` and state hash from the cached
  child hashes, with a full-recompute switch.
- The store microbenchmark `crates/dive-core/examples/store_scaling.rs`.

Deviations, to resolve in M1:
- The literal flag for tuples and records is not part of identity (since M1
  step 7 it is a view bit, §8).
- General lifetime compaction remains open. Owned pure safety and recovery-only
  runs have explicit completed-level reclamation (§10); memo values and
  comparison certificates are cleared when that Store remaps.

M1, stream 1. The steps below are the ones §11 refers to.

1. **Freeze the value interface.** Done: §11.
2. **Node data inline in the store.** Done: §3.1. Size classes 4, 8 and
   boxed (classes 16 and 32 inline were tried: GameOfLife's 32-handle grid
   nodes then cost 15 MB more RSS, through chunk doubling, for no time gain).
   Measured on M3 (indicative, load 6–13): random persistent reads 25–35%
   faster (`examples/store_views.rs`: 47–58 ns to 33–38 ns for 4 and 8
   handles); single-thread `store_scaling` 28% faster; the four spike models
   within noise on cycles, with 3% more instructions on MCLamportMutex (the
   class dispatch); CoffeeCan peak RSS 224 to 214 MB. Counts unchanged.
3. **Arena rollback.** Done: §3.2. CoffeeCan peak RSS on M3 (indicative)
   214 to 138 MB (the reference engine: 134 MB), with 2.5% fewer
   instructions; with `-fold`, 298 to 171 MB. The other models are unchanged.
   Open: an accepted candidate's continuation in the generator is not yet
   rolled back, and GameOfLife's arena still grows across its 65,536 initial
   states (stream 3, `07-engine.md`).
4. **Lazy values, value side.** Done: `Lambda`, `SetPred` and `Union`
   nodes, `View::flags` with `flags::LAZY` propagated by every builder, the
   checks of §11.3 (a lazy value hashed, compared, put in a set or interned
   is an internal error, never a wrong answer), `closure`, `rebuild`,
   lazy constants, and `intern_view` for state-slot views. `Union`
   membership is TLC's `SetCupValue.member` (either side, left first).
   Cost on M3 (indicative): MCLamportMutex +2% instructions, the other
   models within ±0.6%; counts unchanged.
   Left to stream 2, the evaluator side: compiling `[x \in S |-> e]`,
   recursive functions and `{x \in S : P}` to these nodes, application of a
   `Lambda`, `SetPred` membership and enumeration, and the `close` walk.
   The `isNorm` and literal bits followed in step 7.
5. **Big integers.** Done: §9. Numerals, startup constants, arithmetic,
   comparison, `Nat`, `Int` and interval membership handle integers of any
   size. A new `dive-api` fixture (`core-m0/Big`) agrees with the reference
   engine: 21 distinct, 24 generated, depth 7. Cost on M3 (indicative):
   within ±0.4% instructions on the four spike models. Open:
   - the engine does not yet pass `--int=tlc` to `Heap::set_int_mode`
     (stream 3), since the core refuses `--compat=tlc` in M1;
   - the compiler does not yet choose `DivNaturals` for `Naturals`-only
     specifications, nor raise TLC's "integer too big" for numerals in TLC
     mode (stream 2);
   - interval bounds, `Len` and similar remain 61-bit (refused beyond).
6. **Compat-order strings, value side.** Done: `StringOrder::{Text, Token}`
   fixed per store (`Store::with_string_order`), and `Strings::preload` to
   feed TLC's startup encounter order so that ids are TLC's tokens (§7).
   Open: the binder handoff must pass the reference binder's intern order
   (`crates/dive-model/src/intern.rs`) to `preload` (stream 2), and the
   engine must pick `Token` under `--compat=tlc` (stream 3). Runtime names
   with several workers are `07-engine.md` §6.

7. **View bits.** Done: §8. `flags::{UNNORM, RECORD, TUPLE}`, with
   `Kind::SetEnum`, `Heap::{set_enum, record, tuple_literal}` and
   `View::{is_norm, is_record_literal, is_tuple_literal}`. Record and tuple
   literals carry their bit. The store keeps view-flagged nodes apart from
   identities. Cost on M3 (indicative): within +0.3% instructions on the
   four spike models; counts unchanged. Open:
   - the evaluator's TLC-order membership in a `SetEnum` and the C2/C5
     comparisons that read the bits (stream 2, with the compat order, M2);
   - the engine's view side table for state slots (stream 3).

8. **Lock-free intern hits.** Done: §3.1 (#550). Only inserts take the
   shard mutex. Measured on M3 (indicative, load 5.5–11, 4 performance and
   4 efficiency cores), SimpleRegular with `N = 10` (6,375,622 distinct,
   41,739,274 generated at 1, 4 and 8 workers, unchanged): at 8 workers,
   time sampled in the shard mutex wait falls from 1,312–1,507 samples to
   17–327 (of about 40,000), and wall time from 20.2–20.5 s to 19.4–19.5 s;
   at 1 worker the time is unchanged (42.1–42.5 s against 42.4–42.8 s).
   The default `N = 8` model and `store_scaling` are within noise on M3,
   where the lock was rarely contended (0.17% of interns at 8 workers).
   Instructions on the four spike models are within ±0.2%. The hit counter
   is per heap and is added to the store's statistics when the heap is
   reset or dropped. Open: an x86 measurement at 8 and 16 workers, where
   #550 was found. A worker-side `memset` in `dive-explore`'s
   `expand_level` costs about 1.7% of samples at 8 workers (stream 3).

Next (stream 1, after the open PRs merge):
- `TLCSet` and `TLCGet` are done (§6.2). Next: `TLCSet("exit")` (the
  engine stops after the current state), `TLCGet("config")` and the
  counters, and runs with registers that end in a violation (TLC's trace
  rebuild writes them again). #557 (the reference engine passes a
  literal-bodied operator argument as its value) is in its own PR.
- Persistent 32-way trees are deferred by a negative result (§4.1).
- An x86 measurement of steps 2 and 3 (`09-performance-method.md`):
  `store_views`, `store_scaling`, and the four spike models' time and RSS.

### `kSubset` boundary assessment (2026-10-28)

`FiniteSetsExt!kSubset` remains refused by both engines. Eighteen small probes
run through `dive-conformance dev capture` against `tlc-2026.10.06.014338`
(seed/fingerprint 0, one worker) confirm that this requires a symbolic value
slice, not an eager set-valued library function. The pinned implementation is
CommunityModules `FiniteSetsExt.kSubset` at `78b98ae9`, returning TLC's
`KSubsetValue` at `94d0c507` (a subclass of `SubsetValue`).

- Negative k and k above a finite base's size produce the empty set; k=0
  produces the singleton empty set. Enumerating `kSubset(2, {1,2,3})` gives
  three distinct states, six generated states and depth one.
- `Cardinality(kSubset(0, Nat)) = 1` and
  `Cardinality(kSubset(-1, Nat)) = 0` succeed without enumerating the base.
- `<<1,2>> \notin kSubset(1, 1..3)` succeeds, whereas membership of `<<1>>`
  errors. Scalar membership errors too, except when the result is empty.
  Replacing the value with an ordinary enumerated set would change these
  observable membership/error rules under `--compat=tlc`.
- Cardinality with k=0 over `0..2147483647` succeeds; membership of `{}` in
  that same value errors while sizing the base. Cardinality at k=2 over
  `1..65537` raises the Java-override error 2154. Non-integer k and non-set
  bases raise argument error 2169, with the first argument's type checked
  before the second's after their values are evaluated; a negative k does
  not suppress validation of a non-set base. The probes put these
  errors in `ASSUME`, where TLC's deciding wrapper is 2105, exit 10.

The current core has no k-subset representation or cardinality-bounded subset
cursor; its ordinary `SUBSET` cursor enumerates every size and refuses bases
over 24 elements. A future slice needs the k and base retained through the
existing symbolic-value paths (cardinality, membership, comparison, enumeration,
printing, interning and conversion), with bounded work and the observed error
order. Reusing `SUBSET` followed by filtering would lose the operator's
capacity and demand behavior.

There is a separate startup/validation dependency: the reference binder still
evaluates constants and `ASSUME`s before core compilation, and the independent
validator uses that evaluator. Neither implements `kSubset`. Extending only
the core's stdlib dispatch would therefore leave those paths refused. Resolve
that boundary under the frozen-reference policy before attempting full operator
coverage. No runtime change, corpus agreement or performance credit follows
from this assessment; raw probe output stays outside the repository.

### Sequence replacement fallback semantics

Pinned `tlc-2026.10.06.014338` and CommunityModules `SequencesExt.tla`
(SHA-256 `8b96d0ff74836cca2adfb7da3b32846ca64c4d466d543b55c8d05c32b64aa0f6`)
confirm that the evaluating override handles strings and otherwise evaluates
the TLA+ definition. `ReplaceFirstSubSeq` chooses the first occurrence.
`ReplaceAllSubSeqs` maximizes matching partition components, then minimizes their
component indices; this is not the same as earliest absolute text positions
(see the rejected-rule finding below). On a nonempty sequence an empty pattern
inserts before each element, with no trailing insertion. If both pattern and
text are empty, ReplaceAll returns the replacement. The pure oracle fixture
`core-replace/SeqReplacePure` checks fourteen such results and completes with
1 distinct / 2 generated / depth 1, success/exit 0.

Effects must not be treated as a single value snapshot. On
`core-replace/SeqReplaceInlineEffect`, the pinned oracle prints argument tags
`r,s,t,s,t,r` at each of two states, versus only `r,s,t` for the otherwise
equivalent string fixture. Both complete with 2 distinct / 2 generated / depth
2, success/exit 0. These exact order/count observations were captured before
implementing the new supported slice; raw outputs remain external. Pure
sequence arguments can use the bounded direct algorithm within the boundary
below. Non-string
arguments with effects remain explicitly refused; existing effectful string
calls retain the override's single evaluation.

The Auto effect controls establish preservation of the frozen reference output,
not exact TLC argument-demand order. Existing defect #612 reproduces on the
rejected historical source too: for the inline sequence fixture the reference
and Auto print `r,s,t,r,s,t` at each state, versus TLC's `r,s,t,s,t,r`. Counts
match (12 across two states), but order does not. This remains a separate
frozen-reference bugfix obligation; neither refused effectful sequence calls
nor their Auto fallback earn pinned-TLC effect-order parity credit here.

The supported slice is pure materialized sequences whose combined elements
are all integers, all booleans or all strings (including empty sequences).
ReplaceAll additionally refuses overlapping nonempty occurrences after its two
early equality CASE arms. That refusal includes cases where the rejected greedy
rule happens to agree with TLC; no overlap implementation credit is claimed.
Uniform scalar classes avoid skipping the comparison errors that normalization
of the fallback's `SubSeqs(t)` can expose. Mixed, compound, model-value and lazy
sequence arguments remain refused even when an early CASE arm might succeed.
The direct matcher uses a KMP prefix table and consumes each selected match,
so its work is linear in input plus output size. Inputs and materialized output
are bounded at 2^24 elements; scans and copies charge constant-fold fuel.
The repeatability proof also refuses compiled calls and free slots, whose
callee/capture effects are not represented by the expression's effect bit.
The existing string path is unchanged.

`core_sequence_replace` checks the pinned pure results and 3-state first-match /
2-state all-match violations in both profiles at 1, 4 and 8 workers, with folding,
memoization and hash-consing controls. Each word validates independently and a
changed intermediate state is rejected. Refusal fixtures check auto fallback,
including retained output for effectful sequence arguments. An exhaustive
13,230-case test now checks 13,016 admitted results and 214 explicit overlap
refusals. The admitted All results agree with an independent bounded partition
interpreter and with every partition satisfying the CHOOSE predicate. First is
checked against the least-prefix definition. The old occurrence-subset oracle
is rejected, as described below. Remaining work includes normalized partition
choice for overlapping All occurrences, plus fallback demand/comparison order
for compound, mixed, lazy or effectful arguments.

### Rejected greedy ReplaceAll rule (fresh review, 2026-10-28)

The implementation at `7e1195f1` is rejected: its claim that maximum-count,
leftmost absolute match positions reproduce the fallback is false. The pinned
TLC prints `<<0,1,2>>` for
`ReplaceAllSubSeqs(<<2>>, <<1,1>>, <<0,1,1,1>>)`, while that core prints
`<<0,2,1>>`. `core-replace/SeqReplacePartitionChoice` asserts the oracle result
and succeeds with 1 distinct / 2 generated / depth 1, code 2193 / exit 0.
The old core instead reports an invariant violation. The original rejected
commit and external review captures remain intact; its exhaustive test encoded
the same incorrect greedy assumption and did not establish parity.

The pinned source's comment says overlaps prefer the beginning, but the actual
`match(f)` contains **partition component indices**, not absolute text positions.
`T` maximizes the count of components equal to the pattern; the CHOOSE predicate
compares their sorted component indices. Grouping a nonmatching prefix into one
component can make different absolute positions tie. The exact remaining choice
then depends on the normalized set's enumeration. That choice must not be
replaced by an invented earliest-position rule.

Repair scope: preserve ReplaceFirst's separate least-prefix definition, and
admit ReplaceAll's early CASE arms, empty pattern, no occurrences and texts in
which nonempty pattern occurrences do not overlap. Refuse overlapping All
occurrences until the remaining partition choice is implemented and verified.
The independent test enumerates bounded weak compositions of the text into
at most Len(text) components, including empty components, then applies the
source's maximum-count and component-index predicate. TLC's normalized CHOOSE
order is modeled separately: tuple/function comparison first compares domain
length, then corresponding contents recursively (`TupleValue.compareTo`,
`FcnRcdValue.compareTo` and `Tool`'s BoundedChoose at `94d0c50`). It selects the
first satisfying normalized partition and also checks every satisfying
partition's output for admitted cases. The integer-alphabet model reproduces
the actual pinned `<<0,1,2>>` choice and observes the alternate tied-predicate
output; it does not use absolute match positions as its oracle. Refused overlaps
earn no coverage credit.

For nonempty patterns with no overlapping occurrences, every matching partition
component consumes a distinct occurrence. A partition can contain all of them:
make each occurrence one component, and each nonempty intervening gap one
component. All components consume at least one text element, so their number
is at most Len(text), satisfying the bound on R. Thus the maximum count equals
the number of occurrences, and every maximum-count partition must contain all
of them; omission cannot attain that count and repetition cannot reuse consumed
text. Coalescing or splitting the remaining gaps, or adding empty components,
cannot change the flattened replacement result. The coalesced partition also
minimizes each match's component index and witnesses a satisfying CHOOSE
predicate. The chosen partition's remaining normalized order is therefore
irrelevant to the admitted output. This is a uniqueness proof for the supported
subset, not an interpretation of the comment's general overlap claim.

ReplaceFirst has a separate proof: the least prefix containing a nonempty
fixed-length pattern ends at its earliest occurrence's end, so subtracting
Len(pattern) gives the earliest start. For the empty pattern the least prefix
is empty, giving index 1 and a prepended replacement. The corrected pure pinned
fixture includes the prefix/overlap case where First yields `<<0,2,1>>` while
All yields `<<0,1,2>>`. The formerly admitted all-overlap example remains a
separate pinned success fixture, now checked as a core refusal and clean Auto
fallback; it was moved, not discarded.

Corrective source mutations fail when the matcher skips overlap detection, when
the independent oracle reverses normalized tuple length ordering, or when the
repeatability guard is bypassed. The restored implementation and oracle pass
the same controls. Earlier mutation results and smoke/shadow runs on the
rejected greedy commit do not qualify the corrected source.
