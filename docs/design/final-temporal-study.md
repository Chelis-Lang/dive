# Final/short temporal observations

The bounded final backend has its own black-box harness entry:

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
scripts/final-temporal-study.sh conformance/studies/lm1.txt \
  --impl-profile tlc --workers 1,4,8 --timeout 600 \
  --out conformance/reports/studies/lm1-final-short.json
```

Fetch the pinned bundle and Examples corpus first, as described in the harness
README. Use an isolated `DIVE_CACHE_DIR` when working concurrently. `--dive-bin`
and `HARNESS` select builds outside the default target directory.

This command runs `dive check --lncheck final --lasso short` with seed and
fingerprint polynomial 0. A TLC value profile reports `tlc+overrides`.
Every selected model/worker pair appears, including named refusals, missing or
stale goldens, timeouts and corpus exclusions. Requested workers remain separate
from actual workers; graph capture currently runs inline.

A `matches-truth-and-counts` observation requires the expected final verdict and
all state statistics to equal the pinned single-worker final golden. Periodic
violations also need a fresh model binding and independent lossless-trace
validation. The implementation's own `validation` field supplies no evidence.
Contradictory mode flags, displayed states, claim names, exits and unknown verdicts
are findings. Named resource/feature refusals remain `unsupported`; they are not
matches. An unconfirmed witness remains `validation-pending`.

Reports use their own `final-temporal-study` schema and explicit claim. They do
not enter strict `agree` totals, replace `docs/conformance.md`, excuse a strict
mismatch, or change the required conformance gate. They establish no TLC-exact
lasso selection, temporal error class, phase-L stopping, periodic schedule or
parallel scaling. Final versus finite-prefix stopping can produce count findings;
keep these in the report rather than relaxing equality.

`conformance/studies/lm1.txt` is the fifteen-model selection from the M6 plan.
Its observations guide the next planning/validation increments. Full LM1, H1 and
TLC compatibility remain open until their complete requirements are proven.

The latest [LM1 TLC-value study](../../conformance/reports/studies/lm1-final-short-tlc.json)
retains all fifteen models at requested widths 1, 4 and 8: thirty-nine observations
match final verdict truth/counts and six are named refusals, with no findings.
The build includes bounded state-dependent predicate domains, finite CHOOSE
and pure action-helper arguments. FourQueens now
matches the complete safety stop (62 generated, 40 distinct, depth 5), and
ChangRoberts matches its final success (227 generated, 137 distinct, depth 10).
DistributedReplicatedLog now gets past FiniteSetsExt Min/Max and pure
action-helper arguments. It and DiningPhilosophers reach the tableau-node
bound; neither limit is interpreted as a successful check. Every deciding field is
identical across requested widths; successful graph capture reports one actual
worker. Full LM1, TLC-exact temporal behavior and parallel capture remain open.
