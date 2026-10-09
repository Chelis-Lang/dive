# Constant-domain temporal quantifier probes

These owned minimal specifications test finite quantifiers, configured constants
and tuple bindings in the final temporal API. Oracle transcripts were generated
on 2026-10-09 with the pinned `tlc-2026.10.06.014338` oracle, using final checking,
one worker, fingerprint 0 and seed 0. The deciding pass has no `-dumpTrace`;
trace capture is a separate cross-checked pass. Transcript files are generated,
never edited. The reason for this capture is the finite temporal-domain increment.

```sh
DIVE_CACHE_DIR=<verified-cache> dive-conformance dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-quantifiers/specs \
  --out crates/dive-api/tests/fixtures/temporal-quantifiers \
  QtVisit QtFairVisit QtStable QtEmpty QtTuple
```

| Probe | TLC verdict / exit | Generated / distinct / depth | Purpose |
|---|---|---|---|
| QtVisit | temporal violation / 13 | 5 / 2 / 2 | All constant-domain values must recur; an unfair stutter violates P |
| QtFairVisit | success / 0 | 5 / 2 / 2 | Weak fairness forces changes between two values, satisfying universal recurrence |
| QtStable | temporal violation / 13 | 5 / 2 / 2 | Existential eventual stability fails on the fair alternating cycle |
| QtEmpty | temporal violation / 12 | 2 / 1 / 1 | Empty existential domain is FALSE; TLC uses its finite-prefix failure path |
| QtTuple | success / 0 | 5 / 2 / 2 | Tuple destructuring and a pure configured definition as the domain |

`QtEmpty` exposes harness issue #77: the current oracle parser labels exit 12
with message 2116 as an oracle error. The API tests compare temporal truth and
exploration counts, not exact TLC class or trace shape. Full TLC compatibility
requires the finite-prefix path, not merely a valid periodic witness.
