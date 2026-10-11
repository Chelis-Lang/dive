# Bounded multiline generation evidence

The immutable selection is 208 source rows across `primary`, `layout-controls`,
`supplemental`, `bool-controls` and `conditional-controls`. Every row retains
its input, clean pinned translator stdout/stderr, resulting artifact where one
exists, and parse/name-fixed/post-Explode AST bytes. The primary is the pinned
`94d0c50` JAR with SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
Java probes are development adapters; native runtime uses no Java.

The final `full-controls/report.json` and `partial-controls/report.json`
each account for every row in both profiles: 130 admitted, 34 named refusals,
20 parser rejections and 24 SANY-invalid primary outputs per profile. No
invalid artifact receives successful generation credit. Each admitted module
passes native and pinned SANY acceptance, exact significant whole-module CST
comparison and the pinned Validator. The Full report also compares exact
post-Explode AST bytes and CRC derivation. Pretty bytes are not claimed.
`--fair algorithm` retains the reference fair-header-only checksum limitation
(D-020), including its observed source-change disposition.

`baseline-qualified` preserves the unchanged producer's complete 136-source
Full/body-only refusal controls. Earlier candidate folders retain the weak
partial comparator and accepted forgeries, macro-declaration loss after
expansion, CASE parenthesis failures, premature missing-binary run and initial
API filename failure. They are historical failures, not final positive credit.
The original sources and captures were not filtered or overwritten.

Build native examples and the two Java development probes in a private output
directory; set `JAVA`, `JAR`, `DIVE`, `EDITOR`, `CST`, `ADAPTER` and `API` to those
executables/directories. From this repository root, repeat:

```sh
python3 -I crates/dive-pluscal/multiline-acceptance/full-controls.py --java "$JAVA" --jar "$JAR" --native "$DIVE" --editor "$EDITOR" --cst "$CST" --validator "$ADAPTER"
python3 -I crates/dive-pluscal/multiline-acceptance/partial-controls.py --java "$JAVA" --jar "$JAR" --native "$DIVE" --editor "$EDITOR" --cst "$CST" --validator "$ADAPTER"
python3 -I crates/dive-pluscal/multiline-acceptance/check-mutations.py
python3 -I crates/dive-pluscal/multiline-acceptance/check-partial-controls.py --java "$JAVA" --jar "$JAR" --validator "$ADAPTER"
python3 -I crates/dive-pluscal/multiline-acceptance/models.py --java "$JAVA" --jar "$JAR" --native "$DIVE"
python3 -I crates/dive-pluscal/multiline-acceptance/check-guide.py --native "$DIVE" --api "$API"
python3 -I crates/dive-pluscal/multiline-acceptance/check-bounds.py --editor "$EDITOR"
python3 -I crates/dive-pluscal/multiline-acceptance/verify.py
```

Repeat into an isolated copy: scripts create their output folders and must not
overwrite the committed receipt. `check-guide.py` runs the Cargo-owned
`dive-api/examples/pluscal_multiline.rs` executable; it tests 20 CLI/API
profile/policy/disposition controls and preserves raw outputs. Mutations reject
37 Full and 35 body-only forged/incomplete reports. The partial corruption
controls independently observe that source edits cannot be detected by a
disabled algorithm checksum while body edits are detected.

`finite-models` retains 28 safe/violating models with clean primary state counts,
336 native profile/worker/store comparisons, 14 independently valid TLC-profile
witnesses and 56 invalid mutants. These finite models are a separate check,
not a proof of every source grammar or AST serialization shape. Source/node/
output limits remain global. F-034 and F-035 remain partial and audit-required;
no full translator, formatter, integrity or model-proof certification is claimed.
