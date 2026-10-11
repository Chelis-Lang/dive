#!/usr/bin/env python3
"""Always read back portable runtime and frozen input archives; no Java needed."""
import argparse
import base64
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def relative(value):
    path = PurePosixPath(value)
    require(not path.is_absolute() and '..' not in path.parts, 'unsafe archive path')
    return Path(value)


def read_json(path):
    return json.loads(path.read_bytes())


def verify(base=HERE, binaries=None):
    delivery = read_json(base / 'delivery.json')
    require(delivery['source_commit'] is None, 'historical uncommitted producer retagged')
    require(delivery['all_consumers_closed'] and not delivery['family_certified'], 'scope credit')
    producer_path = base / relative(delivery['source_archive'])
    require(digest(producer_path.read_bytes()) == delivery['source_archive_sha256'], 'producer changed')
    producer = read_json(producer_path)
    require(producer['source_commit'] is None and not producer['family_certified'], 'producer qualification')
    source_base = producer_path.parent / producer['source_archive_base']
    for name, record in producer['source'].items():
        relative(name)
        raw_archive = (source_base / relative(record['archive'])).read_bytes()
        require(digest(raw_archive) == record['archive_sha256'], 'source archive: ' + name)
        require(digest(gzip.decompress(raw_archive)) == record['raw_sha256'], 'source bytes: ' + name)
    require(len(producer['source']) == 2261, 'complete frozen compiled-input selection')
    packed = (base / relative(delivery['records_archive'])).read_bytes()
    require(digest(packed) == delivery['records_archive_sha256'], 'runtime pack changed')
    entries = json.loads(gzip.decompress(packed))
    records = {}
    total = 0
    for entry in entries:
        name = str(relative(entry['path']))
        require(name not in records, 'duplicate runtime record: ' + name)
        raw = base64.b64decode(entry['data'], validate=True)
        require(digest(raw) == entry['sha256'] == delivery['files_sha256'].get(name), 'runtime bytes: ' + name)
        records[name] = raw
        total += len(raw)
    require(set(records) == set(delivery['files_sha256']), 'missing runtime record')
    require(len(records) == delivery['records_count'] == 16611 and total == delivery['raw_total_bytes'], 'runtime census')
    reports = {}
    for name, expected in delivery['reports_sha256'].items():
        raw = (base / 'reports' / relative(name)).read_bytes()
        require(digest(raw) == expected, 'report changed: ' + name)
        reports[Path(name).stem] = json.loads(raw)
    require(reports['raw']['selected'] == 234 and reports['raw']['summary'] == {
        'exact-raw-ast': 152, 'named-refusal': 52,
        'primary-invalid-raw-uncredited': 28, 'rejection-accounted': 2}, 'raw dispositions')
    raw_rows = reports['raw']['rows']
    require(len(raw_rows) == len({row['id'] for row in raw_rows}) == 234, 'complete raw selection')
    require(dict(Counter(row['outcome'] for row in raw_rows)) == reports['raw']['summary'], 'raw row census')
    for row in raw_rows:
        document = row.get('document')
        if document is not None:
            require(document.get('model_proved') is False and document.get('artifact_admitted') is False, 'raw output falsely admitted/proved')
    public = reports['public']
    require(public['selected'] == 468 and not public['family_certified'] and not public['model_proved'], 'public scope')
    require(len(public['rows']) == len({row['id'] for row in public['rows']}) == 468, 'complete public selection')
    expected_public = {'admitted-exact-cst-sany-validator': 304, 'named-public-refusal': 36,
                       'invalid-primary-uncredited': 124, 'rejection-accounted': 4}
    require(public['summary'] == expected_public and dict(Counter(row['outcome'] for row in public['rows'])) == expected_public, 'public dispositions')
    for row in public['rows']:
        raw = gzip.decompress(records['corrected-public/' + row['id'] + '.native.stdout.gz'])
        require(digest(raw) == row['native']['stdout_sha256'], 'public stdout')
        document = json.loads(raw)
        require(document.get('model_proved') is False and document.get('exhaustive') is False, 'translation falsely proves model')
        require(document['profile'] == row['profile'] and document['result'] != 'internal-error', 'public profile/result')
        if row['outcome'] == 'admitted-exact-cst-sany-validator':
            require(row['native']['exit_code'] == 0 and document['accepted'] is True, 'artifact admission')
            require(document['algorithm_checksum_verified'] is True, 'algorithm CRC not verified')
            require(row['validator'] == {'configured': '[NO_DIVERGENCE]', 'unconfigured': '[NO_DIVERGENCE]'}, 'Validator admission')
    for name, folder, selected, settings, valid, mutants in [
        ('models', 'corrected-models', 16, 192, 8, 32),
        ('historical-models', 'corrected-historical-models', 24, 288, 12, 48)]:
        report = reports[name]
        require((report['selected'], report['worker_store_controls'], report['valid_witness_controls'], report['rejected_mutants']) == (selected, settings, valid, mutants), 'model census')
        require(report['native_sha256'] == delivery['binary_sha256']['dive'], 'model binary epoch')
        actual_valid = actual_mutants = actual_settings = 0
        for row in report['rows']:
            require(row['outcome'] == 'finite-model-match', 'model disagreement')
            for setting in row['settings']:
                require(setting['outcome'] == 'match', 'model setting')
                capture = setting['capture']; raw = gzip.decompress(records[folder + '/' + capture['stdout']['path']])
                require(digest(raw) == capture['stdout']['sha256'], 'model stdout')
                document = json.loads(raw)
                require(document['verdict'] in ('success', 'invariant-violation'), 'model outcome')
                for field in ('generated', 'distinct', 'depth', 'left_on_queue'):
                    require(document['statistics'][field] == row['primary_counts'][field], 'model count: ' + field)
                actual_settings += 1
            for control in row['independent_validation']:
                raw = gzip.decompress(records[folder + '/' + control['capture']['stdout']['path']])
                require(digest(raw) == control['capture']['stdout']['sha256'], 'witness stdout')
                document = json.loads(raw)
                if control['control'] == 'original':
                    require(control['profile'] == 'tlc' and control['outcome'] == 'valid' and document['result'] == 'valid', 'uncertified witness')
                    actual_valid += 1
                else:
                    require(control['outcome'] == 'rejected' and document['result'] != 'valid' and control['capture']['exit_code'] != 0, 'accepted corrupt witness')
                    actual_mutants += 1
        require((actual_settings, actual_valid, actual_mutants) == (settings, valid, mutants), 'actual model census')
    for name, record in delivery['gate_logs'].items():
        packed_log = (base / relative(record['archive'])).read_bytes()
        require(digest(packed_log) == record['archive_sha256'], 'gate log archive')
        raw = gzip.decompress(packed_log)
        require(digest(raw) == record['raw_sha256'] and record['exit_code'] == 0, 'gate log: ' + name)
    gate = reports['full-gate']
    require(gate['passed'] and gate['all_stages'] == len(gate['completed']) == 33 and all(x['exit_code'] == 0 for x in gate['completed']), 'incomplete gate')
    log = b'\n'.join(gzip.decompress((base / x['archive']).read_bytes()) for x in delivery['gate_logs'].values())
    require(b'1099 tests run: 1099 passed, 0 skipped' in log, 'nextest inventory incomplete')
    metadata = read_json(base / 'packaging-controls.json')
    metadata_raw = (base / relative(metadata['archive'])).read_bytes()
    require(digest(metadata_raw) == metadata['archive_sha256'], 'packaging controls changed')
    metadata_rows = json.loads(gzip.decompress(metadata_raw))
    require(len(metadata_rows) == len(metadata['files_sha256']) == metadata['records_count'], 'packaging selection')
    for row in metadata_rows:
        name = str(relative(row['path']))
        require(digest(base64.b64decode(row['data'], validate=True)) == row['sha256'] == metadata['files_sha256'][name], 'packaging bytes')
    require(metadata['historical_producer_retagged'] is False, 'packaging retagged producer')
    if binaries:
        for name, expected in delivery['binary_sha256'].items():
            require(digest((binaries / relative(name)).read_bytes()) == expected, 'binary changed: ' + name)
    return {'runtime_records': len(records), 'source_inputs': len(producer['source']), 'public_rows': 468, 'model_settings': 480, 'gate_tests': 1099, 'family_certified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binaries', type=Path, help='optional readback of six immutable external executables')
    options = parser.parse_args()
    print(json.dumps(verify(binaries=options.binaries), sort_keys=True))
