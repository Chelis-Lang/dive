# 0154: Admit native atom input rows by Core materialization facts

- **Status:** accepted
- **Date:** 2026-11-07

Amends ADR [0134](0134-native-atom-row-proofs.md) and the input-row clause of
ADR [0129](0129-native-stable-temporal-predicates.md).

## Context

Native liveness atoms (ADR 0129) required every input row to pass a set-free
shape check: Bool, Int and run-owned string leaves under integer-domain or
scalar-keyed functions, within a 4,096-component and string-byte walk. Any set
or model value failed it. On SDP New_Solution MC all 3 prepared atoms fell back
on all 8,643 demands (`row_proofs.certified` 0). On MCFlashWithMutexLive all 16
fell back. A one-variable probe ran 28/28 atoms natively with `s = <<1>>` and
0/28 with `s = {1}`.

The shape check is stricter than needed. Native atoms run on Core's own stored
rows, with the same evaluator and value order that safety checking already
applied to those rows. Builders have marked eager values with
`flags::CONCRETE` since #842: an eager Set, IntFcn or Fcn whose every
descendant is materialized. Liveness admission converts every retained row
to a reference value before it retains the row, and refuses the run when that
conversion fails. So every row that reaches an atom has a reference value.

## Decision

A row is native input when every handle is materialized as
`compare::materialized` defines it: no lazy, symbolic or unnormalized
(SetEnum) node and no unknown name at any depth. Sets, model values, big
integers and functions with any key class qualify. Transient and empty
handles, and a poisoned heap, decline.

- `Heap::proved_concrete` accepts a scalar, a known name, or a persistent node
  carrying `CONCRETE` without a walk.
- Otherwise `Heap::materialized_within` walks the row iteratively, visiting at
  most 16,384 handles. This covers rows whose bit was cleared by snapshot
  restore, owned exchange or candidate decoding, such as multi-worker rows.
- Compiled string, function and Select-key constants use the same predicate.

Native truth is Core's evaluation of the stored representation. Its set
iteration, CHOOSE and enumeration order is `Heap::cmp` under the run's string
order. A property test checks that it agrees with `Value::cmp_with` under
`ValueOrder::Canonical` and `Tlc` on graph conversions of generated nested
values. The values mix model values, booleans, integers, strings, sets, tuples,
records and general functions.

Len precharges its byte length to Machine fuel. Comparison, prefix and
replacement operators on strings do not. So the string allowance stays in a
narrower form. While every text in the Store is at most 4,096 bytes, one scan
is bounded and the bit is trusted. Once a longer text exists, every row is
walked, and each string or model-name occurrence counts against a 4,096-byte
row allowance.

The ADR 0134 positive-bit cache is unchanged: the exact graph borrow, the
Store-owner and poison checks on every hit, the 1 MiB cap, lazy fallible
allocation, and a fresh Reader per analysis. Errors, fuel exhaustion, warnings
and printed output still discard the attempt and run the reference evaluator,
which keeps TLC's error class and message. The analysis Machine keeps its
separate owners, with memo, step memo and folding off, and never interns into
the producer Store. ENABLED atoms keep `Compiler::enabled`, the `enabled_substs`
identity and cutoff proof, and the B2/B4 compat refusals. The trace and
products path and the dive-trace layering are unchanged.

## Consequences

SDP's 3 atoms and Flash's 16 atoms now run natively. Most atoms are still
rejected at prepare by the dive-eval stability certificate. Replacing that
certificate is the next step of #845.

`DIVE_NATIVE_ATOM_AUDIT=1` is the acceptance check for this widening. It
compares every native truth with the reference evaluator, at 1 and 4 workers
in both profiles. Rows with more than 16,384 nodes that lack the bit, and
rows holding long names, still fall back.
