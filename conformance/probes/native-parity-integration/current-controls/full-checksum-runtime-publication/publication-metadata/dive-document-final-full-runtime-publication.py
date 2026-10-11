from pathlib import Path
import json

root = Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work')
rel = 'conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-publication/verification.json'
receipt = json.loads((root / rel).read_text())
assert receipt['source_commit'] == 'b8fb13cdab443e7ad0206a467d9a392075f31397' and receipt['rust_tests'] == 982
base = json.loads((root / Path(rel).parent / 'base/full-controls/report.json').read_text())
nesting = json.loads((root / Path(rel).parent / 'nesting/full-controls/report.json').read_text())
assert base['selected'] == 216 and nesting['selected'] == 320
text = f'''
## Current combined publication checks

The [current source-bound receipt](../{rel}) records the corrected combined
runtime: 982 workspace tests, zero skipped, all eight local gate stages, the
21-model safety gate and 16 ValidationTestSuite smoke models at 1/4/8 workers.
The validation smoke controls also retain CI's existing 30-second per-model
bound. Local timings were measured under shared load and are not performance
claims.

Fresh Full controls account for the complete 216-row base selection and
320-row nested/layout selection in both profiles. Every admitted artifact
passes native enclosing-module analysis, pinned SANY, pinned Validator and an
exact complete nontrivia syntax-tree comparison preserving parentheses, primes
and operator grouping: {receipt['full_base_complete_cst_pairs']} base pairs and
172 nested pairs. Both selections retain their refusals, parser rejections and
SANY-invalid primary outputs uncredited; 24 source/body/header corruptions keep
the measured Validator dispositions. These are separate selections, not a
claim of 536 unique specifications or full translator coverage.

The earlier 977-test producer passed SANY and Validator but failed 12 strict
syntax-tree comparisons. Its source, binaries and complete failing campaign
remain [blocked development evidence](../conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-before-cst-repair/verification.json).
The repair preserves explicit TRUE for an empty ELSE and encloses CASE values
according to the pinned translator's first-line parenthesis rule. Passing CRCs
or parser acceptance never excuse a changed expression scope.

The runnable Full guide passed 12 fresh CLI checks and its exact Cargo-owned
Rust fence; the current user-manual example passed four profile/policy controls.
All kept their input files unchanged and their nonproof fields false. Historical
BodyOnly, quota, forgery, state-count and witness campaigns remain bound to their
original producers. This publication adds no formatter, filesystem-write,
mapping-format or full-family certification.
'''
path = root / 'docs/pluscal-rewriting.md'
assert '## Current combined publication checks' not in path.read_text()
path.write_text(path.read_text() + text)
text = f'''
## Current pure-name recovery publication

The [combined publication receipt](../{rel}) binds fresh recovery controls to
the same corrected runtime used for Full PlusCal editing. All 195 CLI controls
and the exact Cargo-owned Rust guide pass: uninterrupted search, pause,
repeated recovery, retained counters, TLC-reset counters, zero-budget spill,
worker reporting and concrete witness handling. The 21 safety and 16 validation
gate models agree at 1/4/8 workers. Input checkpoint files remain unchanged
during import; their corruption checksum is not authentication.

TLC runtime-name history still prevents independent witness certification;
canonical-mode supported witnesses independently validate. This increment does
not admit random/effectful checkpoint execution, Java archive interchange,
temporal checkpointing or full recovery-family certification. Earlier broad
campaigns retain their original source and executable hashes rather than being
retagged as results of this combined producer.
'''
path = root / 'docs/native-checkpoint.md'
assert '## Current pure-name recovery publication' not in path.read_text()
path.write_text(path.read_text() + text)
path = root / 'docs/user/storage-recovery.md'
path.write_text(path.read_text() + '''
The [current pure-name publication checks](../native-checkpoint.md#current-pure-name-recovery-publication)
rerun pause/recovery, counter choices, spill cleanup and the executable Rust
example on the combined runtime. They preserve the TLC runtime-name validator's
explicit refusal and do not turn a native archive into an independent proof.
''')
print('Documented fresh combined evidence with historical failures and limitations retained')
