# Conformance corpora

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

The original 263-model full-tier inventory contained 171 in-scope model
references, 44 in-scope models without references and 48 previously excluded
models. A separate primary run attempted each of those 44 models once, with
the original pinned Examples source, authored configurations and model modes.
It used pinned TLC, one worker, two jobs, a 3 GB heap and the original
600-second per-job limit, with no overall campaign cap. A separate trace pass
can also receive its own 600 seconds.

The harness generated 27 facts: 25 exhaustive-mode references and two
100-trace simulation references. Their observed results are 24 successes,
one simulated invariant violation and two classified initial-state errors
(TLC code 2102, exit 255). A simulation success does not prove exhaustive
correctness. The violation's generated trace contains 26 states and 25 actions.
All files here are copied byte-for-byte from harness generation; no expected
result or generated provenance was edited. The existing 1,623 references are
unchanged.

The remaining 17 original models exceeded 600 seconds and received no
reference. They remain in scope, without native agreement credit:

- `specifications/CarTalkPuzzle/CarTalkPuzzle.toolbox/Model_3/MC.cfg`
- `specifications/KeyValueStore/MCKVSSafetyLarge.cfg`
- `specifications/KeyValueStore/MCKVSSafetyMedium.cfg`
- `specifications/MultiCarElevator/ElevatorLivenessLarge.cfg`
- `specifications/MultiCarElevator/ElevatorSafetyLarge.cfg`
- `specifications/MultiPaxos-SMR/MultiPaxos_MC.cfg`
- `specifications/NanoBlockchain/MCNanoLarge.cfg`
- `specifications/SlushProtocol/SlushLarge.cfg`
- `specifications/TLC/TestMCReachability.cfg`
- `specifications/bcastFolklore/bcastFolklore.cfg`
- `specifications/byzpaxos/BPConProof.cfg`
- `specifications/byzpaxos/PConProof.cfg`
- `specifications/c1cs/c1cs.cfg`
- `specifications/dag-consensus/TLCSailfish2.cfg`
- `specifications/detector_chan96/EnvironmentController.cfg`
- `specifications/diskpaxos/MC_HDiskSynod.cfg`
- `specifications/ewd998/EWD998.cfg`

Thus the original inventory now has 198 in-scope model references, 17 missing
references and the same 48 exclusions. This is reference availability, not a
new native full-tier measurement. No model was excluded, resized or assigned
a different runtime limit or mode. The original 156-model small-tier
population and its agreement counts do not change.

Owning harness streams, physical status, generated provenance and the full
44-model outcome accounting were retained externally. Original per-Java
stdout/stderr streams were not exported by the generator. Independent review
checked the 27 facts and original inputs, then performed seven direct pinned
Java executions across six selected models, including both classified errors,
both simulation modes and the exact violation trace. It did not rerun the
other 21 facts or repeat the 17 timeouts. Native full-tier checking, semantic
validation of the new violation trace and the full-tier parity target remain
required.
