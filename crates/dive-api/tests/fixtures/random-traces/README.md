# Random-stream placement regressions

`RndWide` produces 5001 candidates per expanded state; the violating
successor comes after more than 4096 generator steps. Independent validation
cannot infer that offset by checking a bounded prefix of the stream.
`RndCoincidence` draws from a singleton, so a predicate can match at any
offset without establishing which draw the generator used.

The pinned TLC capture (`94d0c50`, seed 7, fingerprint polynomial 0, one
worker) reports the `RndWide` violation, then times out after 300 seconds
while reconstructing the trace. It is not a usable golden counterexample.
`RndCoincidence` exits 12 and supplies the captured two-state trace. Both
SANY runs accept. Complete clean commands and outputs are retained.

```sh
dive-conformance dev capture --oracle tlc-2026.10.06.014338 --seed 7 \
  --specs crates/dive-api/tests/fixtures/random-traces \
  --out crates/dive-api/tests/fixtures/random-traces RndWide RndCoincidence
cargo test -p dive-api --test trace_random
```

These cases must return unsupported when their draw position is unproved,
even when a predicate matches by coincidence. They complement the positive
controls for straight-line assignments and unprimed guards, with seed and
polynomial mutations that must be rejected.

`native-before-stream/` retains complete supplied trace inputs from the frozen
admission-era native producer `475ef10` (source and executable digests in its
index). They are not TLC goldens or independent certificates. The checker now
refuses the unproved initial-predecessor printing contract; the independent
validator must still refuse these supplied originals for their unproved draw
positions. The test separately checks the named checker refusal and preserves
the original validator assertions. The pinned TLC captures above remain unchanged.
