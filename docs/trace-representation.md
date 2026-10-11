# Stored trace transport (opt-in version 2)

Version 1 stays the default JSON trace. Use version 2 when an admitted mixed
record/tuple set or a function with tuple/record domain must retain its stored
representation for independent replay:

```sh
dive check conformance/probes/trace-representation-v2/specs/Mixed/Transport.tla --compat=tlc --seed 7 --fp 0 --workers 8 --trace-format representation-v2 --output json > mixed-result.json
# The expected invariant violation exits 12; retain the document.
dive validate-trace conformance/probes/trace-representation-v2/specs/Mixed/Transport.tla --compat=tlc --seed 7 --fp 0 --trace mixed-result.json --output json
```

The validator accepts the embedded trace from the whole report. Repeat without
`--compat=tlc` for the default profile (the checker violation then exits 10).
`--trace-format v1` explicitly selects the legacy counterpart. This changes
transport only, not the model semantics, worker execution, stopping counts,
comparison policy or profile. The checker report schema remains unchanged;
only `counterexample.trace.version` becomes 2. Writing final temporal reports
with this transport is not yet implemented and refuses explicitly.

A successfully written transport has
`counterexample.trace_encoding.status: "encoded"` and
`semantic_validation: false`. Independent validation must separately return
`result: valid`. If the requested stored shape cannot be transported, the trace
is null and the transport receipt instead says `status: unsupported` with its
reason. The concrete checker outcome is not converted into a certificate.

Rust callers set `CheckRequest.trace_encoding` to
`dive_api::trace::TraceEncoding::RepresentationV2`. For an already read concrete
word, use `SpecModel::to_doc_with_encoding(&trace, encoding)`; the default
`to_doc` still writes version 1. The compiled
[API example](../crates/dive-api/examples/trace_representation.rs) performs both
profile checks and independent replay. External implementation words may be
constructed with the explicit version-2 schema, but never gain validation merely
by including claimed representation flags.

## Schema and admission

The envelope remains `format: "dive-trace"`, with `version: 2`, ordered variables,
complete state assignments, optional action labels, length and an ending claim.
Unknown envelope fields reject; `validated: true` is not an accepted shortcut.
The value object describes stored representation:

| Value representation | Object fields |
|---|---|
| literal tuple | `repr: "tuple"`, `items` |
| tuple-domain function | `repr: "tuple-fcn"`, `items` |
| literal record | `repr: "record"`, `order`, `items: [[field, value], ...]` |
| string-domain function | `repr: "record-fcn"`, `order`, `items` |
| general function | `repr: "fcn"`, `order`, `items: [[domain, value], ...]` |
| normalized explicit set | `repr: "set"`, `order`, `items`, `witness` |
| Raw set | `repr: "raw-set"`, `order`, `items`, `normalized` |

Booleans, i64 integers and strings use scalars; larger integers and model values
use `{"int":"decimal"}` and `{"model":"name"}`. `order` is `tlc` or
`canonical` and must match the bound profile. Every name must already be in the
receiving model's interner; the document cannot install its own token table.
Extra/unknown fields and duplicate domains/fields reject. Constructors verify
literal/domain claims rather than trusting a flag to bypass them.

A set witness is a permutation of indices into `items`. The decoder constructs
that permutation using the existing set constructor, then compares its exact
stored output with the declared order and nested representation. Duplicate,
missing, foreign or wrong-result indices reject. The encoder tries stored order
and its reverse, at most two candidates. Failure to prove a witness refuses;
it never total-order-accepts a set that the existing constructor rejects.
A Raw `normalized: true` causes actual normalization and refuses its errors.
No caller flag certifies normalization, Init, Next or an ending condition.

Admission is bounded: value depth 32, 16,384 nodes, 512 compound members and
states/variables, conservative JSON charge 4 MiB (128 bytes/node plus at most six
bytes per UTF-8 text byte), and 1,048,576 conservative comparison-node work units
per value. The whole word shares its JSON byte/node budget before value
construction or state cloning. JSON parser/input allocations, search memory and
exact CPU time are outside those budgets. Unknown providers, deferred, symbolic
and interval sets remain refused. Larger admitted searches may consequently
have an unavailable v2 transport; inspect its receipt.

## Migration and remaining limits

Version-1 evidence remains unchanged: the earlier full selection retained 132
originals/396 altered words, with 36/108 decoder-shape failures. Those failures
are not retrospectively validation credit. Current version-2 results must have
their own executable/source evidence and independently replayed controls.

This increment does not fix reverse mixed-set construction in the default
profile or broaden Random/VIEW shape admission. It does not authenticate an
archive, certify reachability from bytes alone or solve arbitrary symbolic/lazy
transport. The separate trusted-storage tuple-function bit-loss study records
an existing frontier/checkpoint decoder limitation tracked separately as
[#195](https://github.com/Chelis-Lang/dive/issues/195); version 2 does not change
those formats. See [ADR 0055](decisions/0055-stored-trace-transport.md) and
[#190](https://github.com/Chelis-Lang/dive/issues/190).

Decimal large integers are limited to 4,096 digits before parsing. A hand-built
`BigInt` fitting i64 is refused rather than silently converted to another stored
variant; ordinary canonical large integers remain supported in the big-integer
profile. This does not change legacy integer transport or model arithmetic.
