"""Adversarial controls for extensional record keys and quantified refutations."""

import argparse
from pathlib import Path
import tempfile

import z3

from channels import Channel, ChannelModel
from check import bind
from ground import Ground, nnf
from representation import representation
from values import Rec, Seq, Set, require


def solve(formula):
    solver = z3.Solver()
    solver.add(formula)
    result = solver.check()
    require(result in (z3.sat, z3.unsat), "unexpected SMT unknown in control")
    return result


def test_quantifiers():
    # A nested existential may require a DIFFERENT value for each universal
    # binding. Giving it a single constant silently makes the original SAT
    # assertion unsatisfiable. The Skolem function must take the bound value.
    x, y = z3.Ints("control_x control_y")
    for formula, expected in [
        (z3.ForAll(x, z3.Exists(y, y == x + 1)), z3.sat),
        (z3.Exists(y, z3.ForAll(x, y == x + 1)), z3.unsat),
        (z3.And(z3.Exists(x, x == 0), z3.Exists(x, x == 1)), z3.sat),
        (z3.Not(z3.Exists(x, x != x)), z3.sat),
    ]:
        require(
            solve(formula) == expected and solve(nnf(formula)) == expected,
            "Skolem scope or polarity changed satisfiability",
        )
    ground = Ground({}, [z3.IntVal(1) == 1])
    try:
        ground.reject(z3.Exists(x, x == 7), "satisfiable control")
    except ValueError as error:
        require("sat" in str(error), "wrong satisfiable-control error")
    else:
        raise ValueError("a satisfiable failed obligation was discharged")


def test_records(doc):
    model = ChannelModel(doc)
    state, _ = representation(model, Channel)
    request = state["appendEntriesRequestMsgs"]
    tag = model.symbols

    def record(previous=0, padding=9, dest="n2", length=0):
        return Rec(
            dict(
                mtype=tag["AppendEntriesRequest"],
                mterm=1,
                mprevLogIndex=previous,
                mprevLogTerm=0,
                mentries=Seq(length, [padding]),
                mcommitIndex=0,
                msource=tag["n1"],
                mdest=tag[dest],
            )
        )

    original = record()
    same = record(padding=11)
    second = record(dest="n3")
    third = record(previous=1)
    empty = z3.ForAll(
        request.variables(message := request.arbitrary()),
        z3.Not(request.contains(message)),
    )
    image = request.update(
        True, Set([(True, r) for r in (original, same, second, third)])
    )
    require(
        solve(z3.And(empty, z3.Not(image.cardinality_equals(3, 8000)))) == z3.unsat,
        "duplicate or missing live messages",
    )
    require(
        solve(z3.And(empty, image.cardinality_equals(2, 8000))) == z3.unsat,
        "three live messages fit into two",
    )
    removed = image.update(False, Set([(True, same)]))
    require(
        solve(
            z3.And(
                empty,
                z3.Or(
                    removed.contains(original),
                    z3.Not(removed.contains(second)),
                    z3.Not(removed.contains(third)),
                ),
            )
        )
        == z3.unsat,
        "one deletion changed unrelated records",
    )
    concrete = record(length=1, padding=1)

    def changed(**fields):
        return Rec({**concrete.fields, **fields})

    for other in (
        changed(mterm=2),
        changed(mprevLogIndex=1),
        changed(mprevLogTerm=1),
        changed(mcommitIndex=1),
        changed(msource=tag["n3"]),
        changed(mdest=tag["n3"]),
        record(),
        record(length=1, padding=2),
    ):
        require(
            solve(z3.And(request.contains(concrete), z3.Not(request.contains(other))))
            == z3.sat,
            "dynamic record field missing from key",
        )
    require(
        solve(z3.Xor(request.contains(original), request.contains(same))) == z3.unsat,
        "empty sequence padding leaked into the membership key",
    )


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("--exporter", required=True, type=Path)
    args = p.parse_args()
    with tempfile.TemporaryDirectory() as t:
        doc, _, _ = bind(args.source, args.exporter.resolve(), 3, Path(t), 1000, 3, 3)
        test_quantifiers()
        test_records(doc)
    print(
        "quantifier polarity, Skolem scope, canonical dynamic fields, cardinality, retention and deletion passed"
    )


if __name__ == "__main__":
    main()
