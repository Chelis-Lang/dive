"""Prove current safety using a portable predicate on the set of present statuses."""

import json
from pathlib import Path

from induction import write_atomic

FORMAT = "dive-occupancy-support-v1"
MAX_BYTES = 1 << 20


def initial(model):
    return {1 << status if model.ids else 0 for status in model.initial}


def feasible(mask, count):
    return mask.bit_count() <= count and bool(mask) == bool(count)


def successors(model, mask):
    """All successor supports of *any* assignment with this support and size."""
    present = mask.bit_count()
    n = len(model.ids)
    result = set()
    for source, target, _ in model.tables[mask]:
        if source == target:
            result.add(mask)
            continue
        after = mask | (1 << target)
        # With two classes any excess can be assigned to the other class.
        # In a singleton support the departing class contains all n owners.
        if present > 1 or n == present:
            result.add(after & ~(1 << source))
        if n > present:
            result.add(after)
    return result


def closure(model):
    """Least successor-closed support set containing every current initial state."""
    seen = initial(model)
    pending = list(seen)
    while pending:
        for after in successors(model, pending.pop()) - seen:
            seen.add(after)
            pending.append(after)
    return seen


def verify(model, masks):
    if not initial(model) <= masks:
        return "Init inclusion"
    for mask in sorted(masks):
        if not feasible(mask, len(model.ids)):
            continue
        if ending := model.violations.get(mask):
            return ending.get("name", ending["kind"])
        if not successors(model, mask) <= masks:
            return "Next closure"
    return None


def store(path, model, masks):
    doc = {"format": FORMAT, "statuses": model.statuses, "masks": sorted(masks)}
    raw = (json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
    if len(raw) > MAX_BYTES:
        raise ValueError("support predicate exceeds the file limit")
    write_atomic(Path(path), raw)


def object_fields(pairs):
    fields = dict(pairs)
    if len(fields) != len(pairs):
        raise ValueError("duplicate support predicate field")
    return fields


def load(path, model):
    path = Path(path)
    if not path.is_file():
        raise ValueError("support predicate must be a regular file")
    with path.open("rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError("support predicate exceeds the file limit")
    try:
        doc = json.loads(raw, object_pairs_hook=object_fields)
    except RecursionError as error:
        raise ValueError("support predicate nesting limit") from error
    if not isinstance(doc, dict) or set(doc) != {"format", "statuses", "masks"} or doc["format"] != FORMAT:
        raise ValueError("unsupported support predicate format")
    statuses, masks = doc["statuses"], doc["masks"]
    if not (isinstance(statuses, list) and len(statuses) == len(model.statuses)
            and all(isinstance(s, str) for s in statuses) and set(statuses) == set(model.statuses)):
        raise ValueError("support predicate status labels differ")
    limit = 1 << len(statuses)
    if not (isinstance(masks, list) and len(masks) <= limit
            and all(type(m) is int and 0 <= m < limit for m in masks) and len(set(masks)) == len(masks)):
        raise ValueError("invalid or duplicate support mask")
    # A type expression can list the same literal labels in a different order.
    bits = [1 << model.statuses.index(s) for s in statuses]
    return {sum(bit for i, bit in enumerate(bits) if mask & (1 << i)) for mask in masks}
