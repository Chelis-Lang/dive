# Generated ValidationTestSuite coverage

ValidationTestSuite at `fe99cff1ca48535bdbfc99fd95bd2706f056c12f` is a Python
pairwise-test generator, not a directory of ready-to-run models. Its pinned
source remains registered as `validation-test-suite`; the generated bounded
subset is registered separately as `validation-smoke`.

The selection combines case features `Eq`, `Let`, `InstanceNamedWith`,
`Enabled` and `BoolTrue` with plugs `BoolTrue` and `BoolFalse`. Both deadlock
settings are generated. It disables symmetry and anomalous-environment cases,
and requests upstream's reduced worker variants (1 and 2).

Every selected testcase is recorded in
`conformance/corpus/validation-smoke/selection.json`:

| Selected cases | Treatment |
| --- | --- |
| 16 single-worker TLC sides | Imported, with unmodified generated TLA+ and `.cfg` files |
| 16 alternate-worker variants | Explicitly out of scope for this single-worker-side import |
| 4 upstream skips | Retained with the generator's reason |

The raw upstream `specification.json`, generator recipe and dependency versions
are preserved alongside the census. Reference TLC/Apalache comparisons are
**not executed** by the import. Neither upstream's reduction relationship nor
its reference implementation is assumed to supply an oracle result. Our pinned
TLC supplies every model golden; SANY supplies every module golden. Model
manifests record expected result and runtime as `unknown`, putting them in the
deep tier without inventing runtime measurements or expected truth.

The first oracle run records eight successes, six invariant violations and two
action-property violations. TLC reproduces all 16 model goldens and accepts
all 20 modules. `dive check --compat=tlc` fully agrees on all 16, including
independent counterexample validation; `dive analyze` agrees on all 20 modules.
The model gate passes at 1, 4 and 8 workers. These are safety/action-property
cases; they do not demonstrate general liveness conformance. The separate
final/short study refuses these cases because they have no temporal properties.

The required `Conformance gate` includes all imported models using
`conformance/gates/validation-smoke.txt`. Nightly module checks include the
imported modules, and the deep tier includes their models in both profiles.
The import controls require that the gate list equals the imported census,
reject unhandled options/types, reject path escapes and digest changes, and
reject empty selections. Changing a recipe, model or census changes the local
corpus tree hash and requires golden regeneration.

## Reproduce

Fetch the pinned source with the harness, then use Python 3.9 (this run used
3.9.6) and the generator-only dependencies:

```sh
target/release/dive-conformance corpus fetch --corpus validation-test-suite
python3.9 -m venv .validation-venv
.validation-venv/bin/pip install -r scripts/validation-requirements.txt
PYTHONHASHSEED=0 PYTHONDONTWRITEBYTECODE=1 .validation-venv/bin/python \
  scripts/import-validation-suite.py \
  --source "$DIVE_CACHE_DIR/git/validation-test-suite/fe99cff1ca48535bdbfc99fd95bd2706f056c12f" \
  --out /tmp/validation-smoke-new
diff -rq conformance/corpus/validation-smoke /tmp/validation-smoke-new
python3 scripts/test-validation-import.py
```

Set `DIVE_CACHE_DIR` to the verified cache before these commands (the harness's
default is `~/.cache/dive`). The script requires a clean checkout at the pin,
works in a disposable source copy and never modifies a cache checkout. It never
runs the bundled upstream jars or reference checkers. Existing destinations
are refused; review regenerated content before replacing the tracked corpus.

After a reviewed corpus change, generate goldens and run the check:

```sh
target/release/dive-conformance golden generate --corpus validation-smoke \
  --tier all --jobs 2 --timeout 30 --reason 'Describe the reviewed corpus change'
DIVE_GATE_CORPUS=validation-smoke scripts/conformance-gate.sh \
  conformance/gates/validation-smoke.txt --workers 1,4,8 --timeout 30
```

Generated files are derived from NVIDIA's Apache-2.0 source. The source licence
and copyright are preserved in the corpus's `LICENSE.md` and `NOTICE`; the
adapter is original repository code. Coverage beyond this bounded selection,
symmetry/anomalous cases and the JUnit-driven TLC model corpus remain open in
issue #2. This import does not close that issue.
