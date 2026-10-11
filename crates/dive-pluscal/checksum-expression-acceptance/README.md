# Nesting and expression-layout checksum controls

This continuation of ADR 0047 and issue #172 retains a capture-first selection
for native post-translation AST serialization. The preceding Full checksum
producer remains immutable; its receipt describes only that producer's source.
Raw native serialization now accounts for 320 controls: 224 exact post-translation
bytes/CRC matches, 14 named refusals and 82 primary rejections. Public artifact
admission and finite model evidence remain separate stages.

`primary/` retains all 144 original C/P, fair/unfair nesting, expression-layout
and negative rows. A compound C statement before the following `E` label requires
a semicolon in the pinned parser. Those original failures remain visible;
`c-fixed-controls/` separately captures all 72 C forms with that separator. The
two original matrices retain 216 source rows, without erasing rejected inputs
or selecting only successful cases. Separately captured layout (24), With
separator (32) and empty-branch (32) controls bring the complete selection to
304. A separate 16-row terminal-empty-elsif capture brings the final selection
to 320; four original raw serializer disagreements remain in
`terminal-source-failure-comparison.json`. The initial failed serializer and incomplete public/CST campaign remain
archived with their original source and comparator.

The clean pinned `pcal.trans -nocfg` jar is the primary output authority. The
development-only `AstChecksumProbe` captures parsed, name-fixed and
post-translation/Explode bytes separately. It is never invoked by production
code. Exact native serialized bytes must match the corresponding post-translation
capture before a shape receives CRC credit; equal reference stage hashes alone
prove nothing about the native implementation. CRC reproduction is separate
from generated-module SANY admission, operator grouping, finite model checks
and independent witness validation.

The selected shapes include nested `elsif`, two/three and dependent `with`
binders, multiline assignment/declaration/tuple/record/conditional expressions,
blank lines, block/line comments, tab expansion and aligned/misaligned ASCII
junction columns. Malformed binders/tests/junctions, branch labels and proof-token
shapes are retained with their actual dispositions. Source-level name fixes,
processes, procedures and unrelated scoped serialization remain unproved.

The existing `--fair` header-only CRC weakness remains explicit: reproducing
the reference CRC does not establish source identity, integrity or model proof.
No corrected checksum default is authorized by this study. Any lifted boundary
must include public Full rewrite, SANY/Validator corruption and executable user
CLI/Rust controls before delivery; full PlusCal family certification remains
pending.

The serializer now matches raw multiline line-vector/junction-column captures,
but the native generator still names multiline layout/substitution as unsupported.
Empty Then with a nonempty Else parses but generation refuses explicitly; both
empty P branches and empty C bodies retain primary rejections. Empty Else after
a nonempty Then generates the pinned TRUE conjunct before identity updates.
The first native output omitted that conjunct; `operator-cst-failure/` retains
the exact primary/native modules and trees, and `full-controls-first-incomplete/`
retains every completed prefix row. No equal-token shortcut excuses different
operator grouping.

The complete main public campaign accounts for 320 rows in each profile:
86 admitted Full replacements, 140 named refusals, 82 primary-source rejections
and 12 rejected primary-invalid outputs. All 16 invalid reference artifacts
remain uncredited, including four with named refusal dispositions. Every
admitted replacement passes significant operator-CST equality, SANY and the
independent pinned Validator (172 profile/source pairs). All 12 header/body/source
corruptions pass; the weak fair-header source control still reports
NO_DIVERGENCE as a limitation. Literal supplied-primary-body editing matches
218 documents without generation/admission credit; four named refusals and
98 source-invalid/rejected dispositions remain.

Two separately captured `rewrite-boundary-controls/` retain the obsolete old
elsif refusal test and a demonstrably unproved macro case. The former matches
literal reference AST bytes/CRC and generated operator structure; Full CLI/API
controls admit it in both profiles. Macro CRC remains a named refusal. The old
702/703 gate is archived as `second-failed-gate.log`, with its exact old test in
`rewrite-before-elsif-admission.rs`; the new assertion does not weaken modified
body/missing/ambiguous header checks.

Curated model controls cover 12 complete/violation rows at 1/4/8 workers in
fingerprint/exact stores (72 runs), six independently valid strict witnesses
and 24 invalid mutants. Multiline generation retains its separate named refusal.
All 37 forged Full reports and 512 byte/span controls reject or respect quotas;
the guide executes eight replacements, four multiline refusals and two compiled
Rust examples. Ancestor CM1 has 20 models; the parent integrates and checks its
current 21-model gate separately. No results are silently retagged as currentmain.

Reproduce the native comparison with a release `algorithm_checksum` example:

```sh
cargo build --release --locked -p dive-pluscal --example algorithm_checksum
python3 -I crates/dive-pluscal/checksum-expression-acceptance/compare.py \
  --binary target/release/examples/algorithm_checksum --name fresh-comparison
```

`full-controls.py` additionally takes `--native`, `--editor`, `--cst`, `--java`,
`--jar` and `--validator` paths. Build the native CLI and `rewrite`/`translate`
examples; obtain Java and the exact jar through the pinned harness. The
Validator adapter is `crates/dive-pluscal/rewrite-acceptance/adapter`. Run in a
fresh copy of this study so existing complete/failed raw reports are immutable.
`models.py`, `check-guide.py`, `check-mutations.py` and `check-bounds.py` retain
separate execution/refusal/integrity evidence. `receipt.py` binds the final
source and executables; `verify.py` is read-only and only validates that exact
producer, never a subsequently integrated main tree.
