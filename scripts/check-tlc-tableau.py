"""Verify complete pinned tableau selections, raw captures and native source hashes."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROBE = ROOT / 'conformance/probes/tlc-tableau'
PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
FAILURES = ['planning.property-27', 'planning.fairness-27',
            'planning.property-28', 'planning.fairness-28',
            'planning.dnf-integer-overflow', 'direct.empty-conjunction',
            'direct.empty-disjunction', 'direct.action',
            'model.TpActionBad', 'model.TtInstanceNested']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def load(path):
    return json.loads(path.read_text())

def verify(base, count):
    record = load(base / 'capture.json')
    assert record['oracle_sha256'] == PIN
    assert record['compiler']['exit'] == 0
    assert record['reason']
    for path, digest in record['source_sha256'].items():
        assert sha(base / path) == digest, path
    assert record['selection_sha256'] == sha(base / 'selection.json')
    selected = load(base / 'selection.json')
    names = [c['kind'] + '.' + c['name'] for c in selected]
    rows = record['rows']
    assert len(names) == len(rows) == count
    assert len(set(names)) == count
    assert names == [r['name'] for r in rows]
    assert [r['name'] for r in rows if r['exit'] != 0] == FAILURES
    for row in rows:
        assert row['timeout'] is False
        assert row['exit'] in (0, 1)
        for channel in ['stdout', 'stderr']:
            assert row[channel + '_sha256'] == sha(base / 'raw' / row['name'] / channel)
        for path, digest in row['source_sha256'].items():
            assert sha(base / path) == digest, path
        fields = {}
        for line in (base / 'raw' / row['name'] / 'stdout').read_text().splitlines():
            if '\t' in line:
                key, value = line.split('\t', 1)
                assert key not in fields
                fields[key] = value
        assert fields == row['fields']
    assert next(r for r in rows if r['name'] == 'direct.nonpositive-negation')['exit'] == 0
    return selected

def main():
    selected = verify(PROBE, 137)
    historical = verify(PROBE / 'history-initial-136/tlc-tableau', 136)
    assert selected[:136] == historical
    assert selected[-1]['name'] == 'TtVariableInstanceNested'
    original = load(PROBE / '../tlc-planning/cases.json')
    assert [{k: v for k, v in c.items() if k != 'kind'}
            for c in selected if c['kind'] == 'planning'] == original
    original_models = load(PROBE / '../tlc-planning/models.json')
    current_models = [c for c in selected if c['kind'] == 'model']
    assert [c['name'] for c in current_models[:12]] == original_models
    compiler = load(PROBE / 'history-compiler-failure/manifest.json')
    assert compiler['exit'] == 1
    assert 'no controls executed' in compiler['scope']
    for path, digest in compiler['sha256'].items():
        assert sha(PROBE / 'history-compiler-failure' / path) == digest
    assert len(load(PROBE / 'history-compiler-failure/selection.json')) == 136
    matrix = load(PROBE / 'source-matrix.json')
    assert matrix['base_commit'] == '180e9d9'
    for path, digest in matrix['source_sha256'].items():
        assert sha(PROBE / 'historical-before-context-source' / path) == digest, path
    combined = load(PROBE / '../tlc-planning/combined-context-tableau-source-matrix.json')
    assert combined['historical_tableau_matrix_sha256'] == sha(PROBE / 'source-matrix.json')
    for path, digest in combined['source_sha256'].items():
        assert sha(ROOT / path) == digest, path
    print('verified 137 current and 136 historical tableau controls, all ten process failures, complete raw bytes and source hashes')

if __name__ == '__main__':
    main()
