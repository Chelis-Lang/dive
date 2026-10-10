# Recursive sequential procedure evidence

The 116 C/P rows were generated and captured before native source changes.
`fixtures/manifest.json` keeps the primary translator, normalized-source adapter,
SANY and raw output hashes. The adapter is development evidence, never runtime
translation. The frozen predecessor's `initial-comparison.json` accounts for
111 refusals and five matching rejections; it remains unchanged.

The native token campaign accounts for 86 admitted artifacts, 15 matching
rejections, nine named refusals and six diagnostic invalid-reference outcomes.
All ten SANY-invalid primary artifacts remain uncredited, including both
zero-argument self-tail outputs with the measured empty-conjunction defect.
Two invalid-scope kinds are native refusals; diagnostic disposition is not
artifact acceptance. Enclosing SANY and operator CST structure are separate
checks, retaining junction indentation, operator grouping and prime scope.
Historical 84/120 source selections remain intact; their new expanded comparison
reports admit four/ten newly proven recursive rows without replacing originals.

`validation/report.json` keeps 146 generated-model rows, checked against clean
single-worker TLC and native 1/4/8-worker fingerprint/exact searches (876 controls).
72 complete models agree. 72 invariant and two fallthrough-deadlock traces remain
strict replay disagreements under #124. They do not earn fully validated TLC
violation credit. `qualified-replay/report.json` accounts for those pending rows,
then separately checks 74 default/canonical witnesses and 296 corrupted traces.
Every switched-profile control has `strict_tlc_credit=false`. Both canonical
fallthrough deadlocks are decidable here; this does not relax or certify the
strict runtime-namespace guard. The initial inherited expectation that canonical
deadlocks would be unsupported is retained with its failed script/raw output.

`budgets/report.json` keeps 12 clean primary two-second timeout prefixes and
192 native graph-capture controls. 96 default controls reach resource bounds;
96 strict TLC controls retain the existing runtime-name capture refusal. All
report `exhaustive=false`, `model_proved=false`; no partial graph becomes a proof.
A TLC-mode increasing integer can eventually overflow, so the timeout prefix
alone does not prove an infinite TLC execution. No strict guard is removed to
manufacture a budget match. The initial guard-unaware budget expectation and
raw failure are retained under `initial-budget-campaign`.

Public CLI and direct API controls reject all ten primary-invalid artifacts in
both profiles. Full-document comparator mutations reject forged proof/status/
body/token/action/mapping metadata. CLI and compiled API examples in
`docs/pluscal-recursive-procedures.md` are executed. Initial test class/style
mistakes are retained under `initial-test-campaign`; no primary evidence changed.

Reproduce with the pinned JRE/JAR from a private verified cache and fresh native
binaries. Paths below are variables supplied by the developer:

```sh
python3 -I make_cases.py
python3 -I capture.py --help
python3 -I compare.py --manifest fixtures/manifest.json --inputs cases --native "$EXAMPLE" --output comparison.json
python3 -I validate.py --java "$JAVA" --jar "$JAR" --native "$DIVE" --example "$EXAMPLE" --report comparison.json --inputs cases --fixtures fixtures --output validation
python3 -I qualified-replay.py --native "$DIVE" --output qualified-replay
python3 -I check-budgets.py --target "$TARGET" --native "$DIVE" --java "$JAVA" --jar "$JAR"
python3 -I test-comparison.py --native "$EXAMPLE"
python3 -I check-admission.py --native "$DIVE"
python3 -I check-api-admission.py --target "$TARGET"
python3 -I check-guide-cli.py --native "$DIVE"
python3 -I check-guide-api.py --target "$TARGET"
python3 -I verify.py --native "$DIVE" --example "$EXAMPLE"
```

The strict validation script intentionally exits nonzero for retained pending
replay disagreements. Qualified replay is a second disposition/evidence pass,
not a rewrite or suppression of those failures. Capture overwrites generated
study output only with a recorded reason; preserve the original campaign when
performing a different experiment. Nothing here certifies the entire PlusCal
translator, exact formatting/rewrite/checksum/mapping, process-owned frames,
scoped renaming, historical options or complete strict witness provenance.
F-034/F-035 remain partial/audit-required.

Worker widths in this study are requested widths. TLC encounter-order paths may
run inline with the actual execution mode and reason recorded in each checker
document. These controls prove result agreement, not physical parallel scaling.
No performance claim follows from this artifact/execution campaign.
