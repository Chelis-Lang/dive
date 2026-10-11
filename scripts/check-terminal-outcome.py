"""Pinned terminal ordering and preserved failures; no ordinary proof credit."""
from pathlib import Path
import gzip
import hashlib
import json

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/terminal-outcome'
load = lambda path: json.loads(path.read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
imports = load(BASE / 'reference-import.json')
assert not imports['model_proof'] and not imports['family_certified']
for path, digest in imports['files'].items():
    assert sha((BASE / path).read_bytes()) == digest, path
expected_counts = {'primary-development0': 84, 'primary-development1': 84,
                   'primary': 84, 'primary-counterposts': 96, 'primary-postorder': 24}
assert imports['rows_by_selection'] == expected_counts
for name, count in expected_counts.items():
    folder = BASE / name
    for path, digest in load(folder / 'closed-index.json')['files'].items():
        assert sha((folder / path).read_bytes()) == digest, (name, path)
    capture = load(folder / 'capture.json')
    selection = load(folder / 'selection.json')
    assert len(capture['rows']) == len(selection) == count
    assert [row['id'] for row in capture['rows']] == [row['id'] for row in selection]
    assert capture['oracle_sha256'] == PIN and not capture['ordinary_native_proof']
    for row in capture['rows']:
        assert not row['timeout']
        for path, digest in row['artifact_sha256'].items():
            assert sha((folder / row['id'] / path).read_bytes()) == digest
rows = {row['id']: row for row in load(BASE / 'primary-postorder/capture.json')['rows']}
contracts = {'safe-state-post': (2405, 0, 10), 'violation-state-post': (2116, 2116, 13),
             'safe-constant-post': (1000, 0, 255)}
for width in [1, 4, 8]:
    for case, (returned, final, cli) in contracts.items():
        observed = rows[f'{case}-w{width}-observed']
        assert observed['exit'] == 0 and rows[f'{case}-w{width}-cli']['exit'] == cli
        events = observed['events']
        assert [event for event in events if event.startswith('modelCheck-return-code:')] == [f'modelCheck-return-code:{returned}']
        assert events.index(f'live-finalCheck-return:{final}') < events.index(f'modelCheck-return-code:{returned}')
    observed = rows[f'violation-constant-post-w{width}-observed']
    assert observed['exit'] == rows[f'violation-constant-post-w{width}-cli']['exit'] == 1
    assert 'live-finalCheck-enter' in observed['events']
    assert not any(event.startswith(('live-finalCheck-return:', 'modelCheck-return-code:')) for event in observed['events'])
supplement = load(BASE / 'boolean-reference-import.json')
assert supplement['rows'] == 36 and not supplement['model_proof'] and not supplement['ordinary_schedule_certified']
for path, digest in supplement['files'].items():
    assert sha((BASE / 'primary-boolean-postorder' / path).read_bytes()) == digest, path
boolean_rows = {row['id']: row for row in load(BASE / 'primary-boolean-postorder/capture.json')['rows']}
assert len(boolean_rows) == 36
for width in [1,4,8]:
    for label, returned in [('true',0),('false',2404),('scalar',2405)]:
        for kind in ['safe','violation']:
            case = f'{kind}-{label}-post-w{width}'
            observed = boolean_rows[case + '-observed']
            expected = 2116 if kind == 'violation' else returned
            assert observed['exit'] == 0 and not observed['timeout']
            assert f'modelCheck-return-code:{expected}' in observed['events']
            assert f'live-finalCheck-return:{2116 if kind == "violation" else 0}' in observed['events']
            assert boolean_rows[case + '-cli']['exit'] == (13 if kind == 'violation' else 0 if returned == 0 else 10)
example = (ROOT / 'crates/dive-api/examples/terminal_classification.rs').read_text().rstrip()
assert '```rust\n' + example + '\n```' in (ROOT / 'docs/terminal-outcome.md').read_text()
matrix = load(BASE / 'source-matrix.json')
assert not matrix['model_proof'] and not matrix['ordinary_schedule_certified'] and not matrix['family_certified']
for path, digest in matrix['source_sha256'].items():
    assert sha((ROOT / path).read_bytes()) == digest, path
history = load(BASE / 'history-before-terminal-outcome/manifest.json')
for path, digest in history['matrices_sha256'].items():
    raw = (BASE / 'history-before-terminal-outcome/matrices' / path).read_bytes()
    assert sha(raw) == digest and json.loads(raw)['source_sha256'] == history['source_by_matrix'][path]
for digest, path in history['source_archive_paths'].items():
    assert sha(gzip.decompress((ROOT / path).read_bytes())) == digest, path
family = next(item for item in load(ROOT / 'conformance/parity-features.json')['families'] if item['id'] == 'F-023')
assert family['implementation_status'] == 'missing' and family['certification'] == 'audit-required' and 228 in family['issues']
print('Complete84/84/84/96/24 terminal reference histories and source-qualified final/post ordering; no ordinary Holds/family proof')
