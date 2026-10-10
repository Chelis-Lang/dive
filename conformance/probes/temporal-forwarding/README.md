# Retained temporal argument controls

All 14 source/config pairs are SANY-valid. `capture.json` records the complete
clean pinned oracle capture, hashes, reason and jar identity. These are raw
probes, not edited golden files. TLC runs with `-workers 1 -lncheck final -fp 0
-seed 0 -tool`; `-dumpTrace` is absent. The raw commands use symbolic cache and
temporary-directory paths. Reproduce through the pinned harness:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs conformance/probes/temporal-forwarding/specs \
  --out conformance/probes/temporal-forwarding --timeout 30 \
  TfIdentityHolds TfIdentityFails TfNestedHolds TfNestedFails \
  TfBooleanHolds TfBooleanFails TfInstanceBodyHolds TfInstanceBodyFails \
  TfEnabledLiveHolds TfEnabledLiveFails TfInstanceHolds TfInstanceFails \
  TfEnabledHolds TfEnabledFails
python3 scripts/check-temporal-forwarding.py
```

| Pair | Pinned TLC outcome | Native scope evidence |
|---|---|---|
| Identity | hold 0 / violation 13 | Lazy temporal expression argument |
| Nested | hold 0 / violation 13 | Captured LET value and local helper |
| Boolean | hold 0 / violation 13 | Two temporal arguments, conjunction |
| InstanceBody | hold 0 / violation 13 | Nested substituted body forwarded from caller |
| EnabledLive | hold 0 / violation 13 | Scoped temporal argument containing independent ENABLED |
| Instance | both 2213 / exit 255 | Exported INSTANCE temporal formal remains refused |
| Enabled | success 0 / initial invariant failure 12 | State-only safety classification; no temporal match credit |

All ten executing temporal models generate 3 states, retain 2 distinct states
and have complete depth 2. `TfEnabledFails` has no reference state summary;
native safety classification checks no printed-stat claim for that initial
failure. Neither exported INSTANCE refusal is counted as agreement.

The temporal native matrix has 120 final/short truth/count checks (ten models,
two profiles, two stores, requested widths 1/4/8) and 24 exported INSTANCE
refusal controls. Capture uses one worker; no scaling claim. Every returned
violation is independently validated against a freshly bound model and has
three negative controls: bad Init, unknown property and bad closing index.
The state-only pair has four separate safety classification checks.

The independent word matrix covers all 98 Boolean word/closing-edge choices
of lengths 1–4, ten formulas and both profiles: 1,960 property evaluations.
Truth follows direct cycle algebra, compared separately with both periodic
interpretation and planner/formula evaluation. ENABLED and WF exercise the
independent bounded cover path. Impure/live/opaque Rust operator captures and
tiny expansion budgets are negative controls; none produces output.

Recursive local function pairs in `../finite-deferred-audit/` still refuse.
The immutable historical 53-row and ten-row studies remain unchanged. This
suite establishes a narrow language increment, not strict TLC temporal error,
lasso-selection, phase-L or full-scoping certification.


Unchecked Rust expression references and retained lazy expression IDs now
produce typed refusals at the planner/periodic endpoints, including missing
scoped definitions. Missing references have no invented source span. The raw
native `native-unchecked-context-before.*` repro in the probe directory records
the prior panics; it is not an oracle result. Constructing an evaluator with an
invalid redirect target can still panic before these endpoints run (#129).
General caller-created IR/context validation remains a separate partial API
obligation; this increment does not redesign the evaluator constructor.
