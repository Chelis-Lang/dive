# Expanded final/short temporal study, 2026-10-10

The [raw generated study](../conformance/reports/studies/small-temporal-53-final-short-tlc.json)
records all 53 configurations in the explicit selection.
The selection comes from temporal-refused Examples entries in the committed
`20261009T134501007-dive-small.json` baseline. Every selected model has one row,
including refusals and timeouts; no entry is dropped.

| Observation | Models |
|---|---:|
| Final verdict truth and counts match | 26 |
| Named unsupported feature or resource bound | 22 |
| Harness timeout, no verdict | 5 |
| Disagreement, malformed report, oracle error or pending validation | 0 |

The matches comprise 24 successes, one safety violation and one liveness
violation. The harness independently checks counterexamples. All 26 matching checks
report one actual worker. The 22 refusal documents report zero used workers,
but that fallback is unreliable for post-capture failures: the acceptance-cell
refusal follows a completed capture even though its document says "no search
ran". [Issue #109](https://github.com/Chelis-Lang/dive/issues/109) tracks that
metadata defect. The raw document is retained unchanged; a reported zero does
not prove no work occurred. Five timeouts have no result document. Requested
width is one.

New final/short reports retain execution evidence on failures (#109). Schema 2
adds an `execution` object with `stage` and `capture_completed`; a failed
post-capture product or atom evaluation retains capture statistics, output,
warnings and collision qualifications while remaining a refusal/resource
failure. These are dive exploration facts (`reported_by_tlc: false`), not a
temporal verdict. Unknown execution after a thread failure uses `null` worker
usage and capture completion, rather than asserting zero. Before-search binding
and planning failures still prove zero capture workers. Ordinary schema-1
checks are unchanged. Rust callers can use
`dive_api::temporal::check_final_with_evidence` or
`dive_liveness::analyze::analyze_final_with_evidence`; the original error-returning
entry points remain available. This correction does not rewrite the historical
53-model artifact or improve its conformance totals.

This is explicit `--lncheck final --lasso short` with TLC values, reported as
`tlc+overrides`. It proves neither exact TLC temporal error classes, phase-L
stopping, property-set selection nor exact lassos. It does not increase strict
conformance totals or become a passing PR gate. The five timeouts make the
study exit nonzero, and remain in the evidence.

## Pins and reproduction

The binary SHA-256 is
`62bc8577aef390ded5134c0a487a6c40fd48d2e6b554ec9ec334feb351914e86`,
built from `57a3649382e6d0b6a4ed48e1b67f79f077e14a2e` (PR #103).
The oracle is `tlc-2026.10.06.014338`, jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`,
and Examples is pinned at `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`.
The artifact records module-library pins, commands, seed 0, fingerprint 0,
host details and per-row documents. The binary hash was checked unchanged at
the end. Goldens were not edited.

To reproduce that source rather than a later backend, create a clean worktree
at the recorded commit and copy the maintained selection into it:

```sh
git worktree add ../dive-final-study 57a3649382e6d0b6a4ed48e1b67f79f077e14a2e
cp conformance/studies/small-temporal-53.txt ../dive-final-study/conformance/studies/
cd ../dive-final-study
cargo build --release --locked -p dive-cli -p dive-conformance
HARNESS=target/release/dive-conformance scripts/final-temporal-study.sh \
  conformance/studies/small-temporal-53.txt --impl-profile tlc --workers 1 \
  --timeout 600 --dive-bin target/release/dive \
  --out conformance/reports/studies/small-temporal-53-final-short-tlc.json
```

Use a verified pinned cache as described in the harness README. The fixed
600-second timeout is part of the observation. Other builds and checks
overlapped the macOS run; wall times are not performance baselines.

## What the refusals identify

| Remaining capability | Models |
|---|---:|
| Bounded predicate bindings over finite lazy domains | 10 |
| Higher-order predicate arguments | 3 |
| Dynamically bound predicate operators | 2 |
| Random operators or dynamic temporal planning effects | 2 |
| Temporal product acceptance-cell resource bound | 1 |
| VIEW or SYMMETRY graph capture | 1 |
| Opaque substituted predicate atom | 1 |
| Hidden variable of an instantiated module in ENABLED | 1 |
| Recursive/overridden/higher-order temporal call | 1 |

The ten domain refusals include Quicksort, ElevatorLivenessMedium, both pruned
YoYo variants, four allocator configurations, bcastByz and EWD998ChanID.
These are explicit implementation limits; they are not invalid TLA+ specs.
The raw artifact retains the exact locations and explanations. Broadening
finite domain support must preserve bounded enumeration, scope, ordering and
independent counterexample validation. Resource defaults are unchanged.

## Timed out, without a model verdict

The following each reached the fixed 600-second limit:

- CoffeeCan1000Beans
- MCDisruptor_MPMC_liveliness
- MCFlashWithMutexLive
- SingleLaneBridge/MC
- ACP_NB_TLC

They need isolated profiling and bounded graph-capture scaling work. A timeout
cannot be counted as success, failure or unsupported, and these observations
do not establish a speed ratio to TLC.


Tracked next steps: [finite lazy-domain bindings (#107)](https://github.com/Chelis-Lang/dive/issues/107)
and [isolated capture profiling (#108)](https://github.com/Chelis-Lang/dive/issues/108).
Symmetry binding, warnings and canonical keys remain in #104; broader scoped
semantics and exact temporal integration remain in #63 and #66.


The preserved study above predates bounded ordered parallel capture (P4-capture,
issue #117). Current supported pure final/short runs can use the requested pool,
with fixed eight-source waves and a separate per-source scratch admission cap.
The historical model totals, timeouts and source artifacts remain unchanged.
Current graph, tiny-bound and validated witness controls are documented in
[the temporal API](temporal-api.md#bounded-parallel-graph-retention); exact TLC
lassos and phase-L timing are still not certified.
