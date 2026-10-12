"""Read the closed 6b portable packet without invoking tools or changing source."""
import gzip
import base64
import hashlib
import json
import re
import tarfile
from pathlib import Path

PRODUCER = '6b4113aa5e781f7c056e7b912b8a0a4bd126c212'
ROOT = '/Users/jeff/code/dive-native-operator-integration-work'
GATE = '/tmp/dive-core-terminal-qualified-epoch'
ORIGINAL = '/tmp/dive-core-terminal-composed-runtime'
RESUME = ORIGINAL + '-resume-6b'
FINAL = ORIGINAL + '-process-resume-6b'
sha = lambda data: hashlib.sha256(data).hexdigest()

def verify(base, expected_index):
    index_bytes = (base / 'index.json').read_bytes()
    assert sha(index_bytes) == expected_index
    index = json.loads(index_bytes)
    assert sha(Path(__file__).read_bytes()) == index['reader_sha256']
    assert index['producer'] == PRODUCER and index['all_consumers_closed'] is True
    assert index['family_certified'] is False
    payload_path = base / 'payload.tar.gz'
    assert sha(payload_path.read_bytes()) == index['payload_sha256']
    with tarfile.open(payload_path, 'r:gz') as archive:
        members = archive.getmembers()
        assert len(members) == len({m.name for m in members})
        assert all(m.isfile() and re.fullmatch(r'payload/[0-9a-f]{64}', m.name) for m in members)
        payload = {m.name[8:]: archive.extractfile(m).read() for m in members}
    assert all(sha(raw) == digest for digest, raw in payload.items())
    paths = index['files']
    assert set(payload) == {row['sha256'] for row in paths.values()}
    def raw(path):
        row = paths[str(path)]
        value = payload[row['sha256']]
        assert len(value) == row['bytes']
        return value
    def doc(path):
        return json.loads(raw(path))
    source = doc(GATE + '/source-index.json')
    supplement = doc(ORIGINAL + '/runtime-source-supplement.json')
    assert source['producer'] == supplement['producer'] == PRODUCER
    assert source['tree'] == supplement['tree'] == index['tree']
    assert len(source['inputs']) == 2713 and len(supplement['inputs']) == 622
    assert not set(source['inputs']) & set(supplement['inputs'])
    admitted = {}
    for parent, selection in [(GATE, source), (ORIGINAL, supplement)]:
        for name, row in selection['inputs'].items():
            value = gzip.decompress(raw(parent + '/' + row['archive']))
            assert sha(value) == row['sha256'], name
            admitted[name] = value
    assert len(admitted) == index['source_inputs'] == 3335
    gate = doc(GATE + '/gate-receipt.json')
    assert gate['producer'] == PRODUCER
    assert gate['source_index_sha256'] == sha(raw(GATE + '/source-index.json'))
    offline = doc(GATE + '/offline-commands.json')
    assert len(offline) == 73 and len(gate['rows']) == 81
    assert [r['command'] for r in gate['rows'][:73]] == offline
    assert [r['name'] for r in gate['rows']] == ['offline-' + str(i) for i in range(73)] + ['stage-' + str(i) for i in range(8)]
    for row in gate['rows']:
        assert row['exit'] == 0 and sha(raw(GATE + '/' + row['name'] + '.log')) == row['log_sha256']
    matched = re.search(rb'(\d+) tests run:\s*(\d+) passed(?: \(\d+ slow\))?, (\d+) skipped', raw(GATE + '/stage-2.log'))
    assert matched and tuple(map(int, matched.groups())) == (1768, 1768, 0)
    closure = doc(FINAL + '/closure.json')
    steps = doc(FINAL + '/steps.json')
    assert closure['producer'] == steps['producer'] == PRODUCER
    assert closure['all_consumers_closed'] is True and closure['family_certified'] is False
    assert closure['source_index_sha256'] == sha(raw(GATE + '/source-index.json'))
    assert closure['steps'] == steps['rows'] and steps['family_certified'] is False
    failed_indexed = ORIGINAL + '-indexed-resume-6b'
    failed_rows = doc(failed_indexed + '/steps.json')['rows']
    assert failed_rows[-1]['name'] == 'process-procedure-models' and failed_rows[-1]['exit'] == 1
    assert all(row['exit'] == 0 for row in failed_rows[:-1])
    fork = index['failed_runtime_fork']
    assert fork['parent'] == failed_indexed and fork['retained_failure_row'] == failed_rows[-1]
    assert fork['omitted_from_receiving_prefix_ordinal'] == len(failed_rows) - 1
    assert fork['new_receiving_prefix'] == failed_rows[:-1]
    for stream in ['stdout', 'stderr']:
        assert sha(raw(failed_indexed + '/process-procedure-models.' + stream)) == failed_rows[-1][stream + '_sha256']
    chain = [ORIGINAL, RESUME, ORIGINAL + '-terminal-resume-6b', failed_indexed, FINAL]
    expected_bindings = []
    previous = []
    for parent in chain:
        receipt_path = parent + '/steps.json'
        receipt = doc(receipt_path)
        assert receipt['producer'] == PRODUCER and receipt['family_certified'] is False
        rows = receipt['rows']
        if parent == failed_indexed:
            rows = rows[:-1]
        assert len(rows) >= len(previous) and rows[:len(previous)] == previous
        for ordinal in range(len(previous), len(rows)):
            row = rows[ordinal]
            expected_bindings.append({'ordinal': ordinal, 'name': row['name'],
                                      'command': row['command'], 'producer_directory': parent,
                                      'steps_receipt': receipt_path,
                                      'streams': {s: parent + '/' + row['name'] + '.' + s
                                                  for s in ['stdout', 'stderr']}})
        previous = rows
    assert previous == steps['rows'] and index['ordered_runtime_bindings'] == expected_bindings
    assert steps['rows'][:len(failed_rows) - 1] == failed_rows[:-1]
    assert [len(doc(parent + '/steps.json')['rows']) for parent in chain[:3]] == [23, 31, 40]
    required = {'release-guides', 'examples-gate', 'validation-smoke-gate'}
    required.update('guide-' + name for name in ['request_owned_service', 'periodic_candidate',
                    'disk_seen_safety', 'disk_seen_replay', 'tuple_only_replay',
                    'inductive_tuple_replay', 'optional_periodic_candidate',
                    'terminal_periodic_candidate', 'stopped_periodic_candidate',
                    'terminal_classification', 'indexed_stack_replay'])
    for family in ['p-elsif', 'multiple-procedure', 'recursive-procedure', 'process-procedure']:
        required.update(family + '-' + stage for stage in
                        ['raw', 'public', 'models', 'replay', 'guides', 'checksum-controls', 'forgery-controls'])
    for child in ['local-declaration-controls', 'ownership-negatives']:
        required.update('multiple-procedure-' + child + '-' + stage for stage in ['raw', 'public'])
    required.update(['process-procedure-nof-materialize', 'process-procedure-no-fairness-raw',
                     'process-procedure-no-fairness-public'])
    assert required <= {r['name'] for r in steps['rows']}
    assert {'process-procedure-full-reference', 'process-procedure-full-raw',
            'process-procedure-full-public'} <= {r['name'] for r in steps['rows']}
    for ordinal, row in enumerate(steps['rows']):
        assert row['exit'] == 0
        for stream in ['stdout', 'stderr']:
            supplied = expected_bindings[ordinal]['streams'][stream]
            assert sha(raw(supplied)) == row[stream + '_sha256']
    for name, digest in closure['binaries'].items():
        assert index['omitted_binaries'][FINAL + '/bins/' + name]['sha256'] == digest
    process_root = ROOT + '/crates/dive-pluscal/process-procedure-checksum-acceptance'
    capture_index = doc(process_root + '/capture-pack-index.json')
    packed = raw(process_root + '/capture-pack.json.gz')
    assert sha(packed) == capture_index['compressed_sha256']
    capture = json.loads(gzip.decompress(packed))
    assert capture['selected'] == 242 and capture['native_credit'] is False and capture['family_certified'] is False
    assert len(capture['records']) == capture_index['record_count'] == len(capture_index['records']) == 12258
    materialized = FINAL + '/process-procedure-complete-reference'
    for name, row in capture['records'].items():
        value = base64.b64decode(row['data'], validate=True)
        assert sha(value) == row['sha256'] == capture_index['records'][name]
        if name.startswith(('cases/', 'primary/', 'exploded/')):
            assert raw(materialized + '/' + name) == value
    late = doc(FINAL + '/process-complete-reference-inputs.json')
    assert late['producer'] == PRODUCER
    for name, row in late['inputs'].items():
        value = raw(process_root + '/' + name)
        assert sha(value) == row['sha256'] and len(value) == row['bytes']
    selection_bytes = raw(process_root + '/model-selection.json')
    assert raw(materialized + '/model-selection.json') == selection_bytes
    selection = json.loads(selection_bytes)
    selected = selection['source_ids']
    assert selection['selected_before_execution'] is True and len(selected) == len(set(selected)) == 154
    full = doc(materialized + '/primary/manifest.json')
    by_id = {r['id']: r for r in full['rows']}
    assert len(full['rows']) == len(by_id) == 242
    assert all(i in by_id and by_id[i]['translated'] and not by_id[i]['reference_errors']
               and by_id[i]['sany']['accepted'] is True for i in selected)
    partial = doc(process_root + '/primary/manifest.json')
    assert len(partial['rows']) == 72
    assert len(set(selected) - {r['id'] for r in partial['rows']}) == 98
    for folder, count in [('process-procedure-full-raw', 242), ('process-procedure-full-public', 484)]:
        report = doc(FINAL + '/' + folder + '/report.json')
        assert report['selected'] == len(report['rows']) == count and report['family_certified'] is False
        assert not report['summary'].get('disagree') and not report['summary'].get('retained-grammar-disagreement')
        assert all(r['outcome'] not in ['disagree', 'retained-grammar-disagreement'] for r in report['rows'])
        if folder.endswith('-public'):
            assert report['model_proved'] is False
            assert {(r['source_id'], r['profile']) for r in report['rows']} == {(i, p) for i in by_id for p in ['tlc', 'dive']}
        else:
            assert {r['id'] for r in report['rows']} == set(by_id)
            assert report['primary_manifest_sha256'][''] == sha(raw(materialized + '/primary/manifest.json'))
    models = doc(FINAL + '/process-procedure-models/report.json')
    assert models['selection_sha256'] == sha(selection_bytes) and models['selected_source_ids'] == selected
    assert models['selected'] == len(models['rows']) == 308 and models['worker_store_controls'] == 3696
    assert models['native_sha256'] == closure['binaries']['dive'] and models['family_certified'] is False
    assert {r['id'] for r in models['rows']} == {i + suffix for i in selected for suffix in ['.complete', '.violation']}
    for row in models['rows']:
        assert row['outcome'] == 'finite-model-match' and len(row['settings']) == 12
        primary_stream = row['primary']['stdout']
        primary = gzip.decompress(raw(FINAL + '/process-procedure-models/' + primary_stream['path']))
        assert sha(primary) == primary_stream['sha256']
        matches = re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.', primary)
        assert len(matches) == 1
        expected = dict(zip(['generated', 'distinct', 'left_on_queue'],
                            [int(value.replace(b',', b'')) for value in matches[0]]))
        depths = re.findall(rb'The depth of the complete state graph search is ([0-9,]+)', primary)
        if depths:
            expected['depth'] = int(depths[0].replace(b',', b''))
        assert row['primary_counts'] == expected
        assert row['primary']['exit_code'] == (12 if row['id'].endswith('.violation') else 0)
        generation = row['generation']['stdout']
        generated_raw = gzip.decompress(raw(FINAL + '/process-procedure-models/' + generation['path']))
        assert sha(generated_raw) == generation['sha256']
        generated = json.loads(generated_raw)
        assert row['generation']['exit_code'] == 0 and generated['accepted'] is True
        assert generated['algorithm_checksum_verified'] is True and generated['model_proved'] is False
        assert {(r['profile'], r['workers'], r['store']) for r in row['settings']} == {
            (p, w, s) for p in ['tlc', 'dive'] for w in [1, 4, 8] for s in ['fp', 'exact']}
        assert all(r['outcome'] == 'match' for r in row['settings'])
        # Typed raw native outputs bind profile-specific exits/verdict/counts.
        for setting in row['settings']:
            saved = setting['capture']
            stream = saved['stdout']
            value = gzip.decompress(raw(FINAL + '/process-procedure-models/' + stream['path']))
            assert sha(value) == stream['sha256']
            actual = json.loads(value)
            violation = row['id'].endswith('.violation')
            assert saved['exit_code'] == ((12 if setting['profile'] == 'tlc' else 10) if violation else 0)
            assert actual['verdict'] == ('invariant-violation' if violation else 'success')
            assert all(actual['statistics'][k] == v for k, v in row['primary_counts'].items())
        for validation in row['independent_validation']:
            saved = validation['capture']
            stream = saved['stdout']
            value = gzip.decompress(raw(FINAL + '/process-procedure-models/' + stream['path']))
            assert sha(value) == stream['sha256']
            actual = json.loads(value)
            if validation['control'] == 'original':
                assert (saved['exit_code'], actual['result'], validation['outcome']) in [
                    (0, 'valid', 'valid'), (3, 'unsupported', 'pending-named-refusal')]
            else:
                assert saved['exit_code'] != 0 and actual['result'] != 'valid' and validation['outcome'] == 'rejected'
    independent = doc('/tmp/dive-terminal-outcome-strict-independent-readback.json')
    assert independent['producer'] == PRODUCER and independent['selected'] == 206
    assert independent['all_remaining_normalized_observables_equal'] is True
    assert independent['inspection_counter_projection'] is False and independent['proof_claims'] is False
    assert independent['negative_selection'] == independent['negative_rejected'] == 13
    for path, digest in independent['artifacts'].items():
        assert sha(raw(path)) == digest
    for item in independent['actual_sources']:
        if item['path'] in admitted:
            value = admitted[item['path']]
        else:
            value = raw(ROOT + '/' + item['path'])
        assert sha(value) == item['sha256'] and item['git_live_equal'] is True
    negative = doc('/tmp/dive-terminal-outcome-strict-negative-controls.json')
    assert len(negative) == 13 and all(r['rejected'] is True for r in negative)
    review = doc('/tmp/dive-terminal-outcome-strict-review-receipt.json')
    assert review['selection'] == len(review['rows']) == 206
    assert review['exact_nonallowlisted_equality'] is True and review['inspection_adjustment'] is False
    assert review['model_proof'] is False and review['ordinary_schedule_certified'] is False
    for row in review['rows']:
        name = row['case'] + '.json'
        prior = ROOT + '/conformance/probes/terminal-outcome/gate-current/runtime-current/native-current/' + name
        assert paths[prior]['group'] == 'late-reader-original-row-inputs'
        assert sha(raw(prior)) == row['prior_sha256']
        assert sha(raw(FINAL + '/terminal-outcome-rows/' + name)) == row['receiving_sha256']
    completion = doc('/tmp/dive-current-completion-contract-receipt.json')
    assert completion['driver_sha256'] == sha(raw('/tmp/dive-current-completion-contract.py'))
    assert completion['output_sha256'] == sha(raw(FINAL + '/capture-completion-contract.json'))
    assert completion['original_failed_driver_sha256'] == sha(raw('/tmp/dive-terminal-outcome-schema-failed/completion-contract-missing-retained-source.py'))
    components = [('optional-periodic-hook', 46, 12, 48), ('terminal-periodic-service', 70, 12, 48),
                  ('stopped-periodic-service', 146, 12, 48), ('terminal-outcome', 206, 28, 112)]
    for name, count, valid, invalid in components:
        contract_path = FINAL + '/' + name + '-receiving-contract.json'
        contract = doc(contract_path)
        assert contract['producer'] == PRODUCER and contract['selected'] == count
        assert contract['public_valid'] == valid and contract['public_invalid'] == invalid
        assert contract['model_proof'] is False and contract['ordinary_schedule_certified'] is False
        assert contract['clock_schedule_equivalence'] is False
        reference = 'conformance/probes/' + name + '/gate-current/runtime-current/'
        selection_bytes = raw(ROOT + '/' + reference + 'selection.json')
        assert paths[ROOT + '/' + reference + 'selection.json']['group'] == 'late-reader-selection-inputs'
        selected = json.loads(selection_bytes)['rows']
        assert contract['original_selection_sha256'] == sha(selection_bytes)
        assert contract['original_runner_sha256'] == sha(admitted[reference + 'runtime-source.rs'])
        cases = {p.rsplit('/', 1)[-1][:-5] for p in paths if p.startswith(FINAL + '/' + name + '-rows/') and p.endswith('.json')}
        assert len(cases) == count and cases == {r['id'] for r in selected}
        widths = set()
        for case in cases:
            row = doc(FINAL + '/' + name + '-rows/' + case + '.json')
            assert row['model_proof'] is False and row['ordinary_schedule_certified'] is False
            widths.add(row['requested_width'])
            prior = doc(ROOT + '/' + reference + 'native-current/' + case + '.json')
            if 'document' in row:
                assert row['document'] == prior['document']
        assert {1, 3, 4, 8} <= widths
        if name != 'terminal-outcome':
            assert raw(contract_path) == raw(RESUME + '/' + name + '-receiving-contract.json')
        actual_valid = actual_invalid = 0
        assert len(contract['commands']) == valid + invalid
        for row in contract['commands']:
            command = row['command']
            assert command[1] == 'validate-trace' and '--compat=tlc' in command
            assert index['omitted_binaries'][command[0]]['sha256'] == closure['binaries']['dive']
            word = command[command.index('--trace') + 1]
            stem = word[:-5]
            assert word.endswith('.json')
            for stream in ['stdout', 'stderr']:
                assert sha(raw(stem + '.' + stream)) == row[stream + '_sha256']
            result = doc(stem + '.stdout')['result']
            if row['exit'] == 0:
                assert result == 'valid'
                actual_valid += 1
            else:
                assert row['exit'] == 1 and result == 'invalid'
                actual_invalid += 1
        assert (actual_valid, actual_invalid) == (valid, invalid)
    for family, count in [('examples', 21), ('validation-smoke', 16)]:
        reports = [p for p in paths if p.startswith(FINAL + '/' + family + '-reports/') and p.endswith('.json')]
        assert len(reports) == 1
        report = doc(reports[0])
        assert report['summary']['models_selected'] == report['summary']['models']['agree'] == count
        assert len(report['models']) == count
        for row in report['models']:
            assert row['outcome'] == 'agree' and {r['workers'] for r in row['runs']} == {1, 4, 8}
            assert len(row['runs']) == 3 and all(r['outcome'] == 'agree' and not r['diffs'] for r in row['runs'])
    for folder, count in [('disk-original-rows', 1024), ('disk-replay-rows', 1040)]:
        receipt = doc(FINAL + '/' + folder + '/receipt.json')
        assert receipt['producer'] == PRODUCER and receipt['family_certified'] is False
        assert receipt['binary_sha256'] == closure['binaries']['dive'] and len(receipt['rows']) == count
        widths = set()
        for row in receipt['rows']:
            cmd = row['command']
            widths.add(int(cmd[cmd.index('--workers') + 1]))
            for stream in ['stdout', 'stderr']:
                assert sha(raw(FINAL + '/' + folder + '/' + row['case'] + '.' + stream)) == row[stream + '_sha256']
        assert {1, 4, 8} <= widths
    witness = doc(FINAL + '/disk-independent-witnesses.stdout')['rows']
    assert len(witness) == 144 and all(r['positive'] == 'valid' for r in witness)
    assert sum(len(r['mutants']) for r in witness) == 864
    assert all(m['validation'] == 'invalid' for r in witness for m in r['mutants'])
    for prefix, count in [('disk-role-current', 2320), ('disk-role-supplemental', 80)]:
        assert len(doc(FINAL + '/' + prefix + '/receipt.json')['rows']) == count
    role = doc(FINAL + '/disk-role-independent.stdout')['rows']
    assert len(role) == 2088
    words = [r for r in role if 'positive' in r]
    assert len(words) == 432 and all(r['positive'] == 'valid' and r['mutants'] for r in words)
    assert all(m['validation'] == 'invalid' for r in words for m in r['mutants'])
    indexed_prior = '/Users/jeff/code/dive-storage-tuple-function-artifacts/indexed-stack-replay/final-qualified/native/census'
    original_rows = doc(indexed_prior + '/receipt.json')['rows']
    assert len(original_rows) == 80
    original_keys = {(r['family'], r['case'], r['profile']): r for r in original_rows}
    assert len(original_keys) == 80
    for prefix, count in [('census', 80), ('width-store-transport', 384), ('process-semantic-mutants', 80)]:
        rows = doc(FINAL + '/indexed-stack/' + prefix + '/receipt.json')['rows']
        assert len(rows) == count
        for row in rows:
            if prefix == 'process-semantic-mutants':
                assert row['exit'] == 1 and row['result']['result'] == 'invalid'
                continue
            assert row['profile'] in ['tlc', 'dive']
            assert row['check_exit'] == (12 if row['profile'] == 'tlc' else 10)
            if prefix == 'census':
                original = original_keys[(row['family'], row['case'], row['profile'])]
                assert row['check_exit'] == original['check_exit']
                assert row['check_verdict'] == original['check_verdict'] == 'invariant-violation'
                stem = row['family'] + '-' + row['case'] + '-' + row['profile']
                current_raw = doc(FINAL + '/indexed-stack/census/' + stem + '.check.stdout')
                prior_raw = doc(indexed_prior + '/' + stem + '.check.stdout')
                for key in ['file', 'config']:
                    assert prior_raw[key].startswith('/Users/jeff/code/dive-storage-tuple-function-artifacts/indexed-stack-replay/before-native/inputs/')
                    prior_raw[key] = current_raw[key]
                current_raw.pop('elapsed_ms')
                prior_raw.pop('elapsed_ms')
                assert current_raw == prior_raw
                assert raw(FINAL + '/indexed-stack/census/' + stem + '.word.json') == raw(indexed_prior + '/' + stem + '.word.json')
            replay = row if prefix == 'census' else row['replays']
            for kind, disposition, exit_code in [('original', 'valid', 0), ('mutant', 'invalid', 1)]:
                assert replay[kind]['exit'] == exit_code and replay[kind]['result']['result'] == disposition
        if prefix == 'width-store-transport':
            assert {r['width'] for r in rows} == {1, 4, 8}
        if prefix == 'census':
            assert {(r['family'], r['case'], r['profile']) for r in rows} == set(original_keys)
    # Failed/interrupted histories are hashed records, never receiving success.
    assert index['historical_groups']
    for group in index['historical_groups']:
        assert group['receiving_success_credit'] is False and group['paths']
        assert all(path in paths for path in group['paths'])
    return {'producer': PRODUCER, 'source_inputs': 3335, 'gate_commands': 81,
            'workspace_tests': 1768, 'runtime_commands': len(steps['rows']),
            'components': 468, 'valid_component_replays': 64, 'invalid_component_replays': 256,
            'family_certified': False}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--index-sha', required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.base, args.index_sha), sort_keys=True))
