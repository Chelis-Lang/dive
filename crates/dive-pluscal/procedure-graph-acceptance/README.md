# Bounded nonrecursive sequential procedure graphs

All 120 capture-first C/P inputs are preserved. The pinned translator writes 95
modules and rejects 25 sources; eight of those modules fail SANY. Native artifact
comparison accounts for 68 admitted artifacts, 19 matching rejections, 29 named
refusals and four diagnostic invalid-reference dispositions. All eight primary
invalid outputs remain uncredited, including those refused before generation.
Sixteen public CLI and sixteen direct API controls reject these inputs in both
profiles. Enclosing-module SANY and operator CST controls are separate from raw
significant-token comparison; no artifact is a model proof.

The initial validator incorrectly expected implicit callee fallthrough to
succeed. Clean pinned TLC reaches `pc="Error"` and exits 11 (deadlock). Its
stopped campaign, original script and captures are kept in
`initial-validator-selection/`. The corrected campaign retains explicit
fallthrough deadlock classes/counts; it never suppresses deadlock checking to
make a complete proof. Strict namespace replay remains pending under #124.
Default/canonical replay is separate qualified evidence, not TLC certification.
The independent validator's undecidable deadlock endings remain named refusals.

All 68 admitted artifacts pass enclosing-module SANY and operator CST checks.
The 130 clean model rows reproduce exploration verdict/count/depth at 1/4/8
workers in fingerprint/exact stores (780 controls). The original strict report
keeps 64 complete matches and 66 witness replay disagreements (62 invariant,
four deadlock). Qualification accounts for these without claiming a validated
TLC violation.

Source ownership and global action/node budgets are tested natively. Direct,
mutual and tail cycles, including unused/FALSE-branch cycles, are refused.
Duplicate scope/label names and process-owned stacks remain pending. The
comparator rejects 39 full-document mutations; every original source remains
in the active and frozen baseline comparisons.

## Reproduction

Build `dive-cli` and the `translate` example with a private cache/target and two
compile jobs. Copy binaries before starting a campaign so concurrent rebuilds
cannot replace them. `capture.py` uses the pinned primary translator/SANY and a
development-only Java AST normalization probe; native runtime code never invokes
Java. Run the supplied scripts with `--help` for explicit executable/fixture
arguments. `verify.py` binds all final sources/reports/captures and gate receipts.

```sh
python3 crates/dive-pluscal/procedure-graph-acceptance/compare.py --help
python3 crates/dive-pluscal/procedure-graph-acceptance/validate.py --help
python3 crates/dive-pluscal/procedure-graph-acceptance/qualified-replay.py --help
python3 crates/dive-pluscal/procedure-graph-acceptance/check-admission.py --help
python3 crates/dive-pluscal/procedure-graph-acceptance/check-guide-api.py --help
```

Generated goldens are unchanged. The frozen ancestry's conformance gate contains
20 CM1 models and 16 ValidationTestSuite smoke models at 1/4/8 workers; this is
not a claim of the lead's additional current 21st CM1 integration control.
F-034/F-035 remain partial and audit-required. See
[the executable guide](../../../docs/pluscal-procedure-graphs.md) and
[ADR 0042](../../../docs/decisions/0042-pluscal-nonrecursive-procedure-graphs.md).

The separately qualified canonical replay checks 62 concrete invariant witnesses
as valid and rejects 264 corrupted traces (four for every invariant/deadlock
witness). Four original deadlock endings remain unsupported even in this
profile. Every switched-profile control records `strict_tlc_credit=false`.
