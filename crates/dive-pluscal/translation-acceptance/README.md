# Sequential translation evidence

This matrix keeps every original 83-row selection, all 149 intermediate rows
and eleven labelled-assertion/Unicode/layout/label boundary controls: **160 inputs**. It includes
all 57 owned source grammar cases and explicitly retains malformed C brace /
compound-semicolon controls alongside their corrected counterparts. No selected
failure is removed. A separate two-input `label-admission/` selection retains
valid post-colon `L:+` / `L:-` syntax: both primary translations pass SANY, while
native normalization explicitly refuses unproved label fairness modifiers. The
original malformed pre-colon spellings remain in the complete 160-row matrix. The primary is clean pinned `pcal.trans`, version 1.12,
revision `94d0c5070852b37b801bb8998c436383ab7ee37b`, jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.

`fixtures/manifest.json` records complete input hashes, capture script, reason,
flags, oracle/JRE identity, raw gzip transcripts, artifacts and generated cfg.
The clean translator is not instrumented or given special translation flags.
SANY validation is a separate run. Five translator-success artifacts fail SANY:
`Pc_C_definitions`, `Pc_P_definitions`, `Pc_seq_next_conflict`,
`Pc_seq_vars_conflict`, `Pc_seq_with_conflict`. None earns artifact credit.

`report.json` accounts for **46 token-artifact matches**, **55 rejection matches**
and **59 named native refusals**, with zero disagreement. These are scoped
artifacts/rejections, not full historical translator diagnostics or formatting
parity. The comparator verifies every raw/input/output hash and complete
selection before comparing real generated-body tokens, actions, program-counter
metadata and UTF-8 byte associations. Internal errors and proof claims fail.
`mutation-controls.json` records eight complete-document corruption controls;
all are disagreements (false proof, wrong tokens/body, missing map/actions,
wrong pc, internal error and invalid extent).

`generated-controls/report.json` separately validates **all 46 admitted full
modules** with pinned SANY and span/trivia-free native CST equality against the
primary artifact, retaining node kinds, token kinds/text, operator grouping,
prime scope and junction nesting. Tokens alone cannot establish indentation
semantics. It checks **16 finite generated models** against clean TLC for final
truth/counts (12 complete, four invariant violations). Four real counterexamples
validate independently; all 12 mutated Init/Next/missing-variable traces fail.
Raw oracle/native/validator output and each generated model/config are retained.

`initial-control-shape/` preserves a failed development experiment: mutations
were written to the wrapper rather than `states[i].values`, and the harness
inspected the wrong validator field. Its four disagreements are not semantic
matches or current evidence. The repaired validator controls alter actual value
maps and require both exit and disposition consistency.

## Reproduce

Use your own verified cache and worktree/target. Provide `JAVA` and `JAR` paths
to the pinned JRE/jar; these are development tool inputs, never runtime dependencies.

```sh
python3 -I crates/dive-pluscal/translation-acceptance/select.py
python3 -I crates/dive-pluscal/translation-acceptance/capture.py \
  --java "$JAVA" --jar "$JAR" \
  --inputs crates/dive-pluscal/translation-acceptance/cases \
  --output crates/dive-pluscal/translation-acceptance/fixtures \
  --reason 'Recorded pinned translator evidence regeneration' --timeout 5
cargo build --locked -p dive-pluscal --example translate
cargo build --locked -p dive-cli
python3 -I crates/dive-pluscal/translation-acceptance/compare.py \
  --manifest crates/dive-pluscal/translation-acceptance/fixtures/manifest.json \
  --inputs crates/dive-pluscal/translation-acceptance/cases \
  --native "$CARGO_TARGET_DIR/debug/examples/translate" \
  --output crates/dive-pluscal/translation-acceptance/report.json
python3 -I crates/dive-pluscal/translation-acceptance/test-comparison.py \
  --native "$CARGO_TARGET_DIR/debug/examples/translate"
python3 -I crates/dive-pluscal/translation-acceptance/validate.py \
  --java "$JAVA" --jar "$JAR" \
  --native "$CARGO_TARGET_DIR/debug/dive" \
  --example "$CARGO_TARGET_DIR/debug/examples/translate" \
  --report crates/dive-pluscal/translation-acceptance/report.json \
  --inputs crates/dive-pluscal/translation-acceptance/cases \
  --fixtures crates/dive-pluscal/translation-acceptance/fixtures \
  --output crates/dive-pluscal/translation-acceptance/generated-controls
cargo test --locked -p dive-pluscal -p dive-api --test translation
```

The separate label-admission matrix can be regenerated with the same `capture.py`
and `compare.py` commands, using `label-admission/cases`, `label-admission/fixtures`
and `label-admission/report.json`. Its two source files are retained curated
inputs; it never replaces or filters the complete 160-row matrix.

The Rust offline test compares complete body tokens and complete-module CST for
every returned artifact in all 160 primary rows; source, bounds, UTF-8, small
caller stack, folded `elsif` depth and Unicode admission controls supplement it.
No generated `conformance/golden/` file is changed. F-034/F-035 remain partial.
Process/procedure/macro expansion, automatic labels, name disambiguation,
multiline/binder substitution, all options, pretty-width, checksum rewriting and
TLC map-format require later matrices. Native generation produces byte maps
only; it does not claim precise statement/token mapping-format parity.
