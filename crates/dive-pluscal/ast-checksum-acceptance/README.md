# Bounded post-translation AST checksums

This study proves native serialized bytes for a bounded sequential PlusCal AST
subset. It does not certify full PlusCal translation, source integrity, exact
native generator formatting, mapping/file formats or model correctness.
[The user guide](../../../docs/pluscal-rewriting.md) gives runnable CLI and Rust
examples, profiles, quotas and all supported/refused shapes.

The primary oracle is the clean pinned `pcal.trans -nocfg` jar from
`conformance/oracles.toml` (commit `94d0c50`, SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`).
`AstChecksumProbe.java` is a development-only adapter: it captures the parsed,
name-fixed and post-translation/Explode AST bytes and token/origin fields. Only
native bytes equal to the corresponding post-translation capture earn
serialization credit. Equal reference stage hashes alone are insufficient.
The adapter is never linked into, invoked by or required by native execution.

## Complete retained selection

- `primary/`: 84 C/P straight-line/control/fairness rows, including source errors.
- `token-controls/`: 24 tuple-adjacency, malformed-token and escaped-string rows.
- `control-shapes/`: 60 fair/unfair C/P control-flow forms and internal labels.
- `context-controls/`: 24 name/defaultInitValue/pc/Done/missing-label contexts.
- `expression-controls/`: separate 24 Unicode/LET/CHOOSE/function/record/CASE,
  escaped-string, inline-comment and spaced-tuple RHS controls.
- `label-root-controls/`: separate 12 clean option/context controls, with eight
  exact AST/CRC derivations, two invalid-root rejections and two name-fix refusals.

The complete 216-row serialization census is **162 exact post-translation byte
matches, 32 named refusals and 22 accounted rejections**, with no mismatches. The
original 192-row census remains retained (138 exact/32 refused/22 rejected) and
is covered by the fast reference regression; all 24 supplemental expression
controls earn separately measured exact serialization evidence. Unsupported
multiline/junction/proof-token/name-fix and other shapes never produce a partial
CRC or pretend to be admitted translations. The same complete selection is
then run through the public Full rewrite CLI in both profiles. `full-controls/`
retains every disposition, clean primary/native SANY output and independently
invoked pinned Validator output. Its report is the authoritative artifact
census; primary-invalid outputs remain uncredited even if raw serialization
matches. Literal editor equality on supplied primary bodies earns editing-byte
evidence only, not native generation or admission credit.

The final public Full census accounts for all 216 rows in each profile:
**152 admitted derivations, 24 rejections, 36 named refusals and four rejected
primary-invalid artifacts**, with no mismatches. The clean primary translated
188 rows: 184 pass SANY and all four invalid outputs stay uncredited. Twenty-eight
primary source rejections remain visible; four have a named public native refusal
rather than a rejection-class match. The supplied-primary-body editor independently
matches 158 complete primary documents byte for byte, refuses 26 shapes and
leaves 32 primary-rejected/invalid inputs uncredited. These 158 editing matches
are not 158 native generator admissions. All 304 admitted profile/document rows
pass separate native SANY and pinned Validator checks; twelve checksum mutations
retain their measured classifications, including the weak fair-source result.
Thirty-seven forged/incomplete reports are rejected before credit, along with
512 malformed/UTF-8/budget controls. The 699-test full gate, ancestral CM1's 20
models and all 16 ValidationTestSuite smoke models pass; both model gates run at
1/4/8 workers. Current-main integration uses its own CM1 census and receipt.

`model_proved` and `exhaustive` remain false for generation/editing. Full
`algorithm_checksum_verified` means verified reference CRC derivation.
`algorithm_matched` compares CRCs, not source identity. For a fair algorithm the
reference hashes `fair` alone, flagged as `fair_header_crc_only`; unchanged-body
source edits therefore still return `NO_DIVERGENCE`. This measured weakness is
retained as D-020, not silently repaired or credited as integrity certification.

Separate controls retain corrupted algorithm/header/body/source bytes, forged
and incomplete Full JSON reports, deterministic byte budgets/UTF-8 spans,
compiled user guide examples and eight finite generated-model comparisons at
1/4/8 workers with fingerprint/exact stores. Four invariant witnesses are
independently replayed in strict TLC mode and sixteen broken witnesses rejected.
Those curated model checks do not convert the overall translator family into a
certificate. The production `delivery.json` binds the exact native source,
executables, gates and all retained evidence; `verify.py` checks it read-only.

## Failed controls remain visible

The initial wrong-jar invocation was rejected before capture. A failed header
extraction retains its original partial primary outputs. `initial-*` preserves
missing AST template whitespace and comparator-code mistakes; later
`proof-token-comparison-failure.json` preserves two genuine unsupported lexical
mismatches before the proof-token guard. The 52-admitted/28-refused original
straight-line observation remains its own history; it does not erase those
auxiliary failures. `pre-c-block-*` preserves conservative C wrapper refusals
before proved flattening. `initial-full-controls*` retains incorrect editor
exit accounting; `relative-classpath-*` retains a rejected Validator invocation.
`overstrict-error-schema-full-controls/` retains the failed assumption that
source-generation and semantic rejection causes are flat rewrite fields. The
final comparator checks their nested typed cause without excusing internal
errors or proof claims. `ledger-path-structural-failure.json` preserves a
nonliteral evidence-path schema failure; active ledger fields contain one
existing path, with secondary references retained separately.
`debug-*` reports/binaries describe the prior development execution separately
from the final source-bound release controls. No failed raw bytes or original
selection were overwritten to improve the final census.

## Reproduce in a fresh private copy

Copy this selection to a new worktree and use its own verified cache. Existing
capture directories intentionally cause capture scripts to stop; archive prior
production evidence instead of overwriting it. Set `JAVA`, `JAR`, `NATIVE`,
`EDITOR`, `CHECKSUM` and `TARGET` to the verified private tool paths; compile the
two development Java adapters against that exact jar only. Then run:

```sh
python3 -I compare.py --binary "$CHECKSUM"
python3 -I full-controls.py --java "$JAVA" --jar "$JAR" --native "$NATIVE" --editor "$EDITOR" --validator ../rewrite-acceptance/adapter
python3 -I check-mutations.py
python3 -I check-bounds.py --editor "$EDITOR"
python3 -I check-label-root.py --java "$JAVA" --jar "$JAR" --native "$NATIVE"
python3 -I models.py --java "$JAVA" --jar "$JAR" --native "$NATIVE"
python3 -I check-guide.py --native "$NATIVE" --target "$TARGET"
python3 -I verify.py
```

`capture.py`, `capture-controls.py`, `capture-shapes.py` and
`capture-contexts.py` and `capture-expressions.py` enumerate the selections before native comparison;
all primary translation/error bytes and stage captures remain accounted for.
Full AST shapes outside this explicitly proved subset remain pending under
issue #172 and ADR 0047; both relevant parity families stay partial/audit-required.
