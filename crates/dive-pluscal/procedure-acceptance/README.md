# Bounded sequential procedure evidence

This study preserves all 84 capture-first C/P procedure inputs. The pinned
translator accepted 56 modules that SANY accepts, rejected 24 inputs, and wrote
four modules SANY rejects. The native artifact comparison accounts for all rows:
48 token artifacts, 14 matching rejections, 20 named refusals and two
**diagnostic-only invalid-reference** dispositions. All four SANY-invalid primary
outputs remain uncredited; eight CLI and eight direct API admission controls
reject them in both profiles.

All 48 admitted artifacts pass enclosing-module SANY and operator CST comparison.
The curated 76 generated models reproduce clean TLC verdict/count/depth across
1/4/8 workers and fingerprint/exact stores (456 width/store controls). The
original `validation/report.json` is preserved: 46 complete searches agree and
30 invariant witnesses have strict replay disagreements. These are the existing
#124 runtime string encounter-order refusal, not independently certified
TLC-profile counterexamples.

`qualified-replay/report.json` accounts for those 30 as **strict-witness-pending**.
Separate default/canonical replay checks the same 30 witnesses as valid and
rejects 120 corrupted traces. Every such control has `strict_tlc_credit=false`;
changing replay profile does not repair or certify the strict campaign. Full
F-034/F-035 certification remains pending. The current namespace guard is intact.

The first failed validator selection expected an invariant violation in a
call-to-Done model with no `E` label. Its raw captures and script are retained in
`initial-witness-selection/`; the corrected campaign uses that input only as a
complete search. `initial-comparison.json` also retains two native call/goto
rejections before the source-backed call/goto normalization fix. No selection
was dropped to make the active comparison pass.

The comparator rejects all 39 document mutations, including false proof,
false exhaustive status, wrong stage/exit/body/metadata and malformed mappings.
Four checked guide CLI controls and two compiled Rust guide examples establish
usable public entry points. Seven supplemental core/API main-body controls
retain clean primary captures for empty/procedure-only/call-only/error inputs
and reject internal-error/panic results. Their initial document-shape errors
remain visible in `initial-main-admission/`; they earn no coverage credit. Admission validates the enclosing TLA module;
raw generation is never sufficient for an accepted artifact or model proof.

## Reproduction

Build the native CLI and the `translate` example with a private target/cache and
two compile jobs. Supply explicit executable paths; Cargo rebuilds must not
replace binaries midway through a campaign. `capture.py` uses the pinned jar and
JRE for primary translation/SANY, and `JavaNormalizationProbe` only as a
reference AST adapter. Java is a development oracle, never native runtime code.
The manifest binds the jar, source commit, adapter, scripts, JDK and raw streams.

```sh
python3 crates/dive-pluscal/procedure-acceptance/compare.py --help
python3 crates/dive-pluscal/procedure-acceptance/validate.py --help
python3 crates/dive-pluscal/procedure-acceptance/qualified-replay.py --help
python3 crates/dive-pluscal/procedure-acceptance/check-admission.py --help
python3 crates/dive-pluscal/procedure-acceptance/check-guide-api.py --help
python3 crates/dive-pluscal/procedure-acceptance/verify.py --native "$DIVE_NATIVE" --example "$DIVE_TRANSLATE_EXAMPLE"
```

`delivery.json` binds the immutable checked sources, reports, copied executable
hashes and full local/release gate receipts. The source ancestry's CM1 selection
has 20 models and ValidationTestSuite smoke has 16, each run at 1/4/8 workers.
The lead integration uses the current 21-model CM1; this receipt does not claim
that additional integration check. Generated golden files are unchanged.

See [the user guide](../../../docs/pluscal-procedures.md) and
[ADR 0041](../../../docs/decisions/0041-pluscal-sequential-procedure-calls.md).
