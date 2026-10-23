# Conformance snapshot

This compact snapshot is mechanically derived from the original small-tier reports below. It replaces the oversized historical generated summary at this path; the previous summaries remain in Git history. Full generated summaries and original reports are preserved outside the repository. Per-model verdicts, exits, counts, depth and property names remain in the generated [TLC baseline](../conformance/baselines/small-tlc-w1.json) and [default baseline](../conformance/baselines/small-dive-w1.json).

The reports were produced by fullgate source `db8d4ad56143d33b07a8c885a03331b3c44ab30b` on main `1892ddc7ea97902f65d406543a9d68f38a498e9d`, pinned TLC `tlc-2026.10.06.014338`. The full gate passed all 13 phases and 2,295 tests with zero skipped. Both separately required affected-model profiles and all sixteen normal instruction arms also completed. The [complete capacity qualification](survey/data/product-vertices-qualification-2026-10-23.json) records their independent audits, frozen binaries and resource cleanup.

[Scoreboard](scoreboard.md) distinguishes current one-worker parity from historical four-worker and full-tier measurements. [Temporal checking](user/temporal.md#product-vertex-capacity) describes the supported product policy and remaining bounds. Expected divergence does not count as agreement.

## Model and module outcomes

| Profile | Workers | Models selected | Agree | Unsupported | Expected divergence | Disagree / timeout / pending / oracle error | SANY selected / agree |
|---|---:|---:|---:|---:|---:|---:|---:|
| TLC | 1 | 156 | 150 | 6 | 0 | 0 | 458 / 458 |
| default | 1 | 156 | 144 | 5 | 7 | 0 | 0 / 0 |

SANY runs once in the TLC-profile gate; the default-profile report contains no repeated SANY runs. All 156 model IDs per profile remain selected. CoffeeCan1000Beans is the sole changed baseline model: success in both profiles, 501,500 distinct / 2,000,002 generated / depth 1. Every other typed baseline observable is unchanged.

## Report identity

| Profile | Original run ID | Report SHA-256 | Generated baseline SHA-256 |
|---|---|---|---|
| tlc | `20261023T013135052-dive-small` | `37914875d4e7f9af628725fa04c3333e61cfa839ebb332f08b7726191646a7ab` | `2d7289f3320827cef61c009618e16ae1b7e9efb8e9d82ba447a1f24d0e94b650` |
| dive | `20261023T014009568-dive-small` | `4295f6b6d52b5f6cd181193ca7320a9c58fc57578779c665d762be4914698a0b` | `0d6842422b798618af5be64533877ad9cf8d8ebca4a24021f07acf9d41ed8f2e` |

Both profiles use the same actual native CLI SHA-256 `2fa544a9516d17e9679b4940af8443a5b3aa77f556de60294d6fb4ce3cb5d482`. The golden oracle jar SHA-256 is `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`. These are original producer identities; they do not imply a new run of a later delivery head.

## Selection and settings

| Corpus | TLC models / agree | Default models / agree | TLC SANY modules / agree |
|---|---:|---:|---:|
| `configured-temporal-operators` | 2 / 2 | 2 / 2 | 2 / 2 |
| `examples` | 140 / 135 | 140 / 131 | 424 / 424 |
| `probes` | 14 / 13 | 14 / 11 | 12 / 12 |
| `validation-smoke` | 0 / 0 | 0 / 0 | 20 / 20 |

The complete small tier uses one worker, two concurrent harness jobs, no model filters and full SANY selection. Native model budgets are TLC-derived and capped at 900 seconds; the original gate has independent process/campaign backstops. TLC strict-tier and default-profile settings retain their original meanings. There are no oracle or informational drift findings and no worker verdict/count/depth/property variants.

## Reproduction and evidence policy

Use `dive-conformance report summarise <report.json>... --summary <outside-repository-path>` to regenerate the full per-model and per-module Markdown from preserved originals without rerunning a model. The harness still defaults to writing `docs/conformance.md` unless `--no-summary` or `--summary` is selected; those default-generated rows are not the maintained compact snapshot and must not be committed over it. Generate baseline JSON with `report baseline <report.json> --out <path>`. Oracle goldens are unchanged.

This document omits per-model detail prose, raw outputs, hosts and duplicate module rows to stay below the 100 KB evidence-item limit. It changes presentation only: generated baselines and the source-pinned qualification record retain the comparison facts. Full summaries remain regenerable, and historical summaries remain available in Git history.

No new four-worker, full-tier, throughput, scaling or true peak-memory result is inferred. A later-source delivery requires explicit source-transfer evidence; the separate affected-model and normal-input observations are documented in the linked qualification record.
