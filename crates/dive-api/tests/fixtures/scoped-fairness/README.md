# Scoped fairness and enabledness

Pinned raw TLC/SANY transcripts from `tlc-2026.10.06.014338`, one worker,
`-lncheck final -seed 0 -fp 0`; clean runs decide verdicts and counts, and
violation trace JSON comes from a cross-checked second pass.

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/scoped-fairness/specs \
  --out crates/dive-api/tests/fixtures/scoped-fairness \
  SfDisabled SfEnabled SfReach
```

`SfDisabled` moves only from 0 to 1 and permits explicit stuttering. Its fair
counterexample stays at 1 forever, where `Go(1)` is disabled. No recurring
occurrence can certify fairness: independent absence of a non-stuttering
successor must do so. TLC reports P violated (exit 13). The other probes
succeed: recurrent enabledness under two weak-fair actions, and eventual
arrival at 1 under weak fairness.

Tests compare final truth/counts in both profiles/stores and independently
validate the periodic claims. Strict TLC lasso selection, phase-L timing and
broader lazy/higher-order argument scopes remain outside this evidence.
