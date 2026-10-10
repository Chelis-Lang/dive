#!/usr/bin/env python3
"""Verify final binary, report, artifact and raw-capture integrity for this slice."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(args):
    base = Path(__file__).resolve().parent
    counts = [0, 0, 0, 0]
    for directory in [base, base / 'extra', base / 'bounds']:
        comparison = json.loads((directory / 'comparison.json').read_text())
        report = json.loads((directory / 'validation/report.json').read_text())
        assert comparison['native_sha256'] == sha(args.example)
        assert comparison['comparator_sha256'] == sha(base / 'compare.py')
        assert report['native_sha256'] == sha(args.native)
        assert report['cst_dumper_sha256'] == sha(args.example)
        assert report['script_sha256'] == sha(base / 'validate.py')
        assert report['selection_report_sha256'] == sha(directory / 'comparison.json')
        assert not comparison['summary'].get('disagree')
        assert not report['artifact_summary'].get('disagree')
        assert not report['model_summary'].get('disagree')
        assert sum(comparison['summary'].values()) == comparison['selected']
        assert report['selected_artifacts'] == len(report['artifact_rows'])
        assert report['family_certified'] is False
        for row in report['artifact_rows']:
            assert row['operator_structure_equal']
            assert sha(directory / 'validation' / row['artifact']) == row['artifact_sha256']
        for row in report['model_rows']:
            assert sha(directory / 'validation' / row['source']) == row['source_sha256']
            assert sha(directory / 'validation' / row['config']) == row['config_sha256']
            assert len(row['workers_stores']) == 6
            assert all(control['passed'] for control in row['independent_validation'])
            counts[2] += sum(control['control'] == 'original' for control in row['independent_validation'])
            counts[3] += sum(control['control'] != 'original' for control in row['independent_validation'])

        def raw(obj):
            if isinstance(obj, dict):
                for stream in ['stdout', 'stderr']:
                    if stream + '_sha256' in obj:
                        data = gzip.decompress((directory / 'validation' / obj[stream]).read_bytes())
                        assert hashlib.sha256(data).hexdigest() == obj[stream + '_sha256']
                for value in obj.values():
                    raw(value)
            elif isinstance(obj, list):
                for value in obj:
                    raw(value)

        raw(report)
        counts[0] += len(report['artifact_rows'])
        counts[1] += len(report['model_rows'])
    assert counts == [206, 60, 14, 56], counts
    print(json.dumps({'verified_artifacts_models_valid_mutants': counts,
                      'native_sha256': sha(args.native),
                      'example_sha256': sha(args.example)}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    verify(parser.parse_args())
