# Strict replay of tuple-only sequence calls

Independent trace replay in TLC mode accepts `Tail`, concatenation (`\o`) and
`SubSeq` when an unevaluated proof establishes tuple operands. This is a
validator admission improvement: it changes neither model evaluation nor the
checker's conservative sequential fallback for runtime-name-capable operators.

For example, save this module as `TupleReplay.tla`:

```tla
---- MODULE TupleReplay ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = IF Len(Tail(<<0,1>>)) = 1 THEN 1 ELSE 2
Inv == x = 0
====
```

Use `INIT Init`, `NEXT Next` and `INVARIANT Inv` in `TupleReplay.cfg`:

```sh
dive check TupleReplay.tla --compat=tlc --seed 0 --fp 0 --output json > result.json
# The expected invariant violation exits 12; keep its report.
dive validate-trace TupleReplay.tla --compat=tlc --seed 0 --fp 0 --trace result.json --output json
```

The pinned oracle reports `Inv` violated after two generated/distinct states,
at depth two. The actual word is `x=0 -> x=1`. Independent replay must accept
that word and reject `x=0 -> x=2`. A compiled Rust example exercises the same
original and altered word:

```sh
cargo run --locked -p dive-api --example tuple_only_replay
```

## What the proof admits

Tuple literals, tuple-only branches of `IF`, closed tuple constants,
zero-arity tuple definitions and resolved lazy/local scopes can establish an
operand shape. User wrappers such as `Drop(v) == Tail(v)` retain the actual
argument scope. Configured native aliases receive the same proof as direct
calls. Neither the operator's spelling nor a tuple seen in a supplied trace
establishes its operand shape.

Every admitted operand is also inspected for nested name construction. For
example, `Tail(<<0, ToString(x)>>)` retains the runtime-string-order refusal,
even though its outer operand is a tuple. A string operand or a mixed
tuple/string conditional is likewise refused in TLC mode. The canonical
profile's existing validation behavior is unchanged.

The guard inspects all configured checking roles and assumptions, resolving
bound definitions, constants and substitution scopes without evaluating
predicates, normalizing values, calling providers or constructing successors.
Opaque operator arguments, recursive calls, unknown bindings and symbolic
stored constants remain conservative refusals. Admission has an 8,192-step
inspection budget and depth 64; each tuple-operand proof has its existing
256-step budget. These are inspection policies, not total validation/search
memory or CPU limits. Exhaustion never supplies a guessed proof.

Other native calls must be in the audit's closed list of arithmetic, finite-set
size tests and nonconstructing sequence operations. Pure overrides are not
automatically safe: `ToJson` returns a newly constructed string. JSON conversion,
host access, checker introspection, randomness and unclassified native calls
retain the refusal when this tuple-only admission is required. Their existing
behavior in models that do not require this guard is unchanged.

## Remaining limits

Saved procedure stacks require a separate source-wide inductive shape proof.
The [bounded variable-history continuation](inductive-tuple-trace-replay.md)
describes the new supported proof rules and remaining refusals. Inspecting only
stack values in a proposed witness never establishes its history. The original
literal-operand increment does not establish general runtime-name provenance.

The namespace-forgery controls from issue #124 remain mandatory: reading a
counterexample must never establish the string encounter order that proves it.
Lossy TLC documents and directly decoded native words use the same admission
guard before predicate interpretation. Structural decoding is still separate
from checking Init, Next and the ending claim.

See [ADR 0064](decisions/0064-tuple-only-independent-replay.md),
[the runtime-name controls](../crates/dive-trace/tests/runtime_names.rs) and
[the known namespace limitation](tlc-differences.md#current-limitation-independent-runtime-name-trace-validation-124).

## Validation

The implementation was checked with 1,212 passing workspace tests and no skips.
Eight operand forms ran both profiles and stores at workers 1/4/8, with and
without scheduling chaos. All 192 actual counterexamples replayed as Valid;
their 192 altered counterparts replayed as Invalid. The unchanged 21 CM1 and
16 ValidationTestSuite gate models agreed at all three widths. This evidence
covers the bounded operand proof above; it does not certify all sequence or
runtime-name behavior. Original producer identities and an earlier disk-full
gate failure are retained in the [case archive](../conformance/probes/tuple-only-replay/README.md).
