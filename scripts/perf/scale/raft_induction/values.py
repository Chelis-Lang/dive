"""Finite functions, records, extensional sets and variable-length sequences for the SMT checker."""

from dataclasses import dataclass
import z3 as z


def require(condition, message):
    if not condition:
        raise ValueError(message)


@dataclass
class Set:
    cells: list  # (membership guard, element). Duplicate alternatives are extensional.


@dataclass
class Seq:
    length: object
    cells: list


@dataclass
class Tab:
    keys: tuple
    cells: tuple


@dataclass
class Rec:
    fields: dict


def con(*terms):
    terms = [t for t in terms if t is not True and not z.is_true(t)]
    return (
        False
        if any(t is False or z.is_false(t) for t in terms)
        else (z.And(*terms) if terms else True)
    )


def dis(*terms):
    terms = [t for t in terms if t is not False and not z.is_false(t)]
    return (
        True
        if any(t is True or z.is_true(t) for t in terms)
        else (z.Or(*terms) if terms else False)
    )


def neg(x):
    return not x if isinstance(x, bool) else z.Not(x)


def member(x, S):
    return dis(*(con(g, eq(x, y)) for g, y in S.cells))


def eq(x, y):
    if x is y:
        return True
    if isinstance(x, Set) and isinstance(y, Set):
        return con(
            *(
                dis(neg(g), member(v, t))
                for s, t in ((x, y), (y, x))
                for g, v in s.cells
            )
        )
    if isinstance(x, Seq) and isinstance(y, Seq):
        return con(
            x.length == y.length,
            *(
                dis(x.length <= i, eq(a, b))
                for i, (a, b) in enumerate(zip(x.cells, y.cells))
            ),
        )
    if isinstance(x, Rec) and isinstance(y, Rec):
        return x.fields.keys() == y.fields.keys() and con(
            *(eq(v, y.fields[k]) for k, v in x.fields.items())
        )
    if isinstance(x, Tab) and isinstance(y, Tab):
        return x.keys == y.keys and con(
            *(eq(a, b) for a, b in zip(x.cells, y.cells, strict=True))
        )
    if any(isinstance(v, (Set, Seq, Rec, Tab)) for v in (x, y)):
        raise ValueError("unsupported structural comparison")
    return x == y


def ite(g, a, b):
    if g is True or z.is_true(g):
        return a
    if g is False or z.is_false(g):
        return b
    if a is b:
        return a
    if isinstance(a, Tab) and isinstance(b, Tab) and a.keys == b.keys:
        return Tab(
            a.keys, tuple(ite(g, x, y) for x, y in zip(a.cells, b.cells, strict=True))
        )
    if isinstance(a, Set) and isinstance(b, Set):
        return Set(
            [(con(g, h), v) for h, v in a.cells]
            + [(con(neg(g), h), v) for h, v in b.cells]
        )
    if isinstance(a, Rec) and isinstance(b, Rec) and a.fields.keys() == b.fields.keys():
        return Rec({k: ite(g, v, b.fields[k]) for k, v in a.fields.items()})
    if isinstance(a, Seq) and isinstance(b, Seq):

        def val(x, i):
            return x.cells[i] if i < len(x.cells) else 0

        return Seq(
            ite(g, a.length, b.length),
            [
                ite(g, val(a, i), val(b, i))
                for i in range(max(len(a.cells), len(b.cells)))
            ],
        )
    return z.If(g, a, b)


def select(keys, values, k):
    if isinstance(k, int) and k in keys:
        return values[keys.index(k)]
    out = values[-1]
    for i in range(len(values) - 2, -1, -1):
        out = ite(eq(k, keys[i]), values[i], out)
    return out


def concrete(S):
    require(
        all((g is True and type(v) is int for g, v in S.cells)),
        "unsupported source shape: all((g is True and type(v) is int for g, v in S.cells))",
    )
    return tuple(v for _, v in S.cells)


def card(S):
    # a set builder/union can emit the same mathematical value repeatedly.
    counted = []
    total = 0
    for g, v in S.cells:
        h = con(g, neg(member(v, Set(counted))))
        total += ite(h, 1, 0)
        counted.append((g, v))
    return total
