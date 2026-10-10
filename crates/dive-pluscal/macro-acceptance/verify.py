#!/usr/bin/env python3
"""Verify this bounded macro delivery's sources, complete selection and raw evidence."""
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(obj, directory):
    if isinstance(obj, dict):
        for stream in ['stdout', 'stderr']:
            if stream + '_sha256' in obj:
                data = gzip.decompress((directory / obj[stream]).read_bytes())
                assert hashlib.sha256(data).hexdigest() == obj[stream + '_sha256']
        if 'path' in obj and 'sha256' in obj:
            path = directory / obj['path']
            data = path.read_bytes()
            if path.suffix == '.gz':
                data = gzip.decompress(data)
            assert hashlib.sha256(data).hexdigest() == obj['sha256'], path
        for value in obj.values():
            raw(value, directory)
    elif isinstance(obj, list):
        for value in obj:
            raw(value, directory)


def verify(args):
    base = Path(__file__).resolve().parent
    repo = base.parents[2]
    receipt = json.loads((base / 'delivery.json').read_text())
    for key in ['reports', 'checked_sources', 'gate_logs']:
        for path, expected in receipt[key].items():
            assert sha(repo / path) == expected, path
    assert receipt['native_sha256'] == sha(args.native)
    assert receipt['example_sha256'] == sha(args.example)
    counts = [0, 0, 0, 0, 0]
    census = Counter()
    invalid = 0
    for folder in ['', 'extra', 'process', 'scope', 'lexical', 'origins']:
        directory = base / folder
        manifest = json.loads((directory / 'fixtures/manifest.json').read_text())
        comparison = json.loads((directory / 'comparison.json').read_text())
        report = json.loads((directory / 'validation/report.json').read_text())
        assert comparison['manifest_sha256'] == sha(directory / 'fixtures/manifest.json')
        assert comparison['native_sha256'] == sha(args.example)
        assert comparison['comparator_sha256'] == sha(base / 'compare.py')
        assert comparison['model_proved'] is False
        assert len(manifest['rows']) == comparison['selected'] == len(comparison['rows'])
        assert sum(comparison['summary'].values()) == comparison['selected']
        assert comparison['summary'] == dict(Counter(row['outcome'] for row in comparison['rows']))
        assert not comparison['summary'].get('disagree')
        census.update(comparison['summary'])
        for row in manifest['rows']:
            assert sha(directory / 'cases' / row['input']) == row['input_sha256']
            invalid += row.get('sany', {}).get('status') == 'rejected'
            raw(row, directory / 'fixtures')
        assert report['native_sha256'] == sha(args.native)
        assert report['cst_dumper_sha256'] == sha(args.example)
        assert report['script_sha256'] == sha(base / 'validate.py')
        assert report['selection_report_sha256'] == sha(directory / 'comparison.json')
        assert report['family_certified'] is False
        assert report['selected_artifacts'] == len(report['artifact_rows'])
        assert len(report['artifact_rows']) == comparison['summary'].get('token-artifact-match', 0)
        for row in report['artifact_rows']:
            assert row['outcome'] == 'native-sany-match' and row['operator_structure_equal']
            assert sha(directory / 'validation' / row['artifact']) == row['artifact_sha256']
        for row in report['model_rows']:
            assert row['outcome'] == 'finite-model-match'
            assert sha(directory / 'validation' / row['source']) == row['source_sha256']
            assert sha(directory / 'validation' / row['config']) == row['config_sha256']
            assert len(row['workers_stores']) == 6
            assert {(c['workers'], c['store']) for c in row['workers_stores']} == {
                (w, s) for w in [1, 4, 8] for s in ['fp', 'exact']}
            assert all(c['outcome'] == 'match' for c in row['workers_stores'])
            assert all(c['passed'] for c in row['independent_validation'])
            counts[2] += sum(c['control'] == 'original' for c in row['independent_validation'])
            counts[3] += sum(c['control'] != 'original' for c in row['independent_validation'])
            counts[4] += len(row['workers_stores'])
        raw(report, directory / 'validation')
        counts[0] += len(report['artifact_rows'])
        counts[1] += len(report['model_rows'])
    assert counts == [134, 132, 25, 100, 792], counts
    assert invalid == 27, invalid
    assert dict(census) == {'token-artifact-match': 134, 'rejection-match': 48,
                            'native-refusal': 30, 'invalid-reference-artifact': 8,
                            'oracle-error': 2}, census
    mutations = json.loads((base / 'mutation-controls.json').read_text())
    assert mutations['control_count'] == len(mutations['rows']) == 44
    assert all(r['outcome']['outcome'] == 'disagree' for r in mutations['rows'])
    admission = json.loads((base / 'admission/report.json').read_text())
    assert admission['controls'] == len(admission['rows']) == 54
    assert admission['script_sha256'] == sha(base / 'check-admission.py')
    assert admission['native_sha256'] == sha(args.native)
    assert all(r['passed'] for r in admission['rows'])
    raw(admission, base / 'admission')
    print(json.dumps({'verified_artifacts_models_valid_mutants_widths': counts,
                      'selection': dict(census), 'primary_invalid_uncredited': invalid,
                      'family_certified': False}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    verify(parser.parse_args())
