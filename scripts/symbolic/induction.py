"""Transfer a Boolean state predicate. The receiver checks it against its own relation."""

import json
import os
from pathlib import Path
from tempfile import NamedTemporaryFile

MAX_BYTES = 16 << 20
MAX_NODES = 100_000


def store(path, model, states, receipt=None):
    if states.dag_size - 1 > MAX_NODES:
        raise ValueError("predicate exceeds the node limit")
    ids = {model.b.true: 1}
    nodes = []
    positions = {name: i for i, name in enumerate(model.variables)}

    def encode(node):
        if node.negated:
            return -encode(~node)
        if node not in ids:
            low = encode(node.low)
            high = encode(node.high)
            nodes.append([positions[node.var], low, high])
            ids[node] = len(nodes) + 1
        return ids[node]

    root = encode(states)
    doc = {"format": "finite-state-predicate-v1", "nodes": nodes, "root": root}
    if receipt is not None:
        signed = json.dumps({**doc, "closure_receipt": receipt}, separators=(",", ":")).encode("ascii") + b"\n"
        if len(signed) <= MAX_BYTES:
            write_atomic(path, signed)
            return
    data = json.dumps(doc, separators=(",", ":")).encode("ascii") + b"\n"
    if len(data) > MAX_BYTES:
        raise ValueError("predicate exceeds the file limit")
    write_atomic(path, data)


def write_atomic(path, data):
    """Publish the whole predicate or validated trace in one rename."""
    with NamedTemporaryFile(dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def load(path, model, *, include_receipt=False):
    with path.open("rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError("predicate exceeds the file limit")
    doc = json.loads(raw)
    if doc["format"] != "finite-state-predicate-v1" or len(doc["nodes"]) > MAX_NODES:
        raise ValueError("unsupported predicate format or size")
    nodes = [None, model.b.true]

    def resolve(ref):
        if type(ref) is not int or not 0 < abs(ref) < len(nodes):
            raise ValueError("invalid predicate edge")
        return nodes[ref] if ref > 0 else ~nodes[-ref]

    for bit, low, high in doc["nodes"]:
        if type(bit) is not int or not 0 <= bit < len(model.variables):
            raise ValueError("unknown predicate bit")
        nodes.append(model.b.ite(model.b.var(model.variables[bit]), resolve(high), resolve(low)))
    root = resolve(doc["root"])
    return (root, doc.get("closure_receipt")) if include_receipt else root


def verify(model, states):
    r"""Init => P, P /\ Constraint /\ Next => P', and P => each requested safety obligation."""
    if model.initial & ~states != model.b.false:
        return "Init inclusion"
    for ending, _ in model.violations(states):
        return ending.get("name", ending["kind"])
    for guard, partitions in model.actions:
        if model.image(states & model.constraint, guard, partitions) & ~states != model.b.false:
            return "Next closure"
    return None
