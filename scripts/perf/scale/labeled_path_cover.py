#!/usr/bin/env python3
"""Select and independently validate rooted concrete paths in a labeled TLC graph."""

import argparse
from array import array
from collections import Counter, deque
import hashlib
import json
from pathlib import Path
import sys
import time


def rows(path, key):
    """Read the author's single-array JSON without retaining all its objects."""
    decoder = json.JSONDecoder()
    with path.open(encoding="utf-8") as stream:
        expected = '{"' + key + '":['
        if stream.read(len(expected)) != expected:
            raise ValueError(f"expected {key} array in {path}")
        buf = stream.read(1 << 20)
        cursor = 0
        separator = False
        while True:
            while cursor < len(buf) and buf[cursor].isspace():
                cursor += 1
            if len(buf) - cursor < 4096:
                buf, cursor = buf[cursor:] + stream.read(1 << 20), 0
                while cursor < len(buf) and buf[cursor].isspace():
                    cursor += 1
            if buf[cursor:cursor + 2] == ']}':
                if buf[cursor + 2:].strip() or stream.read().strip():
                    raise ValueError(f"trailing data in {path}")
                return
            if separator:
                if buf[cursor:cursor + 1] != ',':
                    raise ValueError(f"missing separator in {path}")
                cursor += 1
                while cursor < len(buf) and buf[cursor].isspace():
                    cursor += 1
            while True:
                try:
                    value, cursor = decoder.raw_decode(buf, cursor)
                    break
                except json.JSONDecodeError:
                    tail = stream.read(1 << 20)
                    if not tail:
                        raise
                    buf, cursor = buf[cursor:] + tail, 0
            if not isinstance(value, dict):
                raise ValueError(f"expected object in {path}")
            yield value
            separator = True


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def vertex_map(states):
    positions = {}
    for value in rows(states, 'states'):
        state = value['fp']
        if type(state) is not int or state in positions:
            raise ValueError('invalid or duplicate vertex ID')
        positions[state] = len(positions)
    return positions


def targets(args, positions):
    return (set(positions) if args.representatives is None else
            set(vertex_map(args.representatives)))


def binding(args):
    return dict(format='rooted-labeled-paths-v1', roots=args.root,
                states_sha256=sha(args.states), edges_sha256=sha(args.edges),
                representatives_sha256=sha(args.representatives) if args.representatives else None)


def select(args):
    identity = binding(args)
    positions = vertex_map(args.states)
    required = targets(args, positions)
    if not required or not required <= positions.keys():
        raise ValueError('every class representative must be a concrete vertex')
    n = len(positions)
    edges = args.edges
    # Linked adjacency retains edge identity and parallel labels without keeping
    # the large action argument objects in memory. The second scan fetches only
    # the selected labeled records by their exact occurrence ordinals.
    head = array('q', [-1]) * n
    next_edge = array('q')
    destination = array('I')
    for index, edge in enumerate(rows(edges, 'edges')):
        source = positions[edge['from']]
        destination.append(positions[edge['to']])
        next_edge.append(head[source])
        head[source] = index
    roots = [positions[v] for v in args.root]
    parent = array('q', [-1]) * n
    via = array('q', [-1]) * n
    pending = deque(roots)
    for root in roots:
        parent[root] = root
    visit = array('I')
    while pending:
        source = pending.popleft()
        visit.append(source)
        edge = head[source]
        while edge != -1:
            child = destination[edge]
            if parent[child] == -1:
                via[child] = edge
                parent[child] = source
                pending.append(child)
            edge = next_edge[edge]
    if any(parent[positions[v]] == -1 for v in required):
        raise ValueError('a required vertex has no path from any declared Init')
    original = list(positions)
    marked = bytearray(n)
    for value in required:
        marked[positions[value]] = 1
    has_test = bytearray(n)
    selected = array('I')
    for node in reversed(visit):
        if marked[node] and not has_test[node]:
            selected.append(node)
            has_test[node] = 1
        if has_test[node]:
            has_test[parent[node]] = 1
    needed = {via[x] for x in visit if has_test[x] and via[x] != -1}
    records = {i: edge for i, edge in enumerate(rows(edges, 'edges')) if i in needed}
    with args.paths.open('x', encoding='utf-8') as out:
        json.dump(identity, out)
        out.write('\n')
        for end in reversed(selected):
            path = []
            node = end
            while parent[node] != node:
                edge = via[node]
                path.append([edge, records[edge]])
                node = parent[node]
            json.dump(dict(root=original[node], steps=path[::-1]), out, separators=(',', ':'))
            out.write('\n')
    return dict(vertices=n, graph_edge_occurrences=len(destination), reached=len(visit),
                selected_paths=len(selected), selected_edge_occurrences=len(needed))


def verify(args):
    """Replay a path file without using the selector's BFS or parent tree."""
    positions = vertex_map(args.states)
    required = targets(args, positions)
    if not required or not required <= positions.keys():
        raise ValueError('every required representative must be concrete')
    roots = set(args.root)
    if not roots <= positions.keys():
        raise ValueError('declared Init is absent from the graph')
    covered, edge_claims = set(), {}
    actions = Counter()
    path_count = steps = 0
    with args.paths.open(encoding='utf-8') as incoming:
        header = json.loads(next(incoming))
        expected = binding(args)
        if header != expected:
            raise ValueError('path graph or Init identity differs')
        for line in incoming:
            path = json.loads(line)
            current = path['root']
            if type(current) is not int or current not in roots:
                raise ValueError('path has no declared Init root')
            covered.add(current)
            path_count += 1
            for index, edge in path['steps']:
                if type(index) is not int or index < 0 or edge['from'] != current:
                    raise ValueError('non-contiguous concrete edge or invalid occurrence ordinal')
                if edge_claims.setdefault(index, edge) != edge:
                    raise ValueError('conflicting payload for one edge ordinal')
                current = edge['to']
                covered.add(current)
                actions[edge['act']] += 1
                steps += 1
    if not required <= covered:
        raise ValueError(f'missing {len(required - covered)} required vertices')
    original_actions = Counter()
    for index, edge in enumerate(rows(args.edges, 'edges')):
        original_actions[edge['act']] += 1
        if index in edge_claims and edge_claims.pop(index) != edge:
            raise ValueError('edge endpoints, action or arguments differ from the bound graph')
    if edge_claims:
        raise ValueError('claimed edge occurrence absent from graph')
    if binding(args) != expected:
        raise ValueError('graph changed during verification')
    return dict(result='verified', path_count=path_count, path_steps=steps,
                classes_required=len(required), classes_covered=len(required),
                concrete_states_covered=len(covered), action_steps=dict(actions),
                graph_action_occurrences=dict(original_actions))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('select', 'verify'))
    parser.add_argument('--states', type=Path, required=True)
    parser.add_argument('--edges', type=Path, required=True)
    parser.add_argument('--representatives', type=Path)
    parser.add_argument('--root', type=int, required=True, action='append')
    parser.add_argument('--paths', type=Path, required=True)
    args = parser.parse_args()
    if len(args.root) != len(set(args.root)):
        parser.error('duplicate Init root')
    try:
        started = time.monotonic()
        result = select(args) if args.operation == 'select' else verify(args)
        print(json.dumps({**result, 'seconds': round(time.monotonic() - started, 6)}, sort_keys=True))
    except (ValueError, KeyError, TypeError, OverflowError, OSError, StopIteration) as error:
        print(json.dumps(dict(result='refused', reason=str(error))))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
