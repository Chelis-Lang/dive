# Recursive enumeration diagnostics

Generated against the pinned `tlc-2026.10.06.014338` oracle with:

```sh
cargo run --release -p dive-conformance -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-value/tests/fixtures/printing/specs \
  --out crates/dive-value/tests/fixtures/printing/tlc \
  --timeout 10 EmptySymbolicFunction NonenumerableFunction
```

The empty symbolic domain prints `{<<>>}` and succeeds in TLC. Dive now
requires this same output. A 2 MiB regression deterministically aborts with
the old printer; the expansion guard retains named errors for the genuinely
non-enumerable case. The ordinary adversarial generator and its limits remain
unchanged.

## Empty-domain/range coverage

Five additional clean probes (`EmptySymbolicSize`, `EmptySymbolicEnumerate`,
`EmptyDomainInfiniteRange`, `SingletonInfiniteDomain`, `EmptySymbolicQuantifier`) all succeed with two
generated states, one distinct state and depth one. They pin cardinality,
finite-set predicates and equality to finite representations. Cardinality one
does not imply enumerability of an infinite-domain function.

Regenerate with the command above, passing these five model names. The empty
symbolic diagnostic regression now requires the oracle's `{<<>>}` output; the
nonempty symbolic regression still requires a named error on a small stack.

Evaluator preflight short circuits structural operands only. Deferred predicate
operands retain their existing guarded behavior so preflight never introduces
an extra spec evaluation. Empty deferred-function coverage remains open.
