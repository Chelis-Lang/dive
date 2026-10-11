#!/usr/bin/env python3
"""Portable integrity/readback for positioned replay; hashes are not certificates."""
import argparse
import copy
import gzip
import hashlib
import io
import itertools
import json
import pathlib
import posixpath
import re

MAX_BYTES = 64 * 1024 * 1024
PRODUCER = '881b75992d0ef5dc11539018da526e5e55066027'
ALLOWLIST = ['elapsed_ms', 'options.workers', 'options.seen_disk', 'seen_storage']
PURE = {'ReplayMembership', 'ReplayNew', 'ReplayLaterViolation', 'ErInvDivZero',
        'ErInvReplayViolation', 'ErInvReplayError'}
PRINT = {'ReplayPrint', 'ReplayPrintInit', 'ReplayPrintEmpty', 'ReplayPrintInvariant',
         'ReplayPrintWrapper', 'ReplayPrintAlias', 'ReplayPrintReplayOnly'}

def safe(base, name):
    parts = pathlib.PurePosixPath(name)
    assert not parts.is_absolute() and parts.parts
    assert all(p not in ('.', '..') for p in parts.parts)
    result = base.joinpath(*parts.parts)
    assert not result.is_symlink() and result.resolve().is_relative_to(base.resolve())
    return result

def read(path):
    assert path.stat().st_size <= MAX_BYTES, path
    return path.read_bytes()

def sha(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    return json.loads(read(path))

def unpack(path):
    with gzip.GzipFile(fileobj=io.BytesIO(read(path))) as stream:
        data = stream.read(MAX_BYTES + 1)
    assert len(data) <= MAX_BYTES, path
    return data

def stable(value, disk=False, workers=False):
    value = copy.deepcopy(value)
    value.pop('elapsed_ms')
    if disk:
        value.pop('seen_storage')
        value['options'].pop('seen_disk')
    if workers:
        value['options'].pop('workers')
    return value

def verify_rows(base, directory, receipt):
    rows = receipt['rows']
    assert len({r['case'] for r in rows}) == len(rows)
    reports = {}
    for row in rows:
        for channel in ('stdout', 'stderr'):
            assert sha(read(safe(base, f"{directory}/{row['case']}.{channel}"))) == row[channel + '_sha256']
        reports[row['case']] = load(safe(base, f"{directory}/{row['case']}.stdout"))
    return reports

def verify(base, binary_root=None, source_root=None):
    index = load(base / 'delivery-index.json')
    assert index['schema_version'] == 1 and index['family_certified'] is False
    assert index['runtime_commit'] == PRODUCER
    assert sha(read(pathlib.Path(__file__))) == index['verifier_sha256']
    expected = {p.relative_to(base).as_posix() for p in base.rglob('*')
                if p.is_file() and p.name != 'delivery-index.json'}
    assert expected == set(index['payload_sha256'])
    for name, digest in index['payload_sha256'].items():
        assert sha(read(safe(base, name))) == digest, name
    source = load(safe(base, index['source_index']))
    assert source['producer'] == PRODUCER and len(source['inputs']) == 1226
    for name, row in source['inputs'].items():
        data = unpack(safe(base, row['archive']))
        assert sha(data) == row['sha256'], name
        if source_root is not None:
            assert read(safe(source_root, name)) == data, name
    supplement = load(base / 'embedded-input-supplement/index.json')
    assert supplement['producer'] == PRODUCER and supplement['family_certified'] is False
    assert supplement['original_source_index_sha256'] == sha(read(base / 'source-index.json'))
    compiled = {name for name in source['inputs'] if name.endswith('.rs') and
                (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/', name) or
                 name.startswith('xtask/src/') or re.match(r'^crates/[^/]+/build\.rs$', name))}
    assert supplement['compiled_rust_source_sha256'] == {n: source['inputs'][n]['sha256'] for n in compiled}
    references = []
    for name in sorted(compiled):
        text = unpack(safe(base, source['inputs'][name]['archive'])).decode()
        matches = list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)', text))
        assert len(matches) == len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(', text)))
        for match in matches:
            target = posixpath.normpath(str(pathlib.Path(name).parent / match[2]))
            data = unpack(safe(base, 'embedded-input-supplement/inputs/' + target + '.gz'))
            if source_root is not None:
                assert read(safe(source_root, target)) == data
            references.append({'source': name, 'macro': match[1], 'literal': match[2],
                               'target': target, 'sha256': sha(data)})
    assert references == supplement['references'] and len(references) == 61
    assert supplement['input_sha256'] == {r['target']: r['sha256'] for r in references}
    assert set(supplement['original_source_map_missing']) == set(supplement['input_sha256']) - set(source['inputs'])
    assert len(supplement['original_source_map_missing']) == 55 and supplement['dynamic_includes'] == 0
    gate = load(safe(base, index['gate_receipt']))
    assert gate['producer'] == PRODUCER
    assert sha(read(safe(base, index['source_index']))) == gate['source_index_sha256']
    assert [r['stage'] for r in gate['rows']] == list(range(8))
    assert all(r['exit'] == 0 for r in gate['rows'])
    assert gate['jobs'] == gate['test_threads'] == 2 and gate['profile'] == 'ci'
    assert gate['per_test_timeout_unchanged'] is True
    assert re.search(r'1284 tests run: 1284 passed, 0 skipped', read(base / 'gate/stage-2.log').decode())
    failed = load(safe(base, index['failed_gate_receipt']))
    assert failed['producer'] == PRODUCER and failed['rows'][0]['exit'] != 0
    assert 'No space left on device' in read(base / 'failed-enospc-gate/stage-0.log').decode()
    runtime = load(safe(base, index['original_runtime_receipt']))
    assert runtime['producer'] == PRODUCER and runtime['metadata_allowlist'] == ALLOWLIST
    assert (runtime['memory_selected'], runtime['disk_selected'], runtime['reference_selected']) == (384, 576, 64)
    assert len(runtime['rows']) == 1024 and len(runtime['primary']) == 8
    assert next(r for r in runtime['primary'] if r['case'] == 'DeterministicError')['native_error_class_verified'] is False
    reports = verify_rows(base, 'runtime', runtime)
    originals = {r['case']: r for r in load(base / 'memory-baseline/receipt.json')['rows']}
    for row in runtime['rows']:
        name = row['case']; actual = reports[name]
        if name.startswith('memory-'):
            old_name = name.removeprefix('memory-')
            assert row['exit'] == originals[old_name]['exit']
            assert stable(actual) == stable(load(safe(base, 'memory-baseline/' + old_name + '.stdout')))
        elif name.startswith('disk-'):
            _, model, profile, store, width, chaos, budget = name.split('-')
            reference = reports[f'reference-{model}-{profile}-{store}-{budget}']
            assert actual['options']['workers']['requested'] == int(width)
            assert actual['options']['workers']['used'] == 1 and actual['options']['workers']['sequential'] is True
            seen = actual['seen_storage']
            assert seen['first_failure'] is None and seen['after_cleanup']['cleanup_completed'] is True
            assert seen['admitted_keys'] == actual['statistics']['distinct'] == seen['before_cleanup']['physical_entries']
            assert seen['before_cleanup']['resident_key_collection_bytes'] == 0
            assert stable(actual, True, True) == stable(reference, workers=True)
    replay = load(safe(base, index['replay_runtime_receipt']))
    assert replay['producer'] == PRODUCER and replay['metadata_allowlist'] == ALLOWLIST
    assert (replay['reference_selected'], replay['pure_disk_selected'], replay['print_refusal_selected']) == (104, 432, 504)
    assert len(replay['rows']) == 1040 and len(replay['primary']) == 13
    data = verify_rows(base, 'replay-runtime', replay)
    disk_selection = set()
    reference_selection = set()
    for row in replay['rows']:
        name = row['case']; actual = data[name]
        if name.startswith('memory-'):
            _, model, profile, store, budget = name.split('-')
            reference_selection.add((model, profile, store, budget))
            continue
        _, model, profile, store, width, chaos, budget = name.split('-')
        disk_selection.add((model, profile, store, width, chaos, budget))
        reference = data[f'memory-{model}-{profile}-{store}-{budget}']
        workers = actual['options']['workers']; seen = actual['seen_storage']
        assert workers['requested'] == int(width) and workers['used'] == 1 and workers['sequential'] is True
        assert seen['first_failure'] is None and seen['after_cleanup']['cleanup_completed'] is True
        if model in PRINT:
            assert row['exit'] == 3 and actual['verdict'] == 'unsupported'
            assert 'Print/PrintT' in actual['message']
            assert actual['tlc'] is None and actual['counterexample'] is None
            assert actual['statistics'] == reference['statistics']
            facts = seen['replay']
            assert facts['unproved_print_output'] is True and 'EvaluationError' in facts['original_error']
            assert 'Unsupported' in facts['halted_by'] and facts['generated'] == 2
            assert facts['admitted_keys'] == facts['queued'] == 2 and seen['admitted_keys'] == 4
        else:
            assert model in PURE and stable(actual, True, True) == stable(reference, workers=True)
            if seen['replay'] is not None:
                assert seen['replay']['unproved_print_output'] is False
    assert disk_selection == set(itertools.product(PURE | PRINT, ['tlc', 'dive'], ['fp', 'exact'], ['1', '4', '8'], ['None', '0', '7'], ['False', 'True']))
    assert reference_selection == set(itertools.product(PURE | PRINT, ['tlc', 'dive'], ['fp', 'exact'], ['False', 'True']))
    proofs = load(safe(base, index['witness_receipt']))
    assert proofs['runtime_source_commit'] == PRODUCER
    assert (proofs['rows'], proofs['positive_valid'], proofs['semantic_mutants_invalid'], proofs['actual_view_rows']) == (144, 144, 864, 36)
    controls = load(base / 'api-controls/stdout.json')['rows']
    original_controls = load(base / 'api-controls/original-stdout.json')['rows']
    key = lambda r: (r['case'], r['compat_tlc'], r['store'], r['requested_workers'], r['chaos'])
    old = {key(r): r['mutants'] for r in original_controls}
    assert len(old) == len(controls) == 144 and set(old) == {key(r) for r in controls}
    assert sum(len(r['mutants']) for r in controls) == 864
    for row in controls:
        assert row['positive'] == 'valid' and row['mutants'] == old[key(row)]
        assert all(m['validation'] == 'invalid' for m in row['mutants'])
        assert row['report']['options']['workers']['used'] == 1
        assert row['report']['seen_storage']['after_cleanup']['cleanup_completed'] is True
    faults = load(base / 'positioned-faults.receipt.json')
    assert faults['producer'] == PRODUCER and faults['exit'] == 0
    assert re.search(r'15 tests run: 15 passed, 141 skipped', read(base / 'positioned-faults.stderr').decode())
    for receipt, directory, reference_prefix in [(runtime, 'runtime', 'reference-'), (replay, 'replay-runtime', 'memory-')]:
        for row in receipt['primary']:
            model = row['case']
            primary_base = base / 'original-primary/tlc-2026.10.06.014338' if directory == 'runtime' else base / 'current-primary'
            raw = read(primary_base / (model + '.stdout')).decode()
            count = re.search(r'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue\.', raw)
            depth = re.search(r'The depth of the complete state graph search is (\d+)\.', raw)
            actual = reports[f'{reference_prefix}{model}-tlc-fp-False'] if directory == 'runtime' else data[f'{reference_prefix}{model}-tlc-fp-False']
            assert count and depth
            assert tuple(map(int, count.groups())) == tuple(actual['statistics'][k] for k in ['generated', 'distinct', 'left_on_queue'])
            assert int(depth[1]) == actual['statistics']['depth']
            primary_exit = int(read(primary_base / (model + '.exit')))
            assert primary_exit == row.get('exit', row.get('reference_exit'))
    for name, count in [('cm1', 21), ('vts', 16)]:
        receipt = load(base / f'{name}.receipt.json')
        assert receipt['producer'] == PRODUCER and receipt['exit'] == 0
        assert receipt['selected'] == count and receipt['workers'] == '1,4,8'
    for name in ('disk_seen_safety', 'disk_seen_replay'):
        receipt = load(base / f'{name}.receipt.json')
        assert receipt['runtime_source_commit'] == PRODUCER and receipt['exit'] == 0
    metadata = load(base / 'metadata/receipt.json')
    assert metadata['runtime_commit'] == PRODUCER
    assert [r['command'] for r in metadata['commands']] == gate['rows'][0]['commands']
    assert all(r['exit'] == 0 for r in metadata['commands'])
    for row in metadata['commands']:
        assert sha(read(safe(base, row['log']))) == row['sha256']
    equality = load(base / 'metadata/rust-cargo-equality.json')
    assert equality['runtime_commit'] == PRODUCER
    assert equality['selected_original_input_pairs'] == 1226
    assert equality['all_git_live_archived_bytes_equal'] is True
    for binary in index['binaries']:
        assert binary['runtime_source_commit'] == PRODUCER
        if binary_root is not None:
            assert sha(read(safe(binary_root, binary['name']))) == binary['sha256']
    return {'producer': PRODUCER, 'source_pairs': len(source['inputs']),
            'payloads': len(index['payload_sha256']), 'family_certified': False,
            'binaries_checked': binary_root is not None, 'original_inputs_checked': source_root is not None}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('delivery', nargs='?', type=pathlib.Path,
                        default=pathlib.Path(__file__).resolve().parent.parent / 'conformance/probes/fallible-invariant-replay/current')
    parser.add_argument('--binary-root', type=pathlib.Path)
    parser.add_argument('--source-root', type=pathlib.Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.delivery, args.binary_root, args.source_root), sort_keys=True))
