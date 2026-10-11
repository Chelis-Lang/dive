from pathlib import Path
import json

root = Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work')
receipt = json.loads((root / 'conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-publication/verification.json').read_text())
assert receipt['rust_tests'] == 982 and receipt['family_certified'] is False
body = f'''Bounded `dive translate --rewrite` now derives TLC's algorithm AST and generated-body checksums for supported sequential shapes, including nested `elsif`/`with`. Generation preserves empty-ELSE and CASE expression scope. Requests return an analyzed replacement in memory; unproved shapes and filesystem writes explicitly refuse. Pure runtime-string constructors also support pause/recovery with restored encounter order, retained or TLC-reset counters and transient spill cleanup.

Update executable CLI/Rust guides, the current user manual, design/provenance notes and partial-feature tracking. Preserve the previous producer's 12 complete-CST mismatches as failed evidence; passing parser/checksum checks never excuse changed expression scope. TLC runtime-name witness validation remains explicitly unsupported.

Validation and affected conformance entries:

- Full local gate: 982/982 tests, zero skipped, all eight stages; four actual CI partitions are complete and disjoint.
- Complete Full selections: 216 base rows and 320 nested/layout rows, both profiles; {receipt['full_base_complete_cst_pairs']} + 172 admitted SANY/Validator/whole-CST pairs, 24 corruption controls; every refusal, rejection and invalid-primary result retained.
- Fresh runtime-name recovery: 195 CLI controls and exact Cargo-owned Rust guide. Full editing guide: 12 CLI controls and exact Cargo-owned Rust guide; four current-manual profile/policy controls leave input unchanged.
- Required CM1 (21) and ValidationTestSuite smoke (16) models fully agree at 1/4/8 workers; smoke also passes the unchanged 30-second CI bound.

The source-bound receipt records frozen runtime b8fb13cd and immutable binaries. Historical campaigns retain their own producer hashes. No whole-corpus rerun, formatter/mapping parity, archive certificate or full-family certification is claimed.
'''
Path('/tmp/dive-final-full-runtime-pr-body.md').write_text(body)
print('Prepared concrete PR description from verified receipt')
