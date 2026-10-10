#!/usr/bin/env python3
"""Verify complete procedure evidence without giving switched-profile replay strict credit."""
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


def main(args):
    base = Path(__file__).resolve().parent
    repo = base.parents[2]
    receipt = json.loads((base / 'delivery.json').read_text())
    for key in ['reports', 'checked_sources', 'gate_logs', 'archived_evidence']:
        for path, expected in receipt[key].items():
            assert sha(repo / path) == expected, path
    assert receipt['native_sha256'] == sha(args.native)
    assert receipt['example_sha256'] == sha(args.example)
    manifest = json.loads((base / 'fixtures/manifest.json').read_text())
    comparison = json.loads((base / 'comparison.json').read_text())
    report = json.loads((base / 'validation/report.json').read_text())
    assert comparison['manifest_sha256'] == sha(base / 'fixtures/manifest.json')
    assert comparison['native_sha256'] == sha(args.example)
    assert comparison['comparator_sha256'] == sha(base / 'compare.py')
    assert comparison['model_proved'] is False
    assert len(manifest['rows']) == comparison['selected'] == len(comparison['rows']) == 84
    assert {r['id'] for r in manifest['rows']} == {r['id'] for r in comparison['rows']}
    assert comparison['summary'] == dict(Counter(r['outcome'] for r in comparison['rows']))
    assert comparison['summary'] == {'rejection-match': 14, 'native-refusal': 20,
                                     'token-artifact-match': 48, 'invalid-reference-artifact': 2}
    invalid = 0
    for row in manifest['rows']:
        assert sha(base / 'cases' / row['input']) == row['input_sha256']
        invalid += row.get('sany', {}).get('status') == 'rejected'
        raw(row, base / 'fixtures')
    assert invalid == 4
    assert report['native_sha256'] == sha(args.native)
    assert report['cst_dumper_sha256'] == sha(args.example)
    assert report['script_sha256'] == sha(base / 'validate.py')
    assert report['selection_report_sha256'] == sha(base / 'comparison.json')
    assert report['family_certified'] is False
    assert report['selected_artifacts'] == len(report['artifact_rows']) == 48
    for row in report['artifact_rows']:
        assert row['outcome'] == 'native-sany-match' and row['operator_structure_equal']
        assert sha(base / 'validation' / row['artifact']) == row['artifact_sha256']
    assert len(report['model_rows']) == 76
    assert report['model_summary'] == {'finite-model-match': 46, 'disagree': 30}
    for row in report['model_rows']:
        assert sha(base / 'validation' / row['source']) == row['source_sha256']
        assert sha(base / 'validation' / row['config']) == row['config_sha256']
        assert len(row['workers_stores']) == 6
        assert {(c['workers'], c['store']) for c in row['workers_stores']} == {
            (w, s) for w in [1, 4, 8] for s in ['fp', 'exact']}
        assert all(c['outcome'] == 'match' for c in row['workers_stores'])
        document = json.loads(gzip.decompress((base / 'validation' / row['native']['stdout']).read_bytes()))
        assert document['options']['profile'] == 'tlc'
        assert row['native']['exit_code'] == row['primary']['exit_code'] == document['tlc']['exit_status']
        assert document['verdict'] == ('success' if row['outcome'] == 'finite-model-match' else 'invariant-violation')
        assert all(document['statistics'].get(k) == v for k, v in row['primary_counts'].items() if v is not None)
    raw(report, base / 'validation')
    qualified = json.loads((base / 'qualified-replay/report.json').read_text())
    assert qualified['original_report_sha256'] == sha(base / 'validation/report.json')
    assert qualified['script_sha256'] == sha(base / 'qualified-replay.py')
    assert qualified['native_sha256'] == sha(args.native)
    assert qualified['family_certified'] is False and qualified['tlc_violation_certified'] is False
    assert qualified['summary'] == {'strict-complete-model-match': 46, 'strict-witness-pending': 30}
    assert len(qualified['rows']) == 76
    assert {r['id'] for r in qualified['rows']} == {r['id'] for r in report['model_rows']}
    reason = 'runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)'
    for row in qualified['rows']:
        if row['outcome'] == 'strict-witness-pending':
            assert row['tlc_certified'] is False
            assert row['strict_validation']['result'] == 'unsupported'
            assert row['strict_validation']['unsupported'] == reason
        else:
            assert row['outcome'] == 'strict-complete-model-match'
    assert len(qualified['controls']) == 150
    assert Counter(c['result'] for c in qualified['controls']) == {'valid': 30, 'invalid': 120}
    for control in qualified['controls']:
        assert control['strict_tlc_credit'] is False and control['profile'] == 'dive-default'
        assert (control['result'], control['exit_code']) == (('valid', 0) if control['control'] == 'original' else ('invalid', 1))
    raw(qualified, base / 'qualified-replay')
    mutations = json.loads((base / 'mutation-controls.json').read_text())
    assert mutations['control_count'] == len(mutations['rows']) == 39
    assert all(r['outcome']['outcome'] == 'disagree' for r in mutations['rows'])
    admission = json.loads((base / 'admission/report.json').read_text())
    assert admission['controls'] == len(admission['rows']) == 8
    assert admission['script_sha256'] == sha(base / 'check-admission.py')
    assert admission['native_sha256'] == sha(args.native)
    assert all(r['passed'] for r in admission['rows'])
    raw(admission, base / 'admission')
    guide = json.loads((base / 'guide-api/report.json').read_text())
    assert guide['script_sha256'] == sha(base / 'check-guide-api.py')
    assert guide['guide_sha256'] == sha(repo / 'docs/pluscal-procedures.md')
    assert guide['controls'] == len(guide['rows']) == 2
    assert all(r['result'] == 'artifact' and r['model_proved'] is False and r['exhaustive'] is False for r in guide['rows'])
    raw(guide, base / 'guide-api')
    main_controls = json.loads((base / 'main-admission/report.json').read_text())
    assert main_controls['script_sha256'] == sha(base / 'check-main-admission.py')
    assert main_controls['native_sha256'] == sha(args.native)
    assert main_controls['example_sha256'] == sha(args.example)
    assert main_controls['controls'] == len(main_controls['rows']) == 7
    for row in main_controls['rows']:
        assert sha(base / 'main-admission' / row['source']) == row['source_sha256']
        assert row['artifact_certified'] is False
        assert len(row['observations']) == 3
        assert all(c['exit_code'] not in [4, 101, -6] for c in row['observations'] if c['entry'] != 'primary')
    raw(main_controls, base / 'main-admission')
    print(json.dumps({'selection': comparison['summary'], 'artifacts': 48,
                      'strict_complete_models': 46, 'strict_witness_pending': 30,
                      'width_store_checks': 456, 'default_valid_witnesses': 30,
                      'default_invalid_mutants': 120, 'primary_invalid_uncredited': 4,
                      'family_certified': False, 'tlc_violation_certified': False}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    main(parser.parse_args())
