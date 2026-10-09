# Recursive enumeration diagnostics

Generated against the pinned `tlc-2026.10.06.014338` oracle with:

```sh
cargo run --release -p dive-conformance -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-value/tests/fixtures/printing/specs \
  --out crates/dive-value/tests/fixtures/printing/tlc \
  --timeout 10 EmptySymbolicFunction NonenumerableFunction
```

The empty symbolic domain prints `{<<>>}` and succeeds in TLC. Dive's current
coverage refusal stays visible; this package fixes the recursive error printer,
not that enumeration gap. A 2 MiB regression deterministically aborts with the
old printer and completes with symbolic diagnostics with the expansion guard.
The ordinary adversarial generator and its limits remain unchanged.
