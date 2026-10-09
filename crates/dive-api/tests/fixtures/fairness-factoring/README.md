# Five-process fairness without a product explosion

Raw clean pinned TLC runs use one worker, seed/fingerprint 0 and `-lncheck final`.
Trace capture is a separate cross-checked pass. Reproduce:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/fairness-factoring/specs \
  --out crates/dive-api/tests/fixtures/fairness-factoring \
  --timeout 30 FwFive FwMissing FwMixed
```

All models generate 161 states, retain 32 and reach depth 6:

| Model | Fairness | TLC result |
|---|---|---|
| FwFive | All five processes weakly fair | Success, exit 0 |
| FwMissing | Only processes 1–4 weakly fair | P violated, exit 13 |
| FwMixed | Two strongly fair, three weakly fair | Success, exit 0 |

Both dive profiles/stores match final truth/counts at requested widths 1/4/8;
actual capture remains inline at width 1. Violations validate through a fresh
binding and lossless document. Replacing the closing edge with forever-stuttering
fails independent fairness validation. The weak-fairness cases fit a 64-node
tableau bound that their unfactored full-language tableaux exceed.

Pure recurring conditions constrain accepting cycles, not finite prefixes.
Nonboolean/nested-temporal operands remain in the general tableau. Formula,
condition-expansion, product-entry and acceptance-cell limits refuse explicitly.
This does not establish TLC-exact temporal classes, selection, phase-L timing,
parallel graph capture, VIEW or symmetry support.
