# Value model

Status: M0 spike on `main` in `crates/dive-core` (`handle`, `strings`,
`store`, `arena`, `node`, `heap`); results in `m0-results.md`. M1 (stream 1):
the value interface is frozen in §11, node data is inline in the store
(step 2), the arena rolls back rejected candidates (step 3), and lazy values
exist on the value side (step 4), integers have any size (step 5), and
strings can compare by TLC token (step 6); the remaining M1 steps are in §12. This document covers strategies S1, S2, S4, S5 and S7 from
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
  - lazy set classes and normalization state, because both affect membership
    errors;
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

## 3. Regions

### 3.1 Persistent store

The persistent store holds every value reachable from a retained state, plus
all constants.

- **Shards.** It is split into 256 shards, selected by the top bits of the
  value hash. Each shard has two parts:
  - **Node storage.** One append-only column per size class: nodes with at
    most 4 handles, at most 8, and more. A column is a chunked vector of
    write-once `OnceLock` slots (chunk `k` holds `2^(4+k)` slots). The first
    two classes hold the node header and its handles inline in the slot; the
    last boxes its handles. Reads need no lock: a read is one chunk-pointer
    load and one slot load. The class is part of the persistent handle.
  - **Deduplication index.** An open-addressing table of `u64` entries (the
    low 32 bits of the hash, and the class and index), consulted and updated
    only while interning, under a per-shard `Mutex` on its own cache line.
- **Hash-consing.** A compound is interned once. Interning looks up the hash,
  compares candidates structurally, and inserts if none matches. Two retained
  values are equal exactly when their handles are equal.
- **Intern after claim.** A successor's values are interned only after its
  state fingerprint wins the seen-set claim (`07-engine.md`). Duplicate
  successors, 74% of them on MCLamportMutex, never touch the store. Interning
  is therefore rare relative to evaluation, and the shard mutex is not a hot
  lock.
- **Node contents.** Every node records its kind, its element or entry handles,
  its hash (section 5), and a lazily computed FP64 (section 6).
- **Permanent region.** Constants and config values are interned at startup
  into a region that is never compacted.

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
- **Large functions and sequences.** Above a size threshold (initially 64),
  the value array is a persistent 32-way tree. An `EXCEPT` copies one path,
  O(log32 n). Interned trees share their unchanged chunks across every state
  that references them.

**Records.**
- The sorted field names are interned as a shape. A record is a shape id plus
  a value array.
- The compiler resolves field access by a constant name to an index when the
  shape is known statically. Otherwise it reads the index from the shape's
  table.
- TLC distinguishes a record literal (`RecordValue`) from a string-domain
  function (`FcnRcdValue`), and a tuple (`TupleValue`) from a `1..n` function.
  That distinction is a one-bit literal flag, as in `dive-value` today
  (`crates/dive-value/src/value.rs:752`).

**Tuples and sequences** are functions with domain `1..n`, plus the literal
flag.

**Big integers** are nodes holding a `BigInt`, used only outside the 61-bit
inline range in the default profile.

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

## 8. Lazy views and normalization state

TLC distinguishes how a set is represented, and the distinction is observable:

- membership of a value in a `SubsetValue`, `IntervalValue`, `SetOfFcnsValue`
  or `SetOfRcdsValue` follows that class's `member` rules;
- membership of a record in a set that has not been normalized can raise an
  error that the normalized set does not (fixtures `RvMemUnnorm` and
  `RvMemNorm`; ADR 0012).

Identity is extensional: equal sets have one handle. The representation class
and normalization flag travel separately, as a **lazy view**:

- **Transients.** A transient symbolic set (`a..b`, `SUBSET S`, `[D -> S]`,
  record sets, filters and maps, unions) is an arena node of a symbolic kind.
  It is never enumerated unless the program forces enumeration. Membership
  tests on it are symbolic (S5).
- **State slots.** When a state retains a set whose lazy view the contract
  requires, the state carries the identity handle in its slot. It also carries
  a view handle in a sparse side table keyed by variable. Hashing and equality
  use only the identity handle. Membership and the error paths consult the
  view.
- **Memoization.** S6 cache keys include the view handles of their inputs
  (`06-memoization.md`).

Which cases need a retained view is listed in `02-semantic-contract.md`
(normalization and membership). In M0 the view side table is empty, because
the spike models do not need it.

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

Compaction is planned for M2, as an option that the memory budget triggers. At
a level boundary it:
1. copies the values reachable from the frontier and the permanent region into
   fresh shards;
2. remaps the frontier handles;
3. clears the memoization caches.

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
| `poisoned` | a stale or foreign handle was read, or a lazy value was hashed (a bug) | check | check | check after each successor |

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
   before the probe). Nothing else interns state values. The compiler interns
   constants with `intern_const`.
4. **Deep close on intern.** `intern` walks a value children first, interning
   each child, and materializes finite symbolic sets (TLC fingerprints a state
   by normalizing it, so this adds no observable behaviour). A lazy value
   is closed by the evaluator first: for each `Lambda` or `SetPred`
   it evaluates the materialization, then rebuilds the parents with
   `rebuild`. `intern` given a lazy value is an internal error.
5. **Persistent values are immutable and never reference transients.** They
   live until the store is dropped (compaction is M2, §10).
6. **Views and identity** (§8). A value's identity handle is its normalized,
   closed form. A lazy view (a symbolic class, or TLC's `isNorm` state) is
   kept beside it, never in it: `hash`, `eq`, `cmp` and `intern` ignore
   `View::flags` (except to refuse a lazy value). The engine keeps a state slot's view handle in a
   side table when the plan asks for it.
7. **Sharing.** `Store` is `Sync` and shared through `Arc`. Interning takes a
   per-shard lock; reads take none. A persistent handle may be passed to any
   worker once it has been returned by `intern`.

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
- The literal flag for tuples and records is not part of identity.
- The store has no compaction. Memo tables are cleared when the store remaps.

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
   Still open on the value side: the `isNorm` and record/tuple literal bits
   of `View::flags` (C1, C5 of `02-semantic-contract.md`), which `--compat=tlc`
   needs (M2).
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

Next (stream 1, after the open PRs merge):
- The `isNorm` and record/tuple literal bits of `View::flags` (C1, C5 of
  `02-semantic-contract.md`), with the view side table, for `--compat=tlc`.
- Persistent 32-way trees for functions above 64 entries (§4), so that
  `EXCEPT` copies one path; measure on MCbtree and EWD998PCal first.
- An x86 measurement of steps 2 and 3 (`09-performance-method.md`):
  `store_views`, `store_scaling`, and the four spike models' time and RSS.
