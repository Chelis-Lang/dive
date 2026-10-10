# Bounded finite lazy-domain follow-up, 2026-10-10

The [generated report](../conformance/reports/studies/lazy-domains-10-final-short-tlc.json)
retains every model in the [ten-model selection](../conformance/studies/lazy-domains-10.txt).
These were the exact ten entries refused for finite lazy-domain bindings in the
[original 53-model study](../conformance/reports/studies/small-temporal-53-final-short-tlc.json).
The original report is unchanged.

| Observation | Models |
|---|---:|
| Final verdict truth and counts match | 0 |
| Named unsupported feature | 7 |
| Named resource bound | 2 |
| Harness timeout, no verdict | 1 |
| Disagreement, malformed report or oracle error | 0 |

The harness groups named resource limits under its `unsupported` total, so the
raw totals are nine unsupported and one timeout. The study exits nonzero because
of the timeout. The implementation is ready for the controlled finite domains,
but **none of these ten full models gains a final matching verdict**.

| Model | Remaining blocker |
|---|---|
| `MCQuicksort` | Deferred or infinite predicate binder domain |
| `ElevatorLivenessMedium` | Effectful, temporal or ENABLED operand in a pure predicate atom |
| `MCYoYoPruning` | Deferred or infinite predicate binder domain |
| `YoYoAllGraphs` | Deferred or infinite predicate binder domain |
| `AllocatorImplementation` | Opaque scoped level |
| `AllocatorRefinement` | Opaque scoped level |
| `SchedulingAllocator` | Opaque scoped level |
| `SimpleAllocator` | Temporal product acceptance-cell resource bound |
| `bcastByz` | 600-second timeout, no verdict |
| `EWD998ChanID` | Temporal formula predicate-binding resource bound |

Elevator and four allocator models pass their earlier finite-domain rejection
and reach later semantic or resource limits. EWD998 also reaches a formula
binding limit. Quicksort and the two YoYo models still require deferred-domain
support. The timeout does not prove that bcastByz passed every old rejection.
These are follow-up capabilities, not a reason to raise resource defaults or
weaken the required gate.

The binder now admits bounded function sets, record sets, Cartesian products
and subsets without increasing any resource defaults. It preflights component
cardinalities, nested values and generated value slots before materialization,
then keeps the evaluator's original scope and profile-specific ordering. Raw
set lengths are conservative upper bounds; heavily duplicated operands can
still exceed the admission budget. Infinite and deferred binder domains remain
named refusals. This does not establish full finite-domain or temporal parity.

The eight minimal paired hold/violation fixtures independently captured from
TLC are under `crates/dive-api/tests/fixtures/lazy-domains/`. Their end-to-end
controls cover both value profiles, both stores and requested worker counts
1, 4 and 8: 96 checks. Every violation is independently validated; fresh
validator models accept the alternating Boolean word and reject its stuttering
mutant. The complete workspace gate passed 592 Rust tests with no skips, plus
12 Python controls. The current-main integration subsequently passed 601
Rust tests and the same Python controls; the historical study artifact and
source/binary pins below remain unchanged.

## Evidence limits

This runs explicit `--lncheck final --lasso short` in TLC value mode and reports
`tlc+overrides`. It compares final verdict truth and complete counts only. It
proves neither exact TLC temporal error classes, phase-L stopping, property-set
selection nor exact lasso choice; it does not increase strict conformance totals
or become a passing PR gate. Every refusal and timeout remains in the report.

The source predates the #109 failure-metadata fix. Failed documents' zero-worker
and "no search ran" fallbacks cannot establish that capture did not occur;
this study has no successful result document from which to credit completed
capture or a used-worker count. Failed-stage metadata is not credited.
Other checks overlapped this macOS run. Per-row timings are observations under
load, not performance baselines.

## Pins and reproduction

Source: `2b305eea34e5726d16cfa680bcd1ee2691852afc`.
Binary SHA-256:
`e9e880ce5326c2688df9e8071103f0b8547135679ef221d5b9252777c924a486`.
The binary hash was verified unchanged after the study.
Report SHA-256: `e46d127a576674f65823db6b4c82ca99bcb7cd953be3d9e2d9a26f93f405a29a`.

The report records oracle `tlc-2026.10.06.014338` and jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`,
Examples commit `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, module-library
pins, seed 0, fingerprint polynomial 0, all commands and host details. Each
model retains its original configuration and fixed 600-second timeout.
Goldens were not edited.

```sh
git worktree add ../dive-lazy-study 2b305eea34e5726d16cfa680bcd1ee2691852afc
cd ../dive-lazy-study
CARGO_BUILD_JOBS=2 cargo build --release --locked -p dive-cli -p dive-conformance
HARNESS=target/release/dive-conformance scripts/final-temporal-study.sh \
  conformance/studies/lazy-domains-10.txt --impl-profile tlc --workers 1 \
  --timeout 600 --dive-bin "$PWD/target/release/dive" \
  --out conformance/reports/studies/lazy-domains-10-final-short-tlc.json
```

Set `DIVE_CACHE_DIR` to an independently copied, verified pinned cache as
described in the harness README. The executable path must resolve after the
harness changes the child's working directory to the model directory.
