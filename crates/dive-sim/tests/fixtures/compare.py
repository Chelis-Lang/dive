#!/usr/bin/env python3
"""Compare an actual native simulation CLI with the clean pinned controls."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


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
        states = [[state['x'] for state in trace['tlc_states']] for trace in report['traces']]
        agree = (exit_code == row['exit'] == 0 and report['result'] == 'sampled'
                 and report['statistics']['generated'] == row['generated']
                 and states == row['traces'] and not report['model_proved']
                 and all(trace['validation']['result'] == 'valid' for trace in report['traces']))
        observations.append(dict(control='selection', seed=row['seed'], aril=row['aril'],
                                 outcome='agree' if agree else 'disagree',
                                 statistics=report['statistics'], states=states))
    for row in json.loads((fixtures / 'error-oracle.json').read_text()):
        exit_code, report = run(row['model'], 0, 0, 4, 1)
        if row['native_scope'] == 'unsupported':
            if exit_code != 3 or report['result'] != 'unsupported':
                raise SystemExit('constrained action-property slice boundary was not refused')
            observations.append(dict(control=row['model'], outcome='unsupported',
                                     result=report['result'], oracle_code=row['codes'][0],
                                     message=report['message'], statistics=report['statistics']))
            continue
        agree = (exit_code == row['exit'] and report['tlc_code'] == row['codes'][0]
                 and (row['generated'] is None or report['statistics']['generated'] == row['generated'])
                 and (report['result'] != 'violation' or report['violation_validated']))
        observations.append(dict(control=row['model'], outcome='agree' if agree else 'disagree',
                                 result=report['result'], exit=exit_code, tlc_code=report['tlc_code'],
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
