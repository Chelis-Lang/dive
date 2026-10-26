# Value model

Status: M0 spike implemented in `crates/dive-core` (`handle`, `strings`,
`store`, `arena`, `node`, `heap`) on `core/m0-spike`; results in
`m0-results.md`. This document covers strategies S1, S2, S4, S5 and S7 from
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
| 3 | Model value | id (u32) into the string table; the entry carries the TLC model-value type |
| 4 | Persistent compound | shard (8 bits) and index (up to 48 bits) in the persistent store |
| 5 | Transient compound | epoch (16 bits) and index (32 bits) in the owning worker's arena |
| 6 | Big integer | a persistent or transient node holding a `BigInt` |
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
  - **Node storage.** An append-only vector with stable indices. Reads need no
    lock. Candidate crates are `boxcar`, or a chunked vector of fixed-size
    blocks behind a published length; this is decided in M0 by the 8-thread
    microbenchmark.
  - **Deduplication index.** A table from hash to node index. It is consulted
    and updated only while interning, under a per-shard `Mutex`.
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
- Transient nodes may reference persistent nodes and other transient nodes of
  the same epoch. Persistent nodes never reference transient nodes.

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

- **Default profile.** Integers are inline while they fit 61 bits; arithmetic
  checks for overflow and promotes to a big-integer node.
- **`--int=tlc`.** TLC's 32-bit semantics: arithmetic outside the TLC range
  raises TLC's overflow error, with the same error class.

The compiler emits integer fast paths per profile (`04-plan-compiler.md`).

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

## 11. Next step

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
- Big integers are refused, not represented.
- The literal flag for tuples and records is not part of identity.
- The store has no compaction. Memo tables are cleared when the store remaps.

Next (M1, stream 1):
1. Freeze the value interface: handles, `Heap`, cursors and views.
2. Move `PNode` data inline into the chunk, to remove one pointer chase per
   view. `Heap::view` and `Store::node` are the top self-time entries on
   MCLamportMutex and GameOfLife.
3. Add lazy function values (`[x \in S |-> e]` applied before it is
   materialized), big integers, and the compat-order intern path.
