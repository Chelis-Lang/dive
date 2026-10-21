# 0110: Persistent finite-function domains

- **Status:** proposed (experimental performance candidate)
- **Date:** 2026-10-21

## Context

General finite functions store an ordered key and a range value together in
one array. EXCEPT reconstructs that array and repeats key normalization;
checked selection allocates another pair vector, and default-profile function
membership repeatedly proves the domain comparable. Retained ADR 0104 profiles
identify this related work across the reduced models, but do not establish a
performance improvement. The experiment must meet ADR 0104's complete eight-cell
5% geometric instruction threshold before production acceptance.

TLC's checked comparison order and its mutable normalization history constrain
lookup changes. A sorted implementation index is not evidence that TLC has
normalized a function or a nested set. Unknown/provider-backed keys must retain
the existing checked comparison and forcing boundaries.

## Decision

In this experiment, a general function stores immutable keys in an `Arc`-owned
private domain descriptor and keeps its range in a separate boxed slice. Tuple
and Record representations and their public borrowed slices stay unchanged.
Same-context EXCEPT may share a proven canonical domain while replacing range
values; the eager native hash is recomputed and the fingerprint cache is fresh.
Different-context or unproved updates retain the original reconstruction path.

Domain facts use only already available eager values. Conservative comparability
and provider-free TLC-order permutations are cached separately from any
observable normalization flag. Checked comparison still visits unknown keys in
the original order, and the existing refusal for large incomparable arguments
remains. Borrowed function views avoid copying range values just to inspect or
reorder a domain; unknown cross-order views keep the original owned sort.

Two exact index certificates are included. A dense small-integer interval must
match every stored key position. A Cartesian integer-tuple domain must have
checked bounds/cardinality, and every key's mixed-radix position must equal its
actual stored range position. Shape or cardinality alone is insufficient.
Incompatible queries yield `DomainLookup::Unknown`, never an inferred miss.
The Cartesian query checks every coordinate for compatibility even after a
coordinate is out of bounds. Domain equality shortcuts are limited to exact,
settled integer intervals/products. TLC membership preserves its existing size
and factor preparation before shortcuts, then checks range membership normally.

Auxiliary metadata adds no range, state, evaluator, host or provider ownership
beyond the original keys. Those keys can themselves retain deferred providers
and their contexts. Provider-bearing keys keep unknown order certificates and
the original fallbacks; canonical EXCEPT does not share their domains. Domain key release
participates in the existing bounded-native/iterative destruction scheme,
including a final owner outside a function. Encodings, fingerprints, literal
flags and function kinds retain their existing meanings. No dependency, flag,
wire schema, capacity policy or admission rule changes.

## Consequences

Repeated updates can retain immutable domain facts and keys. Exact indexing and
borrowed views can eliminate repeated key work while retaining complete checked
fallbacks. Initial construction gains an allocation and separating keys/ranges
can cost more on workloads without reuse. Lazy metadata acquisition also has a
cost. Neither the historical cost family nor its optimistic selected-work
scenario predicts the experiment's gain.

Focused controls cover malformed/sparse/reordered/overflowing index inputs,
incompatible query/error precedence, context and literal preservation,
provider/normalization boundaries, shared descriptor ownership, hash/FP/codec
agreement and deep last-owner destruction. Source review precedes expensive
profiling. Acceptance requires all eight measured cells with identical results,
no material regression, and the full checker gate/both small baselines. A failed
performance candidate remains unmerged with its negative evidence preserved.

## Instruction screening evidence

The complete 2026-10-21 screen compares `510dce00` with `239db53f` across four
reduced ADR 0104 models in both profiles at one worker. All 16 arms completed
with identical verdicts/counts/depth; geometric instruction reduction is
5.365455%, and every cell improves. Original before-arm source/build/tool/input
receipts and all raw arm hashes are retained. See the
[compact evidence](../survey/data/persistent-function-domains-2026-10-21.json).

The conditional full checker gate and both small-baseline comparisons remain
pending. This ADR remains proposed until that evidence and final source review
are complete. Instruction screening alone is not production acceptance or a
wall-clock, TLC-throughput, parallel-scaling or release milestone claim.
