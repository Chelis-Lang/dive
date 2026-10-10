# Bounded native PlusCal macro study

This capture-first study retains all 222 C/P source rows, original errors and
intermediate failed comparisons. Its current reports account for 134 admitted
artifacts, 48 matching grammar rejections, 30 named refusals, eight invalid
reference dispositions and two primary crashes. All 27 SANY-invalid primary
outputs remain uncredited. Raw generation equality is diagnostic only.

`cases/` and `fixtures/` retain the original 92 rows. `extra/`, `process/`,
`scope/`, `lexical/` and `origins/` add 48/32/16/26/8 separate inputs. The original
malformed C process-local declarations, misplaced fairness modifiers, initial
comparator failures and overaccepted non-ASCII string remain visible. Do not
regenerate or overwrite existing evidence; choose a new output directory and
record a reason. This study does not edit `conformance/golden/`.

Build `dive-cli` and the `dive-pluscal` example `translate`, then copy the
executables into a stable artifact directory before campaigns. Cargo's shared
profile builds can replace binaries during a gate; the recorded hashes must
refer to immutable copies. Set `DIVE_CACHE_DIR` to your private verified cache.
Java/JDK access below is for oracle development only.

For the main selection (repeat for each supplemental folder):

```sh
python3 -I capture.py --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" \
  --jar "$DIVE_ORACLE_JAR" --inputs cases --output new-fixtures \
  --timeout 60 --reason 'documented new oracle capture'
python3 -I compare.py --manifest new-fixtures/manifest.json \
  --inputs cases --native "$DIVE_TRANSLATE_EXAMPLE" --output new-comparison.json
python3 -I validate.py --report new-comparison.json --native "$DIVE_BINARY" \
  --example "$DIVE_TRANSLATE_EXAMPLE" --java "$DIVE_JAVA" \
  --jar "$DIVE_ORACLE_JAR" --inputs cases --fixtures new-fixtures --output new-validation
```

All 134 admitted artifacts pass pinned SANY and full enclosing-module operator
CST comparison. The 132 curated finite models match clean single-worker TLC
counts/verdict/depth at 1/4/8 workers in fp/exact stores (792 controls). Twenty-five
independent witnesses validate, and 100 corrupted witnesses are rejected. The
44 full-document comparator mutations include false proof/disposition and
source-map metadata corruptions, including invalid-reference diagnostic rows.
Public CLI and direct API admission checks reject 27 primary-invalid outputs
in both profiles (54 controls each). `doc-controls.json` records the four checked
user-guide examples. Three `multiple-regression-*.json` reports retain every
264 macro-free process input against the new generator.

`delivery.json` binds final sources, reports and gate logs. Verify the receipt
using its original frozen binaries:

```sh
python3 -I verify.py --native "$DIVE_BINARY" --example "$DIVE_TRANSLATE_EXAMPLE"
```

The receipt's gate is for the frozen ancestry: 667 Rust tests, 20 Python tests,
CM1 20 models and ValidationTestSuite 16 models at 1/4/8. Parent integration runs
its current CM1 21 independently. These are curated controls, not certification
of all PlusCal behavior. Procedures, unproved binder/namespace substitution,
historical options, rewriting/checksums and mapping-format parity remain pending.
See [the user guide](../../../docs/pluscal-macros.md) and ADR 0039.
