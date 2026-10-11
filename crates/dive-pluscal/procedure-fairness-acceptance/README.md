# Process-owned procedure fairness evidence

This is a bounded native translation increment for issue #171 and ADR 0046.
It does not certify the full PlusCal translator or liveness execution. Production
translation uses Rust only. The Java normalization adapter is development-only;
clean `pcal.trans` output and separate SANY runs are the primary evidence.

The reference is commit `94d0c5070852b37b801bb8998c436383ab7ee37b`, jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
Every selected input, option, reference stream and generated artifact is retained
with hashes in its `selection.json` and `primary/manifest.json`.

## Complete retained selection

| Campaign | Capture rows | Admitted token/CST/SANY artifacts | Matching rejections | Named native refusals | Invalid-reference diagnostics | Oracle errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Original | 328 | 155 | 102 | 66 | 4 | 1 |
| Corrected modifiers | 146 | 141 | 0 | 0 | 0 | 5 |
| Original timeout follow-up | 1 | 1 | 0 | 0 | 0 | 0 |
| Corrected timeout follow-ups | 5 | 5 | 0 | 0 | 0 | 0 |

There are **480 capture rows and 474 unique source IDs**. The six follow-up rows
repeat the same source/options with larger reference budgets. The 302 admitted
artifact rows include those six repeats. Original failures are never overwritten.
The first selection mistakenly spells modifiers `F+:`/`L-:`; the pinned grammar
uses `F: +`/`L: -`. Both selections and the corresponding raw rejections remain.
Initial five-second timeouts retain their `oracle-error` disposition; separate
30-second captures demonstrate successful reference completion. All six original
SANY-invalid primary outputs remain uncredited, including those classified as
native refusals rather than invalid-reference diagnostics. Twelve public CLI and
twelve direct API controls reject those outputs in both profiles.

`predecessor-comparison.json`, `initial-native-comparison.json`, the exact
`initial-comparator.py`, `predecessor-source/` and the original failed component
gate preserve the before/failure evidence. `previous-*-selection-current-comparison.json`
replays earlier complete selections without changing their original campaigns.

## Validation stages

All **302 admitted artifact rows** independently pass SANY and exact generated
operator CST comparison, including junction grouping and prime scope. Tokens
alone are not acceptance evidence. Artifact documents always report
`model_proved=false` and `exhaustive=false`.

Curated finite controls use `INIT Init` and `NEXT Next`, separately from the
fairness Spec: **118 model rows** match pinned clean TLC verdict/count/depth
observations across **708 controls** (1/4/8 workers × fingerprint/exact stores).
The strict campaign preserves **59 complete matches and 59 invariant witnesses
pending #124**. Its four reports exit 1 and retain their disagreement rows;
namespace-provenance refusals are never counted as validated TLC witnesses.
Separate default/canonical replay validates 59 witnesses and rejects 236 corrupted
traces. Those reports explicitly set `strict_tlc_credit=false`; they do not repair
or certify the strict campaign.

The comparator rejects 39 complete-document mutations and 60 mutations attached
to genuine timeout rows. Missing proof metadata, forged exhaustive/proved status,
internal errors, bad exits, and corrupt tokens/actions/source maps cannot hide
behind reference errors. Source/runtime quotas also have native unit controls,
including shared fairness-work exhaustion across callers before retention.

## Reproduction

Use a private verified cache and the binaries built from the same source. Set
`JAVA` and `JAR` to the pinned JRE and jar, `NATIVE` to `dive`, `EXAMPLE` to the
`translate` development example, and `HARNESS` to a frozen conformance executable.
The reference capture scripts record their own command and source hashes.
Run from the repository root:

```sh
study=crates/dive-pluscal/procedure-fairness-acceptance
for campaign in "$study" "$study/corrected-modifiers" "$study/timeout-followup" "$study/corrected-timeout-followup"; do
  python3 -I "$campaign/compare.py" --manifest "$campaign/primary/manifest.json" --inputs "$campaign/cases" --native "$EXAMPLE" --output "$campaign/comparison.json"
  python3 -I "$campaign/validate.py" --java "$JAVA" --jar "$JAR" --native "$NATIVE" --example "$EXAMPLE" --report "$campaign/comparison.json" --inputs "$campaign/cases" --fixtures "$campaign/primary" --output "$campaign/validation"
  # Strict validation currently exits 1 for retained #124 witness refusals.
  python3 -I "$campaign/qualified-replay.py" --native "$NATIVE" --output "$campaign/qualified-replay"
done
python3 -I "$study/test-comparison.py" --native "$EXAMPLE"
python3 -I "$study/test-timeout-comparison.py" --native "$EXAMPLE"
python3 -I "$study/verify.py" --native "$NATIVE" --example "$EXAMPLE" --harness "$HARNESS"
```

`delivery.json` binds complete evidence, production/development executable hashes,
source and all gate logs. The frozen ancestry full gate passes **691 Rust tests**,
Python fixtures, clippy, doctests, rustdoc, standalone trace layering/build and
cargo-deny. Its CM1 list has **20 models**, passing at 1/4/8 workers; ValidationTestSuite
smoke has **16**, passing at those widths. Parent integration reruns the current
main list, whose additional model is not silently attributed to this ancestry.
Four CLI and two compiled Rust examples from
[the user guide](../../../docs/pluscal-procedure-fairness.md) pass independently.

Unproved scoped renaming, sequential procedure modifiers, macro/procedure
interactions, nonliteral/overlapping process domains, multiline fairness-domain
layout, and historical pretty-print/checksum/map contracts remain named limits.
The parity families remain partial/audit-required.
