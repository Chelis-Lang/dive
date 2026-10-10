# Sequential PlusCal labels and initial values

`dive translate FILE.tla --output json` now handles the pinned translator's
bounded sequential normalization defaults in both current C and P syntax.
Source parsing still preserves your original algorithm; generated labels are
reported separately from the source AST.

A sequential algorithm with **no explicit labels** gets labels automatically:

```tla
--algorithm A
variable x = 0;
begin
  x := 1;
  x := 2;
end algorithm;
```

The pinned translator and dive produce actions `Lbl_1` and `Lbl_2`; the repeated
assignment begins a new atomic action. If the algorithm contains any explicit
label, required missing labels produce a diagnostic. `--label` (TLC spelling
`-label`) permits insertion in that case. `--label-root Step` (`-labelRoot Step`)
changes generated names to `Step1`, `Step2`, and so on; it does **not** enable
insertion. `--report-labels` (`-reportLabels`) enables insertion and exposes the
same native JSON metadata: inserted names, original statement byte extents and
reference display positions. Historical human message formatting is pending.

Omitting a global initializer follows TLC's model-value initialization:

```tla
--algorithm A
variable x;
begin L: x := 1; E: skip; end algorithm;
```

The generated module declares `CONSTANT defaultInitValue`, and `Init` assigns
`x = defaultInitValue`. The JSON artifact's `configuration` field contains
`CONSTANT defaultInitValue = defaultInitValue` for your `.cfg`. This model value
is not zero or an empty set. Translation does not write or update files.
Unproved collisions with that generated constant remain explicit refusals.

In sequential algorithms, label modifiers are spelled **`L:+` / `L:-`**. The
pinned translator retains the program counter when either is present, including
an otherwise eligible `while TRUE` loop. It does not generate process-label
strong/weak fairness formulas for these sequential labels. A fair algorithm
retains `WF_vars(Next)`. Process-label fairness is now covered by the separate
[process fairness contract](pluscal-fairness.md).

## Boundaries and evidence

This increment covers admitted sequential controls and preserves the original
[translation boundary](pluscal-translation.md). Labels inserted inside `if`,
`either` or `with` still receive a named refusal: that control-flow expansion
has not been independently proved. Procedures, `define`,
name disambiguation, binder/multiline substitution, global fairness options,
historical modes, file rewriting/checksums and upstream map formatting remain
pending. Neither profile silently expands these families. Bounded [multiple finite
processes](pluscal-multiple-processes.md) and [macros](pluscal-macros.md) now have
separate admission contracts; this matrix records the original sequential scope.

Source/node/depth/action/output limits remain active. Label roots have a
256-byte native resource bound; syntactically invalid roots are rejected. An
artifact still passes full enclosing-module syntax/name/level admission and
always reports `model_proved=false` / `exhaustive=false`.

The owned selection in `crates/dive-pluscal/normalization-acceptance/` retains
**110** C/P option cases: **82** matching token artifacts, **22** matching
rejections and **6** named refusals, with zero disagreements. Each of the 82
admitted modules also matches the primary span/trivia-free operator CST and
passes separate pinned SANY validation. The secondary `NormalizationProbe`
adapter records `getAlgorithm` ASTs; native inserted label names, reference
positions and default-initialization status match that adapter for all admitted
rows. It is development evidence, never a runtime dependency or a replacement
for the clean primary `pcal.trans` run.

**16** generated finite-model controls match clean single-worker TLC verdicts
and counts (12 complete, 4 invariant violations). All four violations pass
independent replay; 12 Init/Next/missing-value mutants are rejected. A separate **16**-row C/P interaction selection covers `either`, `elsif`, nested
`with`, grouping and Unicode/tab positions: **11** artifacts and **5** rejections.
The initial two tab-coordinate disagreements from the secondary adapter are
retained in `interactions/initial-reference-adapter`; that adapter originally
skipped the primary `trans.removeTabs` and `ParseAlgorithm.uncomment`
preprocessing, for which the reader
explicitly warns that backspacing over tabs gives incorrect results. The corrected
adapter follows both primary preprocessing calls. No primary row or native position is weakened.

Twelve
complete-document comparator corruptions are rejected, including forged proof,
internal error, wrong tokens/artifact evidence, mapping extents, label names/positions
and required configuration. Prior translator selections and invalid primary
outputs remain retained; new regression dispositions are separately reported in
`previous-selection.json` (75 current artifacts, 60 rejections, 25 refusals).
The two retained label-admission rows now match their primary artifacts too.
These bounded controls do not certify full PlusCal
or API family parity.

Reproduce using private paths in environment variables (the scripts verify the
pinned jar hash). `DIVE_JAVAC` is a development-only JDK 17 compiler:

```sh
python3 -I crates/dive-pluscal/normalization-acceptance/make_cases.py
python3 -I crates/dive-pluscal/normalization-acceptance/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_JAR" \
  --inputs crates/dive-pluscal/normalization-acceptance/cases \
  --output crates/dive-pluscal/normalization-acceptance/fixtures \
  --reason 'Explicit reason for replacing the normalization evidence'
cargo build -p dive-pluscal --example translate --release --locked
cargo build -p dive-cli --release --locked
python3 -I crates/dive-pluscal/normalization-acceptance/compare.py \
  --manifest crates/dive-pluscal/normalization-acceptance/fixtures/manifest.json \
  --inputs crates/dive-pluscal/normalization-acceptance/cases \
  --native "$DIVE_TARGET/release/examples/translate" \
  --output crates/dive-pluscal/normalization-acceptance/comparison.json
python3 -I crates/dive-pluscal/normalization-acceptance/validate.py \
  --java "$DIVE_JAVA" --jar "$DIVE_JAR" \
  --inputs crates/dive-pluscal/normalization-acceptance/cases \
  --fixtures crates/dive-pluscal/normalization-acceptance/fixtures \
  --report crates/dive-pluscal/normalization-acceptance/comparison.json \
  --example "$DIVE_TARGET/release/examples/translate" \
  --native "$DIVE_TARGET/release/dive" \
  --output crates/dive-pluscal/normalization-acceptance/validation
python3 -I crates/dive-pluscal/normalization-acceptance/test-comparison.py \
  --native "$DIVE_TARGET/release/examples/translate"
```
