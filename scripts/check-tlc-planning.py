"""Verify complete pinned planning selections, raw captures and native source hashes."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROBE = ROOT / 'conformance/probes/tlc-planning'
PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def load(name):
    return json.loads((PROBE / name).read_text())

def verify_capture(filename, source, selection, raw, count, snapshot=None):
    base = PROBE if snapshot is None else PROBE / snapshot
    record = json.loads((base / filename).read_text())
    assert record['oracle_sha256'] == PIN
    assert record['source_sha256'] == sha(base / source)
    selection_key = 'selection_sha256' if 'selection_sha256' in record else 'cases_sha256'
    assert record[selection_key] == sha(base / selection)
    selected = json.loads((base / selection).read_text())
    names = [s['name'] if isinstance(s, dict) else s for s in selected]
    rows = record['rows']
    assert len(names) == len(rows) == count
    assert names == [r['name'] for r in rows]
    assert len(names) == len(set(names))
    for row in rows:
        path = base / raw / row['name']
        assert row['stdout_sha256'] == sha(path / 'stdout')
        assert row['stderr_sha256'] == sha(path / 'stderr')
        assert row['timeout'] is False
        if 'source_sha256' in row:
            for source_path, digest in row['source_sha256'].items():
                assert sha(base / source_path) == digest
    return rows

def main():
    rows = verify_capture('capture.json', 'PlanningProbe.java', 'cases.json', 'raw', 84)
    failures = [r for r in rows if r['exit'] != 0]
    assert [r['name'] for r in failures] == ['dnf-integer-overflow']
    assert failures[0]['exit'] == 1
    assert 'maximum supported size in disjunctive normal form' in (PROBE / 'raw/dnf-integer-overflow/stderr').read_text()
    models = verify_capture('model-capture.json', 'PlanningProbe.java', 'models.json', 'model-raw', 12)
    assert [r['name'] for r in models if r['exit'] != 0] == ['TpActionBad']
    assert '[] followed by action not of form [A]_v.' in (PROBE / 'model-raw/TpActionBad/stderr').read_text()
    for row in models:
        for key, value in row['fields'].items():
            if key.startswith('actual.') and not key.endswith(('.promises', '.tableauNodes')):
                assert row['fields'][key.removeprefix('actual.')] == value
    # Preserve the initial development-adapter defect and its complete raw
    # selection. It is historical adapter evidence, not accepted oracle credit.
    old = verify_capture('development-capture-empty-fairness-bug.json',
                         'development-PlanningProbe-empty-fairness-bug.java',
                         'development-cases-empty-fairness-bug.json',
                         'development-raw-empty-fairness-bug', 80)
    assert all(r['exit'] == 0 for r in old)
    assert next(r for r in old if r['name'] == 'fairness-34')['fields'] == {'empty': 'true'}
    historical_models = verify_capture('model-capture.json', 'PlanningProbe.java',
                                       'models.json', 'model-raw', 12,
                                       'historical-pre-whitespace-models')
    assert [r['name'] for r in historical_models if r['exit'] != 0] == ['TpActionBad']
    before = load('historical-before-tableau-source-matrix.json')
    assert before['base_commit'] == '0b02e46'
    for path, digest in before['source_sha256'].items():
        assert sha(PROBE / 'historical-before-tableau-source' / path) == digest
    matrix = load('source-matrix.json')
    assert matrix['base_commit'] == '0b02e46'
    for path, digest in matrix['source_sha256'].items():
        assert sha(ROOT / path) == digest
    print('verified 84 projection controls, 12 actual models, 80 historical adapter controls and 12 historical pre-whitespace models')

if __name__ == '__main__':
    main()
