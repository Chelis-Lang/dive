#!/usr/bin/env python3
"""Compare a small Core capture with the authors' complete labeled reference graph."""

import argparse
from collections import Counter
import json
from pathlib import Path

import networkx as nx

from labeled_path_cover import rows
from wiredtiger_paths import argument, state_value


def graph(states, edges, roots, native):
    out = nx.MultiDiGraph()
    for row in rows(states, 'states'):
        values = row['val']
        if native:
            values = {name: state_value(value) for name, value in values.items()}
        out.add_node(row['fp'], values=values, init=row['fp'] in roots)
    for edge in rows(edges, 'edges'):
        params = edge['params']
        if native:
            params = {name: argument(value) for name, value in params.items()}
        label = json.dumps([edge['act'], params], sort_keys=True)
        if edge['from'] not in out or edge['to'] not in out:
            raise ValueError('edge points outside the state inventory')
        out.add_edge(edge['from'], edge['to'], value=label)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('core', type=Path)
    parser.add_argument('states', type=Path)
    parser.add_argument('edges', type=Path)
    parser.add_argument('--reference-root', type=int, action='append', required=True)
    args = parser.parse_args()
    complete = json.loads((args.core / 'complete.json').read_text())
    native = graph(args.core / 'states.json', args.core / 'edges.json', set(complete['roots']), True)
    reference = graph(args.states, args.edges, set(args.reference_root), False)

    def labels(multiedge):
        return Counter(edge['value'] for edge in multiedge.values())

    match = nx.algorithms.isomorphism.MultiDiGraphMatcher(native, reference,
        node_match=lambda a, b: a == b, edge_match=lambda a, b: labels(a) == labels(b))
    if not match.is_isomorphic():
        raise SystemExit('complete graphs differ in rooted states, labels or bound arguments')
    print(json.dumps(dict(result='exact-labeled-isomorphism', states=len(native), edges=native.number_of_edges(),
                          parallel_endpoint_pairs=sum(native.number_of_edges(x, y) > 1
                                                      for x, y in set(native.edges())))))


if __name__ == '__main__':
    main()
