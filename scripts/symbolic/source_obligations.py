#!/usr/bin/env python3
"""Adapt trusted source induction workers to Sail's fresh-process interface."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, directory):
    command = [sys.executable]
    if args.backend == 'bdd':
        command.extend([Path(__file__).with_name('check_named_induction.py'), args.module, args.config,
                        '--exporter', args.exporter, '--layout', args.layout,
                        '--candidate', args.candidate, '--property', args.property])
    else:
        if args.module.name != 'AsyncRaft.tla':
            raise ValueError('the finite Z3 backend requires an AsyncRaft.tla source')
        script = Path(__file__).parents[1] / 'perf/scale/raft_induction/check.py'
        command.extend([script, args.module.parent, '--exporter', args.exporter, '--config', args.config,
                        '--save', Path(directory) / 'certificate.json'])
    command.append('--rebind')
    if args.obligation_cache:
        command.extend(['--obligation-cache', args.obligation_cache])
    result = subprocess.run(command, stdout=subprocess.PIPE, check=False, text=True)
    data = json.loads(result.stdout)
    if result.returncode not in (0, 2) or data.get('result') != ('proved' if result.returncode == 0 else 'inconclusive'):
        raise ValueError('source induction worker did not return a proof or an inconclusive result')
    return result.returncode, data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', required=True, choices=('bdd', 'z3'))
    parser.add_argument('--exporter', required=True, type=Path)
    parser.add_argument('--layout')
    parser.add_argument('--candidate')
    parser.add_argument('--property')
    parser.add_argument('module', type=Path)
    parser.add_argument('config', type=Path)
    parser.add_argument('--obligation-cache', type=Path)
    args = parser.parse_args()
    if args.backend == 'bdd' and not all((args.layout, args.candidate, args.property)):
        parser.error('BDD induction requires a layout, candidate and property')
    if args.backend == 'z3' and any((args.layout, args.candidate, args.property)):
        parser.error('The finite Z3 worker selects the pinned AsyncRaft invariant and strengthening')
    files = [args.module.absolute(), args.config.absolute()]
    args.module, args.config = files
    before = [sha(path) for path in files]
    row = dict(format='dive-source-obligations-v1', backend=args.backend,
               module_sha256=before[0], config_sha256=before[1])
    try:
        with TemporaryDirectory(prefix='dive-source-obligations-') as temp:
            code, data = run(args, temp)
        if [sha(path) for path in files] != before:
            raise ValueError('module or configuration changed during the proof')
    except (OSError, ValueError) as error:
        code, data = 2, dict(result='inconclusive', reason=str(error))
    row['result'] = data['result']
    if code == 0:
        reuse = data.pop('reuse', None) or data.pop('solver_metrics', {}).get('obligation_reuse', {})
        row.update(retained=reuse.get('retained', 0) + reuse.get('retained_groups', 0), reuse=reuse, proof=data)
    else:
        row.update(retained=0, reason=data.get('reason', 'undischarged original source obligation'))
    print(json.dumps(row), flush=True)
    return code


if __name__ == '__main__':
    sys.exit(main())
