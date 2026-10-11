"""Verify immutable pure VIEW normalization evidence and changed source history."""
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/view-concrete-normalization'
sha = lambda b: hashlib.sha256(b).hexdigest()
record = json.loads((BASE / 'before/index.json').read_text())
assert record['oracle_source_commit'] == '94d0c5070852b37b801bb8998c436383ab7ee37b'
assert record['native_source_commit'] == '2c2ebda3940109fae34a033749b63a8560f8834c'
for name, digest in record['files_sha256'].items():
    assert sha((BASE / 'before' / name).read_bytes()) == digest, name
reference = (BASE / 'before/reverse-mixed/tlc.stdout').read_text()
assert 'STARTMSG 1000:1' in reference and 'Attempted to compare record' in reference
native = json.loads((BASE / 'before/reverse-mixed/native.stdout').read_text())
assert native['verdict'] == 'invariant-violation'
assert native['statistics']['distinct'] == 4
assert native['counterexample']['trace'] is None
lead = json.loads((BASE / 'before/main-equivalent-550d959/readback.json').read_text())
assert lead['key_function_source_equal'] is True and lead['reference_exit'] == 255
assert [row['workers'] for row in lead['rows']] == [1, 4, 8]
for row in lead['rows']:
    assert row['exit'] == 12 and row['counterexample_validation'] is None
for directory, parent in (
    ('current-authority', 'd67e8d76388c050d8928533ca3b2d7d5572f6733'),
    ('fe35-current-authority', 'fe35c73f5c5d658a402dd41f264e43bb7887a591'),
    ('15fda-current-authority', '15fda58d9f3e8a9a427428af2a92eac3812a5258'),
):
    history = BASE / 'before' / directory
    original = json.loads((history / 'index.json').read_text())
    assert original['parent_commit'] == parent
    for name, digest in original['changed_source_sha256'].items():
        assert sha(gzip.decompress((history / 'changed-source' / (name + '.gz')).read_bytes())) == digest, name
    for row in original['matrices']:
        matrix = history / row['before_matrix']
        assert sha(matrix.read_bytes()) == row['before_sha256'], matrix
        before = json.loads(matrix.read_text())
        for name, digest in row['changed_source_sha256'].items():
            assert before['source_sha256'][name] == digest, name
failed = BASE / 'before/rejected-lazy-gate'
epoch = json.loads((failed / 'index.json').read_text())
assert epoch['source_commit'] == '15fda58d9f3e8a9a427428af2a92eac3812a5258'
for name, digest in epoch['files_sha256'].items():
    assert sha((failed / name).read_bytes()) == digest, name
results = json.loads((failed / 'results.json').read_text())
assert len(results['stages']) == 8
assert [row['stage'] for row in results['stages'] if row['exit']] == ['nextest']
log = gzip.decompress((failed / '2-nextest.log.gz').read_bytes()).decode()
assert '1103 tests run: 1102 passed (2 slow), 1 failed, 0 skipped' in log
assert 'empty_frontier_and_deferred_view_admission' in log
old_test = BASE / 'before/15fda-current-authority/changed-source/crates/dive-check/tests/frontier_spill.rs.gz'
assert gzip.decompress(old_test.read_bytes()) == (ROOT / 'crates/dive-check/tests/frontier_spill.rs').read_bytes()
lazy = BASE / 'lazy-primary'
capture = json.loads((lazy / 'capture.json').read_text())
assert capture['oracle_source_commit'] == '94d0c5070852b37b801bb8998c436383ab7ee37b'
assert capture['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert [row['model'] for row in capture['rows']] == ['filter', 'cup', 'cap', 'diff', 'union', 'nested']
for row in capture['rows']:
    assert row['exit'] == 0
    for name, digest in row['files_sha256'].items():
        assert sha((lazy / row['model'] / name).read_bytes()) == digest, name
    output = (lazy / row['model'] / 'stdout').read_text()
    assert '2 states generated, 1 distinct states found' in output
    assert 'The depth of the complete state graph search is 1' in output
for name, digest in capture['source_sha256'].items():
    assert sha((lazy / 'java-source' / name).read_bytes()) == digest, name
for name, exit_code in (('nested-filter', 0), ('filter-function-range', 0), ('filter-invalid-range', 255)):
    extra = lazy / name
    capture = json.loads((extra / 'capture.json').read_text())
    assert capture['oracle_source_commit'] == '94d0c5070852b37b801bb8998c436383ab7ee37b'
    assert capture['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
    assert capture['exit'] == exit_code
    for path, digest in capture['files_sha256'].items():
        assert sha((extra / path).read_bytes()) == digest, path
    output = (extra / 'stdout').read_text()
    if exit_code:
        assert 'STARTMSG 1000:1' in output
    else:
        assert '2 states generated, 1 distinct states found' in output
        assert 'The depth of the complete state graph search is 1' in output
for directory, codes in (
    ('incomplete-primary', (1000, 2109)),
    ('incomplete-wellformed-primary', (2102, 2109)),
):
    capture = json.loads((BASE / directory / 'capture.json').read_text())
    assert capture['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
    assert [row['exit'] for row in capture['rows']] == [255, 75]
    assert len(capture['rows']) == len(codes)
    for row, code in zip(capture['rows'], codes):
        location = BASE / directory / ('init' if row['initial'] else 'next')
        for name, field in (
            ('IncompleteView.tla', 'source_sha256'),
            ('IncompleteView.cfg', 'config_sha256'),
            ('stdout', 'stdout_sha256'),
            ('stderr', 'stderr_sha256'),
        ):
            assert sha((location / name).read_bytes()) == row[field]
        assert f'STARTMSG {code}:1' in (location / 'stdout').read_text()
print('verified clean255 vs unvalidated native12 and exact original changed source/matrix authority; current runtime controls are separate')
