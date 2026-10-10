# PlusCal source grammar controls

These fixtures are separate from translated-model goldens. Primary calls pinned
`ParseAlgorithm.getAlgorithm`, including normalization. `--source-grammar` is
an explicitly secondary adapter calling original public grammar methods before
global macro/label normalization. GetMacro still expands earlier macros and
calls fold their tails; those facts remain visible.

The selection is 57 owned cases, six edges, and 159 upstream marker rows.
`fixtures/imported/selection.json` retains commit/path/file hash/byte region.
Four documentation markers remain selection errors. Every input/response has
a SHA-256. Raw `.stdout.gz`/`.stderr.gz` decompress to original bytes identified
by the manifest; AST payloads are base64 JSON. Gzip changes no oracle result.

From the repo root, with verified private cache and development tool paths:

```sh
python3 -I crates/dive-pluscal/acceptance/select.py \
  --cache "$DIVE_CACHE_DIR" --output "$DIVE_PLUSCAL_SELECTION"
python3 -I crates/dive-pluscal/acceptance/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" \
  --inputs "$DIVE_PLUSCAL_SELECTION/inputs" --output "$DIVE_PLUSCAL_CAPTURE" \
  --reason 'Explain this generated capture' --timeout 3
cargo build -p dive-pluscal --example parse
python3 -I crates/dive-pluscal/acceptance/compare.py \
  --native target/debug/examples/parse \
  --bundle crates/dive-pluscal/acceptance/fixtures/curated/manifest.json crates/dive-pluscal/acceptance/cases \
  --bundle crates/dive-pluscal/acceptance/fixtures/edges/manifest.json crates/dive-pluscal/acceptance/edge-cases \
  --bundle crates/dive-pluscal/acceptance/fixtures/imported/oracle/manifest.json crates/dive-pluscal/acceptance/fixtures/imported/inputs \
  --selection crates/dive-pluscal/acceptance/fixtures/imported/selection.json \
  --output conformance/reports/pluscal-source-grammar.json
cargo test -p dive-pluscal
```

To regenerate owned cases, select `acceptance/cases` or `acceptance/edge-cases`
as inputs with a new output directory. Capture verifies the jar digest, starts
a fresh bounded JVM per case and retains errors/timeouts. Never mutate cache
checkouts. Probe output never enters native parsing/translation.

Comparison preserves ordered operands/branches, unfolding only C blocks, P elsif
chains, binder lists and folded call tails. Unknown variants fail. Earlier macro
expansion differences remain pending, never AST agreements. Primary statuses
remain separate. Offline Rust tests verify hashes and ten false-agreement controls.

Extracts are MIT: Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, tlaplus
`94d0c5070852b37b801bb8998c436383ab7ee37b`. Their notices are retained in
`fixtures/EXAMPLES-LICENSE.md` and `fixtures/TLAPLUS-LICENSE`. This AST implements
no translator options, checksums or output specifications; F-034 is uncertified.
