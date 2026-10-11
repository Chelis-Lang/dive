"""Pinned terminal ordering and preserved failures; no ordinary proof credit."""
from pathlib import Path
import gzip
import hashlib
import json
import copy

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
def verify_original_inputs(before, index):
    """Anchor historical gzip inputs to original commit/tree paths, not live code."""
    proof_root = BASE / 'original-396-git-proof'
    proof = load(proof_root / 'proof.json')
    expected = '3968821ee1dedf9a486896443a16f3af7e348570'
    assert before['producer'] == index['production_commit'] == proof['producer'] == expected
    assert proof['source_inputs'] == len(before['source_sha256']) == 491
    assert index['source_sha256'] == before['source_sha256']
    assert set(index['source_git_blob_sha1']) == set(before['source_sha256'])
    trees = {}
    def original_object(oid, kind):
        assert len(oid) == 40 and all(c in '0123456789abcdef' for c in oid)
        record = proof['objects'][oid]
        assert record['kind'] == kind and record['file'] == f'objects/{oid}.gz'
        raw = gzip.decompress((proof_root / record['file']).read_bytes())
        assert sha(raw) == record['sha256']
        assert hashlib.sha1(f'{kind} {len(raw)}'.encode() + b'\0' + raw).hexdigest() == oid
        return raw
    commit = original_object(expected, 'commit')
    root = commit.splitlines()[0].decode().removeprefix('tree ')
    assert root == proof['root_tree']
    def children(oid):
        if oid not in trees:
            raw = original_object(oid, 'tree')
            start = 0
            entries = {}
            while start < len(raw):
                sep = raw.index(b' ', start)
                end = raw.index(b'\0', sep)
                mode, name = raw[start:sep].decode(), raw[sep+1:end].decode()
                assert end+21 <= len(raw) and name not in entries
                entries[name] = mode, raw[end+1:end+21].hex()
                start = end+21
            trees[oid] = entries
        return trees[oid]
    for path, digest in before['source_sha256'].items():
        parts = path.split('/')
        assert parts and all(part not in ['', '.', '..'] for part in parts)
        node = root
        for part in parts[:-1]:
            mode, node = children(node)[part]
            assert mode == '40000'
        mode, blob = children(node)[parts[-1]]
        assert mode in ['100644', '100755'] and blob == index['source_git_blob_sha1'][path]
        raw = gzip.decompress((ROOT / index['source_archive_paths'][path]).read_bytes())
        assert sha(raw) == digest
        assert hashlib.sha1(f'blob {len(raw)}'.encode() + b'\0' + raw).hexdigest() == blob

def check_closed_outcomes(gate, steps, log, manifest, runtime, selection, native):
    """Check actual closed commands and semantic words, never proof promotion."""
    assert gate['all_steps_passed'] is True and gate['build_inputs_unchanged'] is True
    assert gate['expected_steps'] == len(steps) == 49
    assert all(step['exit'] == 0 for step in steps)
    assert '1100 tests run: 1100 passed, 0 skipped' in log
    assert runtime['source_and_binaries_unchanged'] is True
    assert len(runtime['steps']) == 21 and all(step['exit'] == 0 for step in runtime['steps'])
    assert len(manifest['rows']) == len(selection['rows']) == len(native) == 206
    assert len({row['file'] for row in manifest['rows']}) == 206
    assert len({row['id'] for row in selection['rows']}) == 206
    assert {row['file'].removesuffix('.json') for row in manifest['rows']} == {row['id'] for row in selection['rows']}
    valid = []
    for row in native:
        assert row['model_proof'] is False and row['ordinary_schedule_certified'] is False
        assert row.get('terminal_classification') != 'Holds'
        if 'memory_validation' in row:
            assert row['memory_validation'].startswith('Report { validation: Valid,')
            assert row['json_validation'].startswith('Report { validation: Valid,')
            assert len(row['mutants']) == 4
            for mutant in row['mutants']:
                assert mutant['memory_validation'].startswith('Report { validation: Invalid(')
                assert mutant['json_validation'].startswith('Report { validation: Invalid(')
            valid.append(row)
    assert len(valid) == 28 and sum(len(row['mutants']) for row in valid) == 112
    post = {(row['kind'], row['requested_width']): row for row in native if row['kind'].startswith('post-')}
    assert len(post) == 60
    for width in [1, 3, 4, 8]:
        assert post['post-true-safe', width]['terminal_classification'] == 'FinalSelectionsComplete'
        for kind, code in [('post-final-error', 2405), ('post-cached-pass', 1000),
                           ('post-false-safe', 2404), ('post-scalar-safe', 2405)]:
            assert post[kind, width]['terminal_classification'] == f'PostconditionFailure {{ code: Some({code}) }}'
        for kind in ['post-candidate', 'post-true-violation', 'post-false-violation', 'post-scalar-violation']:
            assert post[kind, width]['terminal_classification'] == 'EofWitness'
        for kind in ['post-cached-candidate', 'post-budget', 'post-init', 'post-unbounded']:
            assert post[kind, width]['terminal_classification'] == 'Incomplete'
            assert 'memory_validation' not in post[kind, width]
        assert post['post-source', width]['terminal_classification'].startswith('OrderedSourceStop')
        assert post['post-effect', width]['execution_retained'] is False


delivery = BASE / 'delivery-index.json'
if delivery.exists():
    index = load(delivery)
    assert index['model_proof'] is False and index['ordinary_schedule_certified'] is False and index['family_certified'] is False
    for path, digest in index['artifact_sha256'].items():
        assert sha((ROOT / path).read_bytes()) == digest, path
    gate = BASE / 'gate-current'
    runtime = gate / 'runtime-current'
    before = load(gate / 'before.json')
    assert before['producer'] == index['production_commit'] and len(before['source_sha256']) == 491
    assert len(before['literal_include_sources']) == 50 and not before['unresolved_include_macros']
    verify_original_inputs(before, index)
    native_manifest = load(runtime / 'native-manifest.json')
    native = []
    for row in native_manifest['rows']:
        raw = (runtime / 'native-current' / row['file']).read_bytes()
        assert sha(raw) == row['sha256']
        native.append(json.loads(raw))
    closed = [load(gate / 'after.json'), load(gate / 'steps.json'),
              (gate / 'workspace.log').read_text(), native_manifest,
              load(runtime / 'runtime-closed.json'), load(runtime / 'selection.json'), native]
    check_closed_outcomes(*closed)
    # Corrupt semantic facts even when counts and an observer's success flag
    # otherwise look plausible. These are reader controls, not checker verdicts.
    def rejection(change):
        altered = copy.deepcopy(closed)
        change(altered)
        try:
            check_closed_outcomes(*altered)
        except AssertionError:
            return
        raise AssertionError('corrupted closed evidence was accepted')

    rejection(lambda x: x[0].update(all_steps_passed=False))
    rejection(lambda x: x[1][0].update(exit=1))
    rejection(lambda x: x[1].pop())
    rejection(lambda x: x.__setitem__(2, x[2].replace('1100 passed, 0 skipped', '1099 passed, 1 skipped')))
    rejection(lambda x: x[4]['steps'][0].update(exit=1))
    rejection(lambda x: x[4]['steps'].pop())
    rejection(lambda x: x[6][0].update(model_proof=True))
    rejection(lambda x: x[6][0].update(terminal_classification='Holds'))
    rejection(lambda x: x[5]['rows'][-1].update(id=x[5]['rows'][0]['id']))
    rejection(lambda x: x[6].pop())
    word_index = next(i for i, row in enumerate(native) if 'memory_validation' in row)
    rejection(lambda x: x[6][word_index]['mutants'][0].update(memory_validation='Report { validation: Valid, notes: [], length: 2 }'))
    refusal_index = next(i for i, row in enumerate(native) if row['kind'] == 'post-cached-candidate')
    rejection(lambda x: x[6][refusal_index].update(terminal_classification='FinalSelectionsComplete'))
    for corpus, count in [('examples', 21), ('validation-smoke', 16)]:
        reports = list((runtime / (corpus + '-reports')).glob('*.json'))
        assert len(reports) == 1
        report = load(reports[0])
        assert len(report['models']) == count and all(row['outcome'] == 'agree' for row in report['models'])
        assert all([run['workers'] for run in row['runs']] == [1, 4, 8] for row in report['models'])
    failed = load(BASE / 'development-enospc-fullgate/failed-epoch-index.json')
    assert failed['successful_stages'] == 43 and failed['stage_count'] == 44 and failed['all_steps_passed'] is False
    assert failed['original_after_json_missing'] is True
    frozen_maps = load(BASE / 'history-production-396/manifest.json')
    assert frozen_maps['producer'] == index['production_commit']
    for path, digest in frozen_maps['matrices_sha256'].items():
        raw = (BASE / 'history-production-396/matrices' / path).read_bytes()
        assert sha(raw) == digest and json.loads(raw)['source_sha256'] == frozen_maps['source_by_matrix'][path]
    for digest, path in frozen_maps['source_archive_paths'].items():
        assert sha(gzip.decompress((ROOT / path).read_bytes())) == digest
print('Complete terminal reference histories, source-qualified final/post ordering and closed native receipts; no ordinary Holds/family proof')
