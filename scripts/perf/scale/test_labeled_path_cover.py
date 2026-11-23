#!/usr/bin/env python3
"""Controls for concrete rooted coverage and genuinely distinct parallel labels."""

from argparse import Namespace
import json
from pathlib import Path
from tempfile import TemporaryDirectory

from labeled_path_cover import select, verify


def check(path):
    args = Namespace(states=path / 'states.json', edges=path / 'edges.json', representatives=None,
                     root=[1, 100], paths=path / 'paths.jsonl')
    args.states.write_text(json.dumps(dict(states=[{'fp': state} for state in [1, 2, 3, 100]]),
                                     separators=(',', ':')))
    edges = [dict(**{'from': 1, 'to': 2}, act='send', params={'receiver': 'p1'}),
             dict(**{'from': 1, 'to': 2}, act='send', params={'receiver': 'p2'}),
             dict(**{'from': 2, 'to': 3}, act='receive', params={'receiver': 'p2'})]
    args.edges.write_text(json.dumps(dict(edges=edges), separators=(',', ':')))
    assert select(args)['selected_paths'] == 2
    answer = verify(args)
    assert answer['classes_covered'] == 4 and answer['graph_action_occurrences']['send'] == 2
    saved = args.paths.read_text().splitlines()
    for line, value in enumerate(saved[1:], 1):
        word = json.loads(value)
        if len(word['steps']) == 2:
            assert word['steps'][0] == [1, edges[1]]
            word['steps'][0][1]['params']['receiver'] = 'p1'
            saved[line] = json.dumps(word)
            break
    else:
        raise AssertionError('missing selected concrete path')
    args.paths.write_text('\n'.join(saved) + '\n')
    try:
        verify(args)
    except ValueError as error:
        assert 'arguments differ' in str(error)
    else:
        raise AssertionError('parameter change under the same edge ordinal accepted')


if __name__ == '__main__':
    with TemporaryDirectory(prefix='dive-path-cover-') as scratch:
        check(Path(scratch))
