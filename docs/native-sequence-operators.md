# Append as a temporal operator argument

Explicit bounded final TLC checking accepts `Append` as a two-argument operator
actual, alongside unary `Len`, `Head` and `Cardinality`. Constant and state
expression operands retain their original scopes through direct calls, `LET`
relays and named `INSTANCE` exports. Native authority comes from admitted
`Def.std` or an explicit `StdOp`, not an operator name.

For example:

```tla
---- MODULE SequenceAppendLetFails ----
EXTENDS Sequences
VARIABLE x
Init == x = <<TRUE>>
Next == UNCHANGED x
Use(F(_,_)) == []<>(~(F(x, TRUE) = <<TRUE, TRUE>>))
Live == LET Pass(G(_,_)) == Use(G) IN Pass(Append)
====
```

With `INIT Init`, `NEXT Next`, and `PROPERTY Live` in the configuration:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/native-sequence-operators/models/SequenceAppendLetFails/SequenceAppendLetFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --workers=4 --seed=0 --fp=0 --output=json
```

Exit 13 means an independently validated stuttering liveness violation. Schema 2
retains actual counters, worker facts, original/configured/derived property names,
final-only schedule and independent replay. Removing the negation yields bounded
completed success. Unsupported, resource-limit and incomplete outcomes cannot
prove success.

The [complete runnable Rust example](../conformance/probes/native-sequence-operators/user-example.rs)
uses `dive_api::tlc_temporal::check_final` with
`dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) }`.
It loads the real model/configuration and asserts a private validated violation.
See the [final API guide](tlc-final-api.md) for exact options and proof limits.
The independent word evaluator supports both profiles; this explicit final TLC
facade still refuses the default profile, exact temporal storage and ordinary
periodic scheduling.

Exactly two ordinary constant/state expression operands are admitted. The
bounded scope/purity proof checks both, even when one is irrelevant to the result;
a temporal, primed/action, higher-order, configured, redirected, overridden,
opaque or effectful operand/target refuses. Wrong arity is rejected by the front
end or typed public admission. Provider/deferred values remain refused even if
cached. Before native evaluation, stored values must match the bound namespace
and run order; tuple domains retain their ascending-integer exemption. Existing
50,000-visit/128-depth native input and scope work limits stay unchanged, separate
from Session's 8m cumulative successor visits/depth 256 and final graph bounds.
None is an aggregate byte or evaluation-time guarantee.

`Tail` and `SubSeq` remain refused in this facade. Although the pinned primary
executes the tuple cases, both native implementations also accept strings and
can intern substrings, changing TLC encounter ordering. The current pre-bind
guard deliberately retains `creates_runtime_names`: a tuple example is not a
proof about all startup or supplied arguments. Their stored-tuple type admission
and the runtime-name witness obligation are pending separately. Type-error
primary cases also remain outside deciding runtime-error parity.

The [complete controls and preserved failures](../conformance/probes/native-sequence-operators/README.md)
include clean primary, actual Java field projections, old native refusals,
independent words and corrupted witnesses. They establish the documented bounded
Append slice, not full corpus or temporal family certification.
