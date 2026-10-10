# Single-process PlusCal translation

The original bounded process slice, extended by [multiple finite blocks](pluscal-multiple-processes.md), supports a process
declaration in current C and P syntax. A process with a constant ID has scalar
locals; a process set has locals indexed by `self`:

```tla
--algorithm A
variable result = 0;
process Worker \in {1, 2}
variables count = 0;
begin
  Step: count := count + 1; result := count;
  Finish: skip;
end process;
end algorithm;
```

`count` becomes a function. `Step(self)` updates `count[self]`, and the following
assignment reads the new value as `count'[self]`. The process counter is also a
function. `Next` schedules a worker through an existential quantifier and retains
the reference's terminating stutter. `process Worker = 1` instead keeps `count`
scalar while indexing `pc[1]`. The exact reference program-counter omission rule
for a single `while TRUE` body is preserved too.

The admitted domains are literal nonnegative integer/string IDs, finite literal sets and
intervals with nonnegative integer endpoints. Empty sets and duplicate entries are preserved in generated
source. Nonliteral/composite IDs remain explicit refusals. Translation does not
evaluate or prove the domain, and does not assert a model-checking result.

Process labels have a different default from sequential labels. A missing
process label **requires `--label`** (TLC spelling `-label`), even if the source
has no labels at all. `--label-root` changes the prefix but does not enable
insertion. Inserted labels retain their original statement associations. An
omitted local initializer becomes `defaultInitValue`; the JSON artifact returns
the required model-value `.cfg` binding in `configuration`, without writing files.

`artifact.processes` records admitted names, constant/set binding kind, local
storage variables and original source extents. It is source metadata, not a
claim that processes were explored. Accepted API artifacts pass enclosing-module
syntax/name/level analysis and report `model_proved=false` / `exhaustive=false`.

## Current limits

Unproved/overlapping process domains, procedure/macro fairness, procedures/macros, `define`,
name renaming, branch-internal labelled expansion, multiline/binder substitution,
nonliteral IDs and local membership initializers depending on another local or
`self` remain named refusals. Global/local/generated-name shadowing is refused
rather than silently renamed. Constant-ID references to `self` are refused:
the clean primary translator leaves those references unbound, and SANY rejects
the resulting module. No artifact credit is given to such primary outputs.

The existing source/node/depth/action/output bounds remain active. Translation
uses no Java runtime, performs no host I/O, rewrites no files, and makes no
pretty-width, checksum or upstream map-format claim. Neither compatibility
profile silently expands unproved families.

## Retained evidence

`crates/dive-pluscal/process-acceptance/` retains **242** selected rows: the main
208, 16 process/fairness/name/Unicode interactions, 12 subscript/initializer
interactions and six reserved-name controls. There are **102** token artifacts,
**91** matching rejections and
**49** named refusals, with zero disagreements. Every admitted artifact also
matches the primary full-module operator CST and passes separate pinned SANY.
Process storage metadata and inserted label names/positions are compared with
the clearly labelled development-only normalized-AST adapter.

**26** finite model controls match clean TLC verdicts/counts/depth at **1, 4 and
8 workers**, using **fingerprint and exact stores**. Seven invariant witnesses
pass independent replay, and 28 Init/Next/closing/missing-local-value mutants
are rejected. Seventeen full-document comparator corruptions are rejected,
including forged proof, internal errors and altered process storage metadata.
These curated controls do not certify the full PlusCal or API family.

The initial 148-row capture is retained separately. Its misplaced C local
declarations remain rejection controls; correctly formed C rows were added
without overwriting them. Unspaced `x=<<0,0>>` is also retained: the primary
lexer reads `=<` and rejects it. Separately captured spaced tuple declarations
provide positive subscript controls. Translator-success/SANY-failed outputs
remain visible and receive no artifact credit. A failed development check-report
contract is preserved in `initial-check-contract` with its non-evidence status;
fresh controls use actual trace documents plus independent replay. The initial
ID-token projection also retains two secondary string-quote discrepancies;
`TLAToken.STRING` stores contents without outer quotes, now compared explicitly.
Neither development extractor discrepancy changes or excuses artifact semantics.
Six additional primary controls expose an inherited reserved-name admission defect:
`PcalSymTab` always reserves global `pc`, even when generated pc storage is
omitted. Four former native false acceptances are preserved in
`names/before-fix.json`; both sequential and process forms now reject that global
name, matching all six primary rejections.

Use a private cache and target. The capture checks the pinned jar SHA; Java and
the normalized-AST adapter are development tools only:

```sh
python3 -I crates/dive-pluscal/process-acceptance/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_JAR" \
  --inputs crates/dive-pluscal/process-acceptance/cases \
  --output crates/dive-pluscal/process-acceptance/fixtures \
  --reason 'Explicit reason for replacing process evidence'
cargo build -p dive-pluscal --example translate --release --locked
cargo build -p dive-cli --release --locked
python3 -I crates/dive-pluscal/process-acceptance/compare.py \
  --manifest crates/dive-pluscal/process-acceptance/fixtures/manifest.json \
  --inputs crates/dive-pluscal/process-acceptance/cases \
  --native "$DIVE_TARGET/release/examples/translate" \
  --output crates/dive-pluscal/process-acceptance/comparison.json
python3 -I crates/dive-pluscal/process-acceptance/validate.py \
  --java "$DIVE_JAVA" --jar "$DIVE_JAR" \
  --native "$DIVE_TARGET/release/dive" \
  --example "$DIVE_TARGET/release/examples/translate" \
  --report crates/dive-pluscal/process-acceptance/comparison.json \
  --inputs crates/dive-pluscal/process-acceptance/cases \
  --fixtures crates/dive-pluscal/process-acceptance/fixtures \
  --output crates/dive-pluscal/process-acceptance/validation
```

Repeat the same comparison/validation commands with `interactions/` and
`subscripts/` paths. The complete prior sequential/normalization/label selections
are retained and rerun; historical evidence files are not edited.

The [fairness increment](pluscal-fairness.md) adds bounded weak/strong process
declarations, post-colon label modifiers and four pinned fairness options. The
242-row study above remains the original unfair-process delivery evidence.
