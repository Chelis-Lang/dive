# Bounded block rewriting and body checksums

This capture-first study covers native **in-memory source editing**, separately
from native generation/admission and finite checking. It does not certify full
PlusCal translation, exact generator pretty bytes, normalized/name-fixed AST CRC,
file writing, checksum semantic security or mapping-format parity.

The complete 258-row selection is retained: 96 original rows, 72 partial-header
rows, 84 corrected C follow-ups and six fair-checksum controls. The original C
selection accidentally omitted the required inner main-body braces; its raw
48 failures remain unchanged alongside corrected sources. Primary totals are
93 rejections and 165 translations, of which 162 pass SANY and three fail SANY.
No invalid primary artifact earns native success credit. Every raw stdout/stderr
and source is hash-bound. The clean primary is the pinned `pcal.trans -nocfg`;
callback and validator adapters are explicitly **development-only references**.

Each native CLI profile accounts for all rows: 146 admitted partial-checksum
replacements, 89 rejections and 23 named refusals. The editor-only primary-body
study has 48 literal exact-primary-byte matches, 98 explicit partial-policy edits,
16 named refusals, 93 uncredited primary rejections and three uncredited invalid
artifacts. Supplied primary-body matches earn **no native generation/admission or
model proof credit**. The default full checksum request is refused; explicit
`--checksums tla` disables the unproved AST CRC with the pinned sentinel. Under
compatibility, reports label this policy `tlc+overrides`.

Every 292 admitted native document is checked separately by pinned SANY and
Validator. Four body mutations must be detected as `TLA_DIVERGENCE`. Four curated
C/P plain/fair algorithms have safe and violating finite controls: eight models,
48 width/store runs, four independent strict valid witnesses and sixteen rejected
mutants. These minimal witness controls do not remove other namespace replay
limitations. Proof/result/exit/schema corruption controls run before any primary
rejection/disposition; accepted plans independently reconstruct source spans and
recompute body CRC. An additional 512 deterministic byte-budget and malformed
source-span controls check named failure and bounded replacement. Original marker-token false admissions and the initial raw
marker comparator failure remain archived before the fixed re-run.

D-020 records a separate measured upstream defect: all six fair algorithm CRCs
are `7c28162a` despite three different initial values/body CRCs. Four source-only
edit controls remain undetected by pinned Validator. This implementation does
not fix or certify full algorithm checksums.

Reproduce from the workspace root using private verified cache paths:

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
cargo build --release --locked -p dive-pluscal --examples
python3 -I crates/dive-pluscal/rewrite-acceptance/compare.py --native target/release/dive --example target/release/examples/rewrite
python3 -I crates/dive-pluscal/rewrite-acceptance/test-comparison.py --native target/release/dive --example target/release/examples/rewrite
python3 -I crates/dive-pluscal/rewrite-acceptance/check-bounds.py --example target/release/examples/rewrite
python3 -I crates/dive-pluscal/rewrite-acceptance/validate.py --java "$DIVE_JAVA" --jar "$DIVE_JAR" --adapter "$DIVE_REWRITE_ADAPTER"
python3 -I crates/dive-pluscal/rewrite-acceptance/models.py --java "$DIVE_JAVA" --jar "$DIVE_JAR" --native target/release/dive
python3 -I crates/dive-pluscal/rewrite-acceptance/check-guide.py --native target/release/dive --target target
python3 -I crates/dive-pluscal/rewrite-acceptance/verify.py
```

`capture.py` verifies the pinned jar hash before capture. Copy a study into a
new directory for a new campaign; never rerun it over prior raw evidence. Compile `ChecksumProbe.java` against that
same jar for `DIVE_REWRITE_ADAPTER`. Supplemental `selection.json` files are the
frozen exact source list; keep original failures, corrected follow-ups and all
258 dispositions. `delivery.json` binds final sources, immutable executable hashes,
reports, raw evidence and gate logs. The user contract and executable examples are
in [`docs/pluscal-rewriting.md`](../../../docs/pluscal-rewriting.md).
