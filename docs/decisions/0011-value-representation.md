# 0011: Value representation, order and fingerprints

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

**Functions are canonical by domain:**

- a **tuple** for domain `1..n`, including `<<>>`;
- a **record** for a non-empty domain of strings, with fields sorted;
- a **general** function otherwise, with pairs sorted by argument.

Equal functions therefore have equal representations, and printing by domain reproduces TLC's printing.

**Sets are enumerated or lazy.**

- **Enumerated sets** are kept normalized: sorted and without duplicates.
- **Raw sets**, built with `Value::set_raw`, reproduce TLC's lazily normalized set enumerations, including linear-scan membership.
- **Lazy sets** cover:
  - `a..b` and `SUBSET S`;
  - `[S -> T]`, `[a : S]` and `S \X T`;
  - `Nat`, `Int`, `STRING` and `Seq(S)`;
  - evaluator-driven filters and maps (the `DeferredSet` trait, implemented later by `dive-eval`).

**How lazy sets behave:**

- **Membership** is decided without enumerating, by porting each class's `member`.
- **Enumeration** happens at most once (`OnceLock`) and produces normalized order directly:
  - subsets by size, then lexicographically;
  - products, function sets and record sets as odometers.
- **Infinite sets** refuse enumeration (`DIVE-V0021`).
- **Size limit:** dive enumerates at most 2^31−1 elements in either mode (`DIVE-V0020`).

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

The same rules apply in both profiles. The better default proposed in open question 9 is not adopted.

**Normalization** (`Value::set`, function domains) is exact and fast:

- **Fast path.** A cheap structural "shape" join proves when every pair of elements is comparable in TLC. Then a stable sort with first-kept deduplication gives exactly TLC's result. This covers sets of integers, strings, records, tuples and so on.
- **Slow path.** Otherwise dive runs TLC's own algorithm with TLC's checked comparisons: binary insertion (`ValueVec.sort`), or swap-then-insert (`FcnRcdValue.normalize`). This reproduces TLC's result, or TLC's error, including the order-dependent and asymmetric cases.

### 5. Equality and hashing

**`Value` implements total semantic `Eq`/`Hash`, distinct from TLC's checked `equals`.**

- **Never errors.** Values TLC refuses to compare are simply unequal.
- **Looks through representation.** An interval equals the enumerated set with the same elements.

**The hash is consistent with `Eq`.** It is order-independent for sets and functions (sums of mixed element hashes), so equal values hash equally whatever their representation and whichever order is active.

**Lazy sets hash their enumeration.** That is computed once and cached. A set that cannot be enumerated hashes and compares by its definition.

### 6. Fingerprints

dive's fingerprint is TLC's FP64.

- `Fp64` ports `tlc2.util.FP64`, including its 131 polynomials.
- `TlcFingerprint` ports every value's `fingerPrint(long)`.
- Both sit behind the `Fingerprinter` trait, so another fingerprint can be added later.

**Why FP64:**

- The goldens pin `-fp 0`.
- TLC-identical fingerprints allow TLC-exact collision-bound reporting under `--compat=tlc` ("based on the actual fingerprints").
- They allow state-by-state cross-checking against TLC when a conformance run disagrees.
- At about 0.44 µs for a small state they are not a bottleneck yet.

**Verification.** The low 32 bits (`TLCFP`) match TLC on 22 fixture values. Those include records, nested sets, lazy sets and non-ASCII strings.

**Model values** fingerprint their intern token. The fingerprint matches TLC's only when the interner's absolute numbering matches TLC's, which needs the front end's encounter-order feed (see Consequences).

**Big integers** outside 32 bits use a dive-only kind byte, `0x80`, that TLC never emits.

**A future speed-up.** FP64 is linear over GF(2), so a later optimization can cache per-value sub-fingerprints and combine them by multiplying by `x^(8n)`. The result would not change.

### 7. Printing and JSON

**`print::to_tlc_string`** ports every `toString`, including the expansion rules.

**`print::ppr`** ports `tlc2.pprint`, quirks included. It counts positions in UTF-16 code units, as Java does.

**`print::format_state`** ports `TLCStateMut.toString`.

**`json::to_tlc_json`** ports `tlc2.module.Json.getNode` with Gson's escaping. That is the value part of `-dumpTrace json`.

## Consequences

### Verified

All of the following is verified without Java (`crates/dive-value/tests/tlc_differential.rs`):

- 122 printed values;
- 55 error messages, byte for byte, covering comparison, equality, membership, overflow, size and literal errors;
- 4 order-dependent outcomes;
- the counterexample states and their JSON.

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
- **Representation-dependent message wording.** A function TLC holds with an explicit `1..n` domain (built with `:>`/`@@`) gives slightly different message wording from a `TupleValue` in rare comparison errors. The error class is the same.
- **Unicode letter classes.** Java's `Character.isLetter` differs from Unicode's alphabetic property outside ASCII. This only affects the pretty-printer and record-syntax decisions for non-ASCII names.
- **Lazy set operators.** `UNION`, `\cup` and `\cap` on infinite sets, `Seq` enumeration, `KSubset` and `FcnLambdaValue` are left to `dive-eval` (`DeferredSet`).

### Performance

Measured on an Apple M3 (8 cores, 24 GB), rustc 1.99.0, with `cargo bench -p dive-value --bench values`. These are divan medians on a shared machine, so treat them as indicative.

| Operation | Median |
|---|---|
| clone a set of 100 records | 2.8 ns |
| hash a set of 100 records | 3.6 ns |
| `=` of two equal 100-record sets, separate allocations | 2.4 µs |
| TLC `compareTo` of two records | 6.5 ns |
| membership: 1000-element set | 45 ns |
| membership: interval | 4.4 ns |
| normalize 10 integers | 166 ns |
| normalize 100 integers | 4.1 µs |
| normalize 1000 integers | 55 µs |
| normalize 100 records | 10 µs |
| FP64 of a 3-variable state | 0.44 µs |
| enumerate `SUBSET` of 10 elements (1024 sets) | 216 µs |
| print 20 records | 24 µs |

No comparison with TLC or the other Rust checkers was run for this ADR. That belongs to `dive-bench` once states can be explored end to end.
