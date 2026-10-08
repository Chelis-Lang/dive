# 0012: Value representation, order and fingerprints

- **Status:** accepted
- **Date:** 2026-10-08

## Context

`dive-value` is the value layer under every backend. It sits on the hot path of model checking, and it is where most of TLC's observable value semantics live. Those semantics come from `tlc2.value.impl` and `tlc2.module` at the pinned tlaplus commit `94d0c50`.

The facts below were all measured against the pinned oracle `tlc-2026.10.06.014338`. The fixtures are in `crates/dive-value/tests/fixtures/`.

### TLC's order

**Names are ordered by intern token, not by text.** TLC keeps every name (string, model value, record field, identifier) in one global table, `util.UniqueString`. `UniqueString.compareTo` returns `this.tok - other.tok`, so names compare by when they were first interned.

- Some names are already in the table before parsing starts, among them `TRUE`, `x`, `S`, `a`, `b`, `c`, `y` and `z`.
- The lexer then interns identifiers in source order, root module first.
- String-literal contents are interned later.
- Strings built at run time are interned when they are created.

Examples:

- `{"a", "b", "zebra", "Init", "c"}` prints in exactly that order.
- `[qfb |-> 1, qfa |-> 2]` prints `qfb` first when `qfb` is lexed first.
- `CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE` is `"zeta"`. It becomes `"alpha"` when an unused definition `Unused == "alpha"` precedes it (fixtures `ChooseIntern`, `ChooseInternUnused`).

**The rest of the order is built from that.** The structure is `compareTo` per class:

- **Sets:** by size first, then element by element.
- **Functions:** by domain size, then domain elements, then values. Tuples and records are functions, so tuples order by length first.
- **Model values:** an untyped model value is below every non-model value.
- **Booleans:** `FALSE < TRUE`.

`CHOOSE x \in S : P` returns the first element of `S` in this order that satisfies `P` (`Tool`, `OPCODE_bc` with `Ordering.NORMALIZED`).

### TLC's comparability

TLC refuses some comparisons, and the rules have three quirks.

1. **Comparisons across kinds are errors.**
   - `1 = "a"`, `{1} = 1` and `T_q1 = 1` all fail, where `T_q1` is a typed model value, named with the `x_name` pattern.
   - The message depends on the class of the left operand.

2. **Normalization is lazy, and it depends on insertion order.**
   - A set enumeration stays unnormalized until it is needed. Until then, membership is a linear scan using `equals`, so `1 \in {1, "a"}` is `TRUE`, while `"a" \in {1, 2}` is an error.
   - Normalization is a binary insertion sort (`ValueVec.sort`), so whether it raises an error depends on which pairs it happens to compare.

3. **Comparability is asymmetric between records and tuples.**
   - `TupleValue.compareTo(record)` compares sizes first. `RecordValue.compareTo(tuple)` fails at once.
   - So `{[qfa |-> 1, qfb |-> 1], <<1>>}` is a set, but `{<<1>>, [qfa |-> 1, qfb |-> 1]}` is an error.
   - Likewise, `<<1>> = [qfa |-> 1, qfb |-> 1]` is `FALSE`, but the reverse is an error (fixtures `Asym*`).

### TLC's printing

**Printing depends on the representation, not only on the value.**

- **Intervals:**
  - `1..3` prints as `1..3`, and an empty interval as `{}`.
  - `{1..3, {1, 2, 3}}` prints as `{1..3}`: when elements are equal, the first one is kept.
- **Lazy sets print expanded only when small:**
  - `SUBSET S` when `|S| < 7`;
  - `[S -> T]`, `[a : S]` and `S \X T` below 2000 elements.
- **Functions** print by domain:
  - `<<...>>` for domain `1..n`;
  - record syntax for "name" strings (`FcnRcdValue.isName`);
  - `(d :> v @@ ...)` otherwise.

**The pretty-printer is a fragile re-parser.** Every printed value goes through `tlc2.pprint`, which re-parses the one-line form and wraps it at 80 columns. It gives up, leaving the value on one line, on anything it cannot parse:

- a model value containing `_`;
- a negative interval;
- a string with an escaped quote.

### TLC's integers

Integers are Java `int`s.

- **Arithmetic overflow** is `EC.TLC_MODULE_OVERFLOW`, with messages such as `Overflow when computing 2147483647+1` and `--2147483648`.
- **Large literals:** a literal above 2^31−1 is `EC.TLC_INTEGER_TOO_BIG`, even under unary minus. So `-2147483648` is rejected, although its value fits.
- **Division** is floored (`Integers.Divide`) even in specs that extend only `Naturals`.
- **Sizes:** the size of a set must also fit in an `int`.
  - `Cardinality(SUBSET (1..31))` overflows.
  - So does `Cardinality(-2147483647..2147483647)`.

### TLC's fingerprints

Fingerprints are `tlc2.util.FP64` (64-bit Rabin), applied through each value's `fingerPrint(long)` method.

- A model value contributes its intern *token*.
- A string contributes the low byte of each UTF-16 unit.

## Decision

### 1. Representation

**`Value` is a 16-byte enum** with these variants:

- `Bool`, `Int(i64)`, `BigInt(Arc<BigInt>)`;
- `Str(Atom)`, `Model(Atom)`;
- `Set(Set)`, `Fcn(Fcn)`.

**It is cheap to share and safe across threads.**

- It is immutable and `Send + Sync`.
- Compound values are one `Arc`, so cloning is a reference-count increment and sharing is structural.
- `Atom`, `Set` and `Fcn` are 8 bytes each.
- `tests/layout.rs` enforces these sizes.

**Compound values cache their hash.** Sets and functions carry a precomputed or lazily cached 64-bit hash. Hashing is O(1), and inequality is usually decided by the first hash comparison.

**Dropping is iterative.** `SetInner` and `FcnInner` implement `Drop` by moving uniquely owned children onto a work list, so a value nested a million levels deep drops without recursion.

**Integers are numeric everywhere.** Constructors keep integers canonical, but the `Value` variants are public. Equality, ordering, hashing and fingerprints therefore compare integers by value, so a hand-built `BigInt(5)` equals `Int(5)`.

**Functions are canonical by domain:**

- a **tuple** for domain `1..n`, including `<<>>`;
- a **record** for a non-empty domain of strings, with fields sorted;
- a **general** function otherwise, with pairs sorted by argument.

Equal functions therefore have equal representations, and equality, hashing and fingerprints depend only on the function.

**A record remembers whether it is a record literal.** TLC keeps a record literal (`[f |-> e]`, or an element of `[f : S]`) as a `RecordValue`. A function with a string domain built any other way (`:>`, `@@`, `[x \in S |-> e]`, or an element of `[S -> T]`) stays a `FcnRcdValue`. The two behave differently:

- **Comparison with a tuple.** `RecordValue` refuses to compare with a non-record outright. `FcnRcdValue` compares sizes first. So `("qa" :> 1 @@ "qb" :> 2) = <<1>>` is `FALSE`, but `[qa |-> 1, qb |-> 2] = <<1>>` is an error (probes `RvFcnRcd1`–`RvFcnRcd8`).
- **Printing.** `RecordValue` always prints in record syntax. `FcnRcdValue` does so only when every field is a name.

The bit (`Fcn::is_record_literal`) selects TLC's rules in `tlc` and the printer. Equality, hashing and fingerprints ignore it.

**Sets are enumerated or lazy.**

- **Enumerated sets** are kept normalized: sorted and without duplicates.
- **Raw sets**, built with `Value::set_raw`, reproduce TLC's lazily normalized set enumerations, including linear-scan membership.
- **TLC's normalization state is observable**, so it is kept apart from dive's caches.
  - A raw set carries its own flag for TLC's `isNorm`.
  - Only the operations that normalize in TLC set it: comparison, equality, size, fingerprinting, printing and enumeration.
  - Hashing, `==`, `cmp_with` and `Debug` may compute and cache the normalized elements, but they never set the flag.
  - So `<<1>> \in {[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]}` stays TLC's error however often the set is hashed. It becomes `FALSE` only once the set is fingerprinted as part of a state (probes `RvMemUnnorm`, `RvMemNorm`).
- **Only successes are cached.** An enumeration error is recomputed under the caller's context, so its message never depends on who enumerated first.
- **Lazy sets** cover:
  - `a..b` and `SUBSET S`;
  - `[S -> T]`, `[a : S]` and `S \X T`;
  - `Nat`, `Int`, `STRING` and `Seq(S)`;
  - evaluator-driven filters and maps (the `DeferredSet` trait, implemented later by `dive-eval`).

**How lazy sets behave:**

- **Membership** is decided without enumerating, by porting each class's `member`.
- **Streaming.** Elements are generated in normalized order without materializing the set:
  - intervals ascending;
  - subsets by size, then lexicographically;
  - products, function sets and record sets as odometers.

  Equality, ordering, hashing and fingerprints consume these streams. Only the components (`S`, `T`) are materialized.
- **Materialization** happens only on request (`Set::elements`), at most once, and is cached.
- **Infinite sets** refuse enumeration (`DIVE-V0021`).
- **Size limit:** dive enumerates at most 2^31−1 elements in either mode (`DIVE-V0020`). Larger finite sets are compared by definition.

**No `unsafe`.**

### 2. Integers

The mode is a parameter (`Ctx::int_mode`), not a global.

**Big mode** is the owner-approved default (ADR 0010).

- An inline `i64` fast path uses checked operations and spills to `num-bigint` on overflow.
- The representation stays canonical: `BigInt` is used only outside the `i64` range.
- Exponentiation refuses results above 2^26 bits (`DIVE-V0030`). TLC would have overflowed long before that.

**TLC mode** checks every operation against 32-bit `int` semantics. It reproduces:

- which operations overflow;
- the exact messages;
- the literal rule.

**Why `num-bigint`:**

- It is licensed MIT/Apache-2.0.
- It is the ecosystem standard, with a small dependency footprint (`num-integer`, `num-traits`).
- Big integers are rare in TLA+ models, and every common value stays on the inline path, so big-integer speed does not matter.

**Alternatives considered:**

- `dashu`/`ibig` are faster at large multiplications, but that does not matter here.
- `rug` and `malachite` are LGPL, which the licence allowlist excludes.

### 3. Order of names, and so of all values

The order is a parameter (`Ctx::order`, `ValueOrder`). Both orders are built.

**`ValueOrder::Canonical` is the default.** The owner chose it in open question 8: a canonical order that does not depend on the module text. Names are ordered by the lexicographic order of their UTF-8 bytes, which is their Unicode scalar order. The order is total, stable, documented, and independent of parse and evaluation order.

**`ValueOrder::Tlc`** is `--choose-order=tlc`, which `--compat=tlc` includes. Names are ordered by intern token from the run's `Interner`.

**How TLC's order is reproduced.** The `Interner` numbers names in insertion order. Fed names in SANY/TLC's encounter order, it reproduces TLC's order exactly. The fixtures do this for the module text.

**Cost.** Comparing two names in either order is one integer or one byte-string comparison. Atoms are interned and compared by pointer first.

**The rest of the total order** extends TLC's partial order. Wherever TLC compares without error, the two agree; elsewhere the kinds rank:

```
model values < booleans < integers < strings < finite sets < functions < infinite sets
```

### 4. Comparability and normalization

`dive_value::tlc` ports `compareTo`, `equals`, `member` and `isEmpty` class by class. It dispatches on dive's equivalent of TLC's class for the left operand, and raises TLC's errors with TLC's messages.

TLC's comparability rules apply in both profiles. The better default proposed in open question 12 is not adopted.

**Normalization** (`Value::set`, function domains) is exact and fast:

- **Fast path.** A cheap structural "shape" join proves when every pair of elements is comparable in TLC. Then a stable sort with first-kept deduplication gives exactly TLC's result. This covers sets of integers, strings, records, tuples and so on.
- **Slow path.** Otherwise dive runs TLC's own algorithm with TLC's checked comparisons: binary insertion (`ValueVec.sort`), or swap-then-insert (`FcnRcdValue.normalize`). This reproduces TLC's result, or TLC's error, including the order-dependent and asymmetric cases.
- **Both profiles.** Whether TLC's insertion sort hits an incomparable pair depends on the order it sorts in. So the slow path always runs in TLC's name order (intern tokens), and the result is then re-sorted in the run's order. With names interned in TLC's encounter order, the default profile raises exactly TLC's errors too (probe `RvCanonAsym`).
- **Residual difference.** Membership in a normalized set is a binary search, and its comparisons follow the run's order. In the canonical profile, a membership test that mixes incomparable values can therefore meet a different pair from TLC, and give TLC's error where TLC answers, or the reverse. This is recorded under D-009 in `docs/tlc-differences.md`.

### 5. Equality, the order and hashing

**`Value` implements total semantic `Eq`/`Hash`, distinct from TLC's checked `equals`.**

- **Never errors.** Values TLC refuses to compare are simply unequal.
- **Looks through representation.** An interval equals the enumerated set with the same elements.
- **Iterative.** `==` and `cmp_with` use an explicit work list over enumerated sets and functions, so any nesting is safe.
- **Never changes anything observable** (see the `isNorm` flag above).

**Sets fall in three classes:**

| Class | Members | Compared by |
|---|---|---|
| Intervals | `a..b` | bounds |
| Finite | at most 2^31−1 elements | cardinality, then element by element in normalized order, streamed |
| Structural | infinite, larger, or unenumerable (such as a literal `{1, "a"}` that TLC cannot normalize, or an unenumerable deferred set) | definition |

So `0..2147483646 = 0..2147483646` takes constant time, as in TLC.

**Deferred sets have a stable key.** `DeferredSet::key` replaces allocation addresses, so the order never depends on allocation.

**The hash is consistent with `Eq`.** It is order-independent for sets and functions (sums of mixed element hashes), so equal values hash equally whatever their representation and whichever order is active.

- **Large sets.** A set of more than 65,536 elements hashes by its cardinality alone. Hashing a huge lazy set is then O(1), and equality still compares the elements.
- **`Seq({})`.** It is the finite set `{<<>>}`, and hashes as such.

### 6. Fingerprints

dive's fingerprint is TLC's FP64.

- `Fp64` ports `tlc2.util.FP64`, including its 131 polynomials.
- `TlcFingerprint` ports every value's `fingerPrint(long)`.
- Both sit behind the `Fingerprinter` trait, so another fingerprint can be added later.

**Why FP64:**

- The goldens pin `-fp 0`.
- TLC-identical fingerprints allow TLC-exact collision-bound reporting under `--compat=tlc` ("based on the actual fingerprints").
- They allow state-by-state cross-checking against TLC when a conformance run disagrees.
- At about 1 µs for a small state they are not a bottleneck yet.

**The traversal is iterative and streams lazy sets.** `0..2147483646` fingerprints in constant memory, as TLC's `IntervalValue.fingerPrint` does.

**Verification.**

- **All 64 bits.** State fingerprints match TLC's on the 28 states of `FpFull`. The reference is TLC's own `-fp 0 -dump dot` output, whose node ids are `TLCState.fingerPrint()`. The states cover integers at the 32-bit bounds, booleans, ASCII, non-ASCII and non-BMP strings, sets, intervals, tuples, records, functions, nested sets, `SUBSET`, `[S -> T]`, `\X` and `[a : S]`.
- **Low 32 bits.** `TLCFP` matches TLC on 22 further fixture values.

**Model values** fingerprint their intern token. The fingerprint matches TLC's only when the interner's absolute numbering matches TLC's, which needs the front end's encounter-order feed (see Consequences).

**Big integers** outside 32 bits use a dive-only kind byte, `0x80`, that TLC never emits.

**A future speed-up.** FP64 is linear over GF(2), so a later optimization can cache per-value sub-fingerprints and combine them by multiplying by `x^(8n)`. The result would not change.

**Cached value fingerprints.** Each set and function caches its own fingerprint (`TlcFingerprint::fingerprint_cached`), tagged with the polynomial and `Ctx` it was computed under, so states that share a value fingerprint it once. `dive-store`'s two-level state fingerprint is built on it (ADR 0015 §5). The cache is two atomics per compound value, filled once and never replaced; errors are not cached.

**Byte encodings.** `codec` encodes values for state storage (ADR 0015 §5, §8). The lossless form keeps each value's representation, including `isNorm` and the record-literal bit. The canonical form is the exact-store key: equal values, and only equal values, have equal keys within a run. Names are written as intern tokens and resolved with `Interner::resolve`.

### 7. Printing and JSON

**`print::to_tlc_string`** ports every `toString`, including the expansion rules.

**`print::ppr`** ports `tlc2.pprint`, quirks included. It counts positions in UTF-16 code units, as Java does.

**`print::format_state`** ports `TLCStateMut.toString`.

**`json::to_tlc_json`** ports `tlc2.module.Json.getNode` with Gson's escaping. That is the value part of `-dumpTrace json`.

### 8. Nesting

**Most operations are iterative.** Dropping, `==`, `cmp_with`, hashing and fingerprinting handle any depth.

**The rest recurse, with a limit.** TLC's checked operations (`tlc::compare`, `equals`, `member`), printing and JSON recurse. They refuse values nested more than `MAX_DEPTH` = 500 levels deep, with `DIVE-V0030`, instead of overflowing the stack.

- **Why 500.** It is safe on a 2 MiB thread stack in debug builds. The measured limit there is about 700 levels.
- **The pretty-printer** falls back to the one-line form for deeper values. TLC does the same whenever its re-parser fails.
- **TLC's own limit** is the JVM stack (`-Xss`, 8 MiB in the harness), so TLC handles deeper values. This is listed as a dive limitation in `docs/tlc-differences.md`.

## Consequences

### Verified

All of the following is verified without Java (`crates/dive-value/tests/tlc_differential.rs`, `review_adversarial.rs`, `limits.rs`):

- 122 printed values;
- 55 error messages, byte for byte, covering comparison, equality, membership, overflow, size and literal errors;
- 4 order-dependent outcomes, plus the 14 review probes `Rv*`, with outputs and errors;
- the counterexample states and their JSON;
- all 64 bits of 28 state fingerprints;
- the reviewer's adversarial tests, including a 3,000-case fuzz test of every public operation for panic-freedom and cross-API consistency;
- dropping, `==` and fingerprints at a million and 200,000 levels of nesting.

Property tests cover:

- totality and consistency of the order;
- agreement of hashing with `==`;
- normalization;
- set-algebra laws;
- lazy versus enumerated membership and fingerprints;
- both integer modes.

### For the front end

TLC's order needs the right feed. `--choose-order=tlc` is exact only when `dive-sema`/`dive-eval` intern names in SANY/TLC's encounter order. Measured so far:

1. a set of names pre-interned before parsing;
2. identifiers in lexing order, root module first;
3. string-literal contents;
4. strings created at run time.

Reproducing the full sequence, including the pre-interned set, standard modules, cfg values and model values, is a tracked task for `dive-sema` and `dive-config`. With several workers, TLC's run-time interning order is itself nondeterministic. dive's canonical order has none of these problems.

### Known gaps

- **Unnormalized records.** TLC prints a record in construction order until it is normalized, for example in some error messages. dive's records are always normalized.
- **Message wording for explicit `1..n` domains.** TLC holds a function with domain `1..n` in one of three forms:
  - a `TupleValue` (`<<...>>`);
  - a `FcnRcdValue` with an interval domain (`[i \in 1..n |-> e]`);
  - a `FcnRcdValue` with an explicit domain (`:>`/`@@`, `[x \in {1, 2} |-> e]`).

  dive models the first two exactly (probes `RvFcnRcd5`–`RvFcnRcd7`). For the third, a comparison error with a non-integer domain is worded `Attempted to compare integer 1 with non-integer: ...` in TLC and `Attempted to compare integer with non-integer: ...` in dive. Which comparisons fail, and the results, are the same; this follows from the class-by-class port, but no probe covers it.
- **Canonical-profile membership errors** (§4).
- **Unicode letter classes.** Java's `Character.isLetter` differs from Unicode's alphabetic property outside ASCII. This only affects the pretty-printer and record-syntax decisions for non-ASCII names.
- **Lazy set operators.** `UNION`, `\cup` and `\cap` on infinite sets, `Seq` enumeration, `KSubset` and `FcnLambdaValue` are left to `dive-eval` (`DeferredSet`).

### Performance

Measured on an Apple M3 (8 cores, 24 GB), rustc 1.99.0, with `cargo bench -p dive-value --bench values`. These are divan medians on a shared machine, so treat them as indicative.

| Operation | Median |
|---|---|
| clone a set of 100 records | 2.2 ns |
| hash a set of 100 records | 4.7 ns |
| `=` of two equal 100-record sets, separate allocations | 3.1 µs |
| TLC `compareTo` of two records | 18 ns |
| membership: 1000-element set | 83 ns |
| membership: interval | 8.6 ns |
| normalize 10 integers | 265 ns |
| normalize 100 integers | 3.0 µs |
| normalize 1000 integers | 41–70 µs |
| normalize 100 records | 18 µs |
| FP64 of a 3-variable state | 0.95 µs |
| enumerate `SUBSET` of 10 elements (1024 sets) | 162 µs |
| print 20 records | 31 µs |
| `0..2147483646 = 0..2147483646` (Rust `==`, `tlc::equals`, `cmp_with`) | constant time (under 1 ms in a debug build) |

These numbers are after the review fixes. They pay for the record-literal dispatch, the depth guard on TLC's checked operations and the separate normalization flag. That roughly doubles TLC `compareTo` and membership against the first version, which got those cases wrong.

No comparison with TLC or the other Rust checkers was run for this ADR. That belongs to `dive-bench` once states can be explored end to end.
