# Conformance corpora

## Constant predicate binding capacity

`constant-predicate-binders` contains two finite Boolean models with a pure
5,000-element set constructor inside an always property. The pinned TLC oracle
accepts the holding property (generated 3, distinct 2, depth 2); it normalizes
the false property to an initial invariant failure of `P` (code 2107, physical
exit 12, counts unreported). This distinguishes admission capacity from actual
predicate truth and preserves the oracle's safety classification.

Generate these model goldens with:

```sh
target/release/dive-conformance golden generate \
  --corpus constant-predicate-binders --tier all --no-sany \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 \
  --reason 'Requalify pure constructor capacity against the pinned TLC oracle'
```

The API controls additionally compare admitted planner and independent lasso
truth in both profiles, check exact class/status and holding counts, and validate
native and oracle witnesses against a fresh model. Forged initial states fail
independent validation. Effectful, checker-reading and infinite constructor
domains retain separate refusal controls. These two models are an explicit
local corpus; they do not enlarge the existing small-tier examples denominator.

## Generated ValidationTestSuite coverage

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

## INSTANCE query namespace controls

`instance-temporal` is a separate local corpus of ten finite Boolean models and
23 modules. It preserves primary-first observations of lexical INSTANCE and
`ENABLED` successor namespaces before runtime changes. Every source input is
bound by SHA-256 in
[`selection.json`](../conformance/corpus/instance-temporal/selection.json),
along with pinned TLC/JRE identities, the original clean primary outcome and
stream hashes. Original raw observations remain external.

| Models | Clean primary outcome |
| --- | --- |
| `guard_decoy_exported`, `guard_decoy_nested` | Success; generated 3, distinct 2, depth 2 |
| `conflict_same_direct`, `conflict_same_outside`, `conflict_no_root_nested`, `rebind_no_root_fresh` | Success; generated 2, distinct 1, depth 1 |
| `conflict_same_exported`, `conflict_decoy_exported`, `conflict_no_root_exported`, `rebind_no_root_old` | Initial invariant `P` failure; code 2107, physical exit 12; counts and depth absent |

All selected modules were accepted by SANY. The exported conflict results are
surprising: substitutions captured before the query do not identify its two
successor names. By contrast, the nested case shares the hidden name `q` and
remains unsatisfiable. The guard cases retain the actual current-state value; a
same-spelled caller variable does not replace it. The old/fresh pair distinguishes
bindings established before and after the query branch. These distinctions
prevent a blanket flattening of substitutions into caller root-variable IDs.
The pinned source explanation is recorded in [provenance](provenance.md#instance-query-namespace-controls).

Manifest runtimes are the original measured clean TLC process times rounded up
to whole seconds: all ten are `00:00:01`. These models therefore enter the small
tier normally. They are nevertheless reported as an explicit separate corpus;
the established 156-ID selection (examples, probes, validation-smoke and the two
configured-temporal-operators models) remains unchanged and separately reported.
Adding these controls does not increase its denominator or demonstrate a gain.
Gate lists and the original corpora and goldens are unchanged.

Set `DIVE_RUN_OUTPUT` to an external output directory. Generate only this corpus through the ordinary pinned harness, then self-check
the generated goldens with TLC:

```sh
dive-conformance golden generate --corpus instance-temporal --tier all \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 --timeout-cap 30 \
  --reason 'Record lexical INSTANCE ENABLED namespaces before runtime implementation'
dive-conformance run --impl tlc --corpus instance-temporal --tier all \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 --timeout-cap 30 \
  --no-summary --report-dir "$DIVE_RUN_OUTPUT/instance-temporal-selfcheck"
```

The harness supplies pinned Java and libraries, seed 0, fingerprint 0 and final
liveness checking. Its SANY process uses the existing fixed 1 GB heap; the TLC
heap above is 512 MB. Goldens preserve the harness's separate clean-result and
trace-pass consistency provenance. Internal transient Java streams are not
exported by this workflow; preserve the harness streams and physical status,
generated provenance, and already captured primary input/stream bindings.
A TLC self-check establishes oracle reproducibility, not native conformance or
independent validation of a counterexample. General fairness and closing-edge
word controls, callable substitutions, and uncovered derived-query errors remain
required beyond this initial selection.


## Full-tier primary reference recovery

The full tier is `--tier all` over `examples` (233 models), `probes` (14),
`validation-smoke` (16) and `configured-temporal-operators` (2): 265 models,
of which 48 Examples models are out of scope (for example symbolic Apalache
models). The original 263-model inventory, taken before the last corpus was
added, had 171 in-scope model references and 44 in-scope models without one.

All references for the 44 models were generated by `golden generate` with the
pinned TLC, the pinned Examples source, the authored configurations and model
modes, and one worker. Each golden records its own `--reason`, timeout and
heap in its provenance; no file was edited after generation.

- **First pass** (3 GB heap, 600-second limit per job): 27 references, 25
  exhaustive and two 100-trace simulations. They are 24 successes, one
  simulated invariant violation (`specifications/ewd687a/EWD687a_anim.cfg`,
  `InterestingBehavior`, a 26-state trace) and two classified initial-state
  errors (TLC code 2102, exit 255). A simulation success does not prove
  exhaustive correctness.
- **Second pass** (8 GB heap, derived limits capped at four hours) and
  **third pass** (6 GB heap, eight-hour limit) over the models still without
  a reference: exhaustive successes only.

| Model | Pass | Distinct states | Depth | TLC time |
|-------|-----:|----------------:|------:|---------:|
| `specifications/KeyValueStore/MCKVSSafetyMedium.cfg` | 2 | 17,220,672 | 22 | 23 min |
| `specifications/MultiCarElevator/ElevatorLivenessLarge.cfg` | 2 | 50,653 | 39 | 38 min |
| `specifications/MultiCarElevator/ElevatorSafetyLarge.cfg` | 2 | 59,007,145 | 44 | 42 min |
| `specifications/MultiPaxos-SMR/MultiPaxos_MC.cfg` | 2 | 37,078,209 | 37 | 77 min |
| `specifications/NanoBlockchain/MCNanoLarge.cfg` | 2 | 120,130,843 | 15 | 86 min |
| `specifications/bcastFolklore/bcastFolklore.cfg` | 3 | 501,552 | 9 | 68 min |
| `specifications/byzpaxos/PConProof.cfg` | 3 | 1,009,133 | 28 | 43 min |
| `specifications/SlushProtocol/SlushLarge.cfg` | 3 | 244,335,240 | 59 | 265 min |

The remaining 9 models have no reference and stay in scope, without
agreement credit. A run reports each of them as `oracle-error` (no golden):

- `specifications/CarTalkPuzzle/CarTalkPuzzle.toolbox/Model_3/MC.cfg`
- `specifications/KeyValueStore/MCKVSSafetyLarge.cfg`
- `specifications/TLC/TestMCReachability.cfg`
- `specifications/byzpaxos/BPConProof.cfg`
- `specifications/c1cs/c1cs.cfg`
- `specifications/dag-consensus/TLCSailfish2.cfg`
- `specifications/detector_chan96/EnvironmentController.cfg`
- `specifications/diskpaxos/MC_HDiskSynod.cfg`
- `specifications/ewd998/EWD998.cfg`

The full tier now has 208 in-scope model references (including the two
`configured-temporal-operators` models), 9 missing references and the same
48 exclusions. No model was excluded, resized or given a different
mode. The small tier and its agreement counts do not change.

Independent review of the first pass checked the 27 facts and their inputs and
reran six of the models directly with the pinned Java, including both
classified errors, both simulation modes and the exact violation trace.
`crates/dive-config/tests/tlc_oracle.rs` checks every model golden's
configuration, including the new violation's property name.

Twenty-five directories of the pinned Examples checkout have no
`manifest.json`, and four (`BlockingQueue`, `CCF`, `azure-cosmos-tla`,
`microwave`) are unfetched submodules; neither contributes models.
