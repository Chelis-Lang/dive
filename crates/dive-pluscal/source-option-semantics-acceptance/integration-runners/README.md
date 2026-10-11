# Fresh combined-source replay

These configurable consumers are distinct from the original executed scripts and
receipts. Preserve the original 318-input uncommitted producer archive and all
752 selected raw/public rows. A replay must use copied immutable executables from
one explicitly recorded combined producer; no historical binary is relabelled.

Set `BASE` to `crates/dive-pluscal/source-option-semantics-acceptance`, `OUT` to a
new evidence directory, and `BIN` to the combined producer's immutable executable
directory. Set `JAVA`, `JAR`, `JAVAC`, `ADAPTER`, `WORKTREE`, and `CARGO_TARGET_DIR`
to the private pinned cache/compiler, fresh development adapter output and
combined checkout/target. The clean pinned jar SHA-256 remains
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
The adapter is development-only; clean primary captures remain authoritative.

```sh
"$JAVAC" -cp "$JAR" -d "$ADAPTER" "$BASE/SourceOptionProbe.java" "$BASE/OptionValidatorProbe.java"
python3 "$BASE/compare.py" --binary "$BIN/context" --cst "$BIN/cst" --out "$OUT/raw"
python3 "$BASE/public-controls.py" --binary "$BIN/context" --cst "$BIN/cst" --java "$JAVA" --jar "$JAR" --adapter "$ADAPTER" --comparison "$OUT/raw" --out "$OUT/public"
python3 "$BASE/integration-runners/models.py" --java "$JAVA" --jar "$JAR" --native "$BIN/dive" --public "$OUT/public" --comparison "$OUT/raw" --out "$OUT/models"
python3 "$BASE/integration-runners/report-controls.py" --raw "$OUT/raw" --public "$OUT/public" --out "$OUT/report-controls"
python3 "$BASE/integration-runners/guides.py" --worktree "$WORKTREE" --artifacts "$OUT/guide-build" --binary "$BIN/dive" --context "$BIN/context" --target "$CARGO_TARGET_DIR" --out "$OUT/guides"
python3 "$BASE/integration-runners/checksum-controls.py" --binary "$BIN/dive" --java "$JAVA" --jar "$JAR" --adapter "$ADAPTER" --guides "$OUT/guides" --out "$OUT/checksum-controls"
```

`OUT` must be an absolute external path or a fresh directory relative to `BASE`.
All output directories must be absent before their first invocation. Compile the
Cargo-owned `pluscal_source_context` and `pluscal_source_cst` examples against the
combined producer, then copy their executables as `context` and `cst`; build and
copy the combined CLI as `dive`. Hash all executable and source inputs in the
new receipt. The exact guide Rust code is extracted from the maintained user
Markdown and compiled in a separate Cargo project owned by that consumer.

The complete public selection has 752 unique profile rows. The original census
is 480 admitted exact CST/SANY/Validator rows, 16 primary-invalid uncredited rows,
32 rejection-accounted rows and 224 named refusals. Never change expected
admission to hide a new defect. The 24-model control selection spans 288
profile/worker/store settings. Strict valid witnesses and replay refusals are
reported separately; canonical or pending replay never confers strict credit.
The 15 forged report/context/CRC controls and six actual checksum/Validator
corruptions remain distinct from usable-artifact admission and model proof.

Run the current full local gate, all current offline source guards and current
CM1/VTS subsets on the same combined producer. Its runtime/census/source receipt
is separate from historical `550d959e` and the frozen original source-option
producer. F-034/F-035 remain partial/audit-required.
