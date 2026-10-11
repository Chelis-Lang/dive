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
history = BASE / 'before/current-authority'
original = json.loads((history / 'index.json').read_text())
for name, digest in original['changed_source_sha256'].items():
    assert sha(gzip.decompress((history / 'changed-source' / (name + '.gz')).read_bytes())) == digest, name
for row in original['matrices']:
    matrix = history / row['before_matrix']
    assert sha(matrix.read_bytes()) == row['before_sha256'], matrix
    before = json.loads(matrix.read_text())
    for name, digest in row['changed_source_sha256'].items():
        assert before['source_sha256'][name] == digest, name
print('verified clean255 vs unvalidated native12 and exact original changed source/matrix authority; current runtime controls are separate')
