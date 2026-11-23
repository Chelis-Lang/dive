#!/usr/bin/env python3
"""Render verified labeled Storage paths with the authors' pinned WiredTiger template."""

import argparse
import json
import os
from pathlib import Path
import re
import runpy
import sys

from labeled_path_cover import binding, rows, sha, verify

PINS = {
    'testgen.py': 'cff0ec841f76a6206b7cdafe4b57b360b374ff677b0d5093acdf20a20ce90409',
    'test_txn_model_template.py': '75349f48df2bfa5a5ccfb6b6998088c27b071c14c6d900dba2128b680f6ec375',
}
ACTIONS = {
    'StartTransaction', 'TransactionWrite', 'TransactionRead', 'TransactionRemove',
    'PrepareTransaction', 'CommitTransaction', 'CommitPreparedTransaction', 'AbortTransaction',
    'SetStableTimestamp', 'SetOldestTimestamp', 'RollbackToStable',
}
SAFE = re.compile(r'[a-zA-Z0-9_]+\Z')


def string(value):
    if not isinstance(value, str) or SAFE.fullmatch(value) is None:
        raise ValueError('WiredTiger template requires an identifier-like value')
    return value


def model(value):
    if not isinstance(value, dict) or value.keys() != {'model'}:
        raise ValueError('expected a named Storage model value')
    return string(value['model'])


def state_value(value):
    if type(value) is int:
        return value
    if isinstance(value, str):
        return string(value)
    if isinstance(value, dict) and value.keys() == {'fcn'}:
        return {model(key): state_value(element) for key, element in value['fcn']}
    raise ValueError('Storage projection contains a value outside the template schema')


def argument(value):
    if type(value) in (bool, int):
        return str(value).upper()
    if isinstance(value, dict):
        return model(value)
    return json.dumps(string(value))


def template(root):
    for name, expected in PINS.items():
        if sha(root / name) != expected:
            raise ValueError('WiredTiger template or generator differs from the authors’ release')
    previous = Path.cwd()
    try:
        os.chdir(root)
        return runpy.run_path(str(root / 'testgen.py'))['gen_wt_test_from_traces']
    finally:
        os.chdir(previous)


def generate(args):
    graph_before, paths_before = binding(args), sha(args.paths)
    evidence = verify(args)
    vertices = {vertex['fp']: {name: state_value(value) for name, value in vertex['val'].items()}
                for vertex in rows(args.states, 'states')}
    render = template(args.authors.resolve())
    args.output.mkdir()
    paths = steps = batches = 0
    pending = []

    def flush():
        nonlocal batches
        if pending:
            batches += 1
            render(pending, fname=args.output / f'test_dive_paths_{batches:03}.py', compact=True)
            pending.clear()

    with args.paths.open() as file:
        next(file)
        for line in file:
            word = json.loads(line)
            trace = []
            for _, edge in word['steps']:
                action = edge['act']
                if action not in ACTIONS:
                    raise ValueError(f'unsupported WiredTiger template action {action}')
                params = {string(name): argument(value) for name, value in edge['params'].items()}
                if 'tid' in params and params['tid'] not in ('t1', 't2', 't3'):
                    raise ValueError('transaction ID has no authored session')
                trace.append([[1, vertices[edge['from']]], {'name': action, 'context': params},
                              [2, vertices[edge['to']]]])
            pending.append({'action': trace})
            paths += 1
            steps += len(trace)
            if len(pending) == 1000:
                flush()
    flush()
    if (graph_before != binding(args) or paths_before != sha(args.paths)
            or (paths, steps) != (evidence['path_count'], evidence['path_steps'])):
        raise ValueError('path input changed after verification')
    return dict(result='rendered', paths=paths, steps=steps, batches=batches,
                output_bytes=sum(p.stat().st_size for p in args.output.glob('test_dive_paths_*.py')))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--graph', type=Path, required=True)
    parser.add_argument('--paths', type=Path, required=True)
    parser.add_argument('--authors', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        report = json.loads((args.graph / 'complete.json').read_text())
        if report['format'] != 'dive-labeled-concrete-graph-v1':
            raise ValueError('unrecognized concrete graph completion')
        args.root = report['roots']
        args.states = args.graph / 'states.json'
        args.edges = args.graph / 'edges.json'
        args.representatives = args.graph / 'representatives.json'
        print(json.dumps(generate(args), sort_keys=True))
    except (OSError, KeyError, ValueError, TypeError, StopIteration) as error:
        print(json.dumps(dict(result='refused', reason=str(error))))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
