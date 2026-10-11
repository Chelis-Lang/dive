"""Verify the closed complete successor census without promoting other epochs."""
import collections
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/studies/stored-successor-final-small'
PRODUCER = ROOT / 'conformance/probes/stored-successor-material'


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


closed = load(BASE / 'closed-after.json')
before = load(BASE / 'before.json')
after = load(BASE / 'after.json')
assert closed['production_commit'] == 'c8114616825629d2f54e3d3c7ec224fc1ced91da'
assert closed['source_commit'] == before['source_commit'] == '29c7a20ff63fe3a8782451862cb0520c8eff19ee'
assert closed['harness_exit'] == after['harness_exit'] == 1
assert after['source_and_binaries_unchanged'] is True
assert before['selection'] == dict(corpus='examples', tier='small', filters=[], workers=[1], timeout_seconds=600, jobs=1)
assert before['implementation'] == 'dive-final-tlc'
assert closed['selected'] == 140 and closed['raw_invocations'] == 138
assert len(closed['artifact_sha256']) == 556
for path, digest in closed['artifact_sha256'].items():
    assert sha(BASE / path) == digest, path
assert closed['source_sha256'] == before['source_sha256']
assert closed['binary_sha256'] == before['binary_sha256']
manifest = load(PRODUCER / 'source-manifest.json')
assert manifest['commit'] == closed['production_commit']
assert manifest['source_sha256'] == before['source_sha256']
assert len(before['source_sha256']) == 389
for path, digest in before['source_sha256'].items():
    raw = gzip.decompress((PRODUCER / 'source' / (path + '.gz')).read_bytes())
    assert hashlib.sha256(raw).hexdigest() == digest, path
reports = list((BASE / 'reports').glob('*.json'))
assert len(reports) == 1
report = load(reports[0])
rows = report['models']
assert len(rows) == 140 and len({(r['corpus'], r['model']) for r in rows}) == 140
counts = collections.Counter(r['outcome'] for r in rows)
assert counts == dict(agree=60, unsupported=75, timeout=5)
assert report['summary']['models'] == closed['summary']
for row in rows:
    if row['outcome'] == 'agree':
        assert row['runs'] and all(r['outcome'] == 'agree' and not r['diffs'] for r in row['runs'])
    if row['outcome'] == 'timeout':
        assert row['runs'] and all(r['exit_code'] is None for r in row['runs'])
invocations = list((BASE / 'raw').glob('*/invocation.json'))
assert len(invocations) == 138
for path in invocations:
    invocation = load(path)
    assert invocation['real_binary_sha256'] == before['binary_sha256']['dive'], path
comparison = load(BASE / 'model-by-model.json')
assert comparison['selected'] == len(comparison['models']) == 140
assert comparison['current_report_sha256'] == sha(reports[0])
assert comparison['transitions'] == {
    'agree -> agree': 53, 'unsupported -> agree': 7,
    'unsupported -> unsupported': 75, 'unsupported -> timeout': 3,
    'timeout -> timeout': 2,
}
assert collections.Counter(r['prior_outcome'] + ' -> ' + r['current_outcome'] for r in comparison['models']) == comparison['transitions']
verification = load(BASE / 'independent-readback.json')
assert verification['closed_receipt_sha256'] == sha(BASE / 'closed-after.json')
assert verification['artifacts_verified'] == 556 and verification['source_inputs'] == 389
print('Verified complete immutable c811140 census:60 agree/75 unsupported/5 timeout/0 disagree;138 raw invocations; no ordinary/family credit')
