#!/usr/bin/env python3
"""Compare an actual native simulation CLI with the clean pinned controls."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def metadata_mismatches(report, process_exit):
    """Sampling and refusals must never hide a proof or inconsistent evidence."""
    mismatches = []
    if report.get('exhaustive') is not False or report.get('model_proved') is not False:
        mismatches.append('proof-scope')
    if type(report.get('exit_code')) is not int or report['exit_code'] != process_exit:
        mismatches.append('process-exit')
    if report.get('command') != 'simulate' or report.get('profile') != 'tlc':
        mismatches.append('mode-profile')
    validated = report.get('result') == 'violation'
    if report.get('violation_validated') is not validated:
        mismatches.append('independent-validation')
    if report.get('result') == 'internal-error':
        mismatches.append('internal-error')
    return mismatches


def selection_mismatches(row, report, process_exit):
    mismatches = metadata_mismatches(report, process_exit)
    states = [[state['x'] for state in trace['tlc_states']] for trace in report['traces']]
    if (process_exit != row['exit'] or process_exit != 0
            or report.get('result') != 'sampled'
            or report['statistics']['generated'] != row['generated']
            or states != row['traces']):
        mismatches.append('selection')
    if any(trace['validation']['result'] != 'valid' for trace in report['traces']):
        mismatches.append('sample-validation')
    return mismatches, states


def error_comparison(row, report, process_exit):
    mismatches = metadata_mismatches(report, process_exit)
    # `native_scope` records the scope when the control was captured; a row
    # that dive refused then and supports now must agree like any other.
    if row['native_scope'] == 'unsupported' and report.get('result') == 'unsupported':
        if (process_exit != 3
                or not isinstance(report.get('message'), str) or not report['message'].strip()):
            mismatches.append('named-refusal')
        return ('disagree' if mismatches else 'unsupported'), mismatches
    expected = 'violation' if row['codes'][0] in (2107, 2110, 2112, 2114) else 'evaluation-error'
    if (process_exit != row['exit'] or report.get('tlc_code') != row['codes'][0]
            or report.get('result') != expected
            or (row['generated'] is not None
                and report['statistics']['generated'] != row['generated'])):
        mismatches.append('deciding-fields')
    if expected == 'violation':
        traces = report.get('traces', [])
        states = [state['x'] for state in traces[-1]['tlc_states']] if traces else []
        if states != row['states'] or not traces or traces[-1]['validation']['result'] != 'valid':
            mismatches.append('violation-witness')
    return ('disagree' if mismatches else 'agree'), mismatches


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dive', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    binary = args.dive.resolve()
    fixtures = Path(__file__).resolve().parent
    provenance = json.loads((fixtures / 'capture-provenance.json').read_text())
    for name, digest in provenance['raw_files'].items():
        if hashlib.sha256((fixtures / name).read_bytes()).hexdigest() != digest:
            raise SystemExit(f'raw evidence hash mismatch: {name}')
    observations = []

    def run(model, seed, aril, depth, traces):
        command = [str(binary), 'simulate', str(fixtures / f'{model}.tla'),
                   '--compat=tlc', '--seed', str(seed), '--aril', str(aril),
                   '--depth', str(depth), '--traces', str(traces), '--output', 'json']
        completed = subprocess.run(command, text=True, capture_output=True, timeout=30)
        return completed.returncode, json.loads(completed.stdout)

    for row in json.loads((fixtures / 'choice-oracle.json').read_text()):
        exit_code, report = run('SimChoice', row['seed'], row['aril'], 5, 3)
        mismatches, states = selection_mismatches(row, report, exit_code)
        observations.append(dict(control='selection', seed=row['seed'], aril=row['aril'],
                                 outcome='disagree' if mismatches else 'agree', mismatches=mismatches,
                                 statistics=report['statistics'], states=states))
    for row in json.loads((fixtures / 'error-oracle.json').read_text()):
        exit_code, report = run(row['model'], 0, 0, 4, 1)
        outcome, mismatches = error_comparison(row, report, exit_code)
        observations.append(dict(control=row['model'], outcome=outcome, mismatches=mismatches,
                                 result=report['result'], exit=exit_code, tlc_code=report['tlc_code'],
                                 oracle_code=row['codes'][0], message=report.get('message'),
                                 statistics=report['statistics'], violation_validated=report['violation_validated']))
    result = dict(schema_version=1, scope='curated pure finite single-worker simulation; not family certification',
                  oracle=provenance['oracle'], oracle_sha256=provenance['jar_sha256'],
                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  selected=len(observations), agree=sum(row['outcome'] == 'agree' for row in observations),
                  unsupported=sum(row['outcome'] == 'unsupported' for row in observations),
                  observations=observations)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2) + '\n')
    print(f"simulation controls: {result['agree']}/{result['selected']} agree; {result['unsupported']} named refusals")
    if result['agree'] + result['unsupported'] != result['selected']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
