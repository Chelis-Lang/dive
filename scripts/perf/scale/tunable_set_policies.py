"""Check the reconstructed bounded FIFO policy obligations on one symbolic element."""

import argparse
import hashlib
import json
import re
from pathlib import Path

import z3

BOUND_SOURCES = {
    "TunableSetHistory.tla": "c59b7a02af26badd2a522c8384ba0f99abb7dca58c342a5f2c507e157c2ce88f",
    "TunableSetPolicies.tla": "aa5d632daeeb98e498b3b3354f54dd8d5cbbf35324e53e89b13d86c78468eb7e",
}


def read_policy(source, edited):
    """Bind every guard and allow only the supported pure branch expressions."""
    def sections(text):
        # A renamed research module may be checked alongside its original.
        text, matches = re.subn(r"^---- MODULE [A-Za-z][A-Za-z0-9_]* ----",
                                "---- MODULE BoundPolicies ----", text, count=1)
        if matches != 1:
            raise ValueError("missing policy module header")
        start = text.index("CASE owner.policy = 3 ->")
        end = text.index("Maximal(S) ==", start)
        cases = []
        for arm in text[start + len("CASE "):end].split("[]"):
            guard, result = arm.split("->", 1)
            cases.append((" ".join(guard.split()), " ".join(result.split())))
        return text[:start], cases, text[end:]

    prefix, original, suffix = sections(source)
    new_prefix, revised, new_suffix = sections(edited)
    if (new_prefix, new_suffix) != (prefix, suffix) or len(original) != len(revised):
        raise ValueError("change outside the five supported policy branches")
    modes = []
    for i, ((guard, expression), (new_guard, body)) in enumerate(zip(original, revised, strict=True)):
        if new_guard != guard:
            raise ValueError("changed policy guard")
        if body == expression:
            modes.append("source")
        elif body in ("FALSE", "TRUE"):
            modes.append(body)
        elif i == 0 and body == expression.replace("a.stamp > b.stamp", "a.stamp < b.stamp"):
            modes.append("earliest")
        else:
            raise ValueError(f"unsupported expression for {guard}")
    return modes


def policy_obligations(size, modes):
    stamps = range(size)
    before = [z3.Bool(f"present_{i}") for i in stamps]
    focus = [z3.Bool(f"same_element_{i}") for i in stamps]
    add = [z3.Bool(f"add_{i}") for i in stamps]
    policy = [z3.Int(f"policy_{i}") for i in stamps]
    new = z3.Int("incoming_stamp")
    hb = [[z3.Bool(f"earlier_{i}_{j}") if i < j else z3.BoolVal(False)
           for j in stamps] for i in stamps]
    solver = z3.Solver()
    solver.add(new >= 0, new < size)
    for i in stamps:
        solver.add(policy[i] >= 1, policy[i] <= 3, z3.Implies(new == i, z3.Not(before[i])))
        for j in range(i + 1, size):
            # Each replica receives in global issue order, excluding its own already-known issues.
            solver.add(z3.Implies(z3.And(hb[i][j], z3.Or(before[j], new == j)), before[i]))
            for k in range(j + 1, size):
                solver.add(z3.Implies(z3.And(hb[i][j], hb[j][k]), hb[i][k]))

    def applies(owner, a, b):
        guards = [policy[owner] == 3, z3.And(policy[owner] == 1, add[owner]),
                  z3.And(policy[owner] == 2, z3.Not(add[owner])),
                  z3.And(policy[owner] == 2, add[owner])]
        guards.append(z3.Not(z3.Or(guards)))
        bodies = [
            z3.And(policy[a] == 3, z3.Or(policy[b] != 3, a < b if modes[0] == "earliest" else a > b)),
            z3.And(add[a], z3.Not(add[b]), policy[b] == 1),
            z3.And(z3.Not(add[a]), add[b], policy[b] == 1),
            z3.And(add[a], z3.Not(add[b]), policy[b] != 3),
            False,
        ]
        terms = []
        for guard, mode, body in zip(guards, modes, bodies, strict=True):
            if mode == "TRUE":
                terms.append(guard)
            elif mode != "FALSE":
                terms.append(z3.And(guard, body))
        return z3.Or(terms)

    def project(membership):
        maximal = [z3.And(membership[b], z3.Not(z3.Or([
            z3.And(membership[a], hb[b][a]) for a in stamps
        ]))) for b in stamps]
        active = [z3.And(maximal[b], z3.Not(z3.Or([
            z3.And(maximal[a], z3.Not(hb[a][b]), z3.Not(hb[b][a]),
                   z3.Or(applies(b, a, b), applies(a, a, b))) for a in stamps if a != b
        ]))) for b in stamps]
        return maximal, active

    previous = [z3.And(before[i], focus[i]) for i in stamps]
    following = [z3.And(z3.Or(before[i], new == i), focus[i]) for i in stamps]
    _, old_active = project(previous)
    maxima, active = project(following)

    def contains(values, kinds):
        return z3.Or([z3.And(values[i], kinds[i]) for i in stamps])

    def incoming(values):
        return z3.Or([z3.And(new == i, values[i]) for i in stamps])

    remove = [z3.Not(a) for a in add]
    lookup, removes = contains(active, add), contains(active, remove)
    old_lookup, old_removes = contains(old_active, add), contains(old_active, remove)
    maximal_x, active_x, add_x, focus_x = map(incoming, [maxima, active, add, focus])
    seen_later = z3.Or([z3.And(new == x, old_active[y], hb[x][y]) for x in stamps for y in stamps])
    related_max = z3.Or([z3.And(new == x, maxima[y], z3.Or(hb[x][y], hb[y][x]))
                         for x in stamps for y in stamps])
    obligations = {
        "no_conflicting_kinds": z3.Not(z3.And(lookup, removes)),
        "nonempty_active_for_every_used_element": z3.Implies(z3.Or(following), z3.Or(active)),
        "unchanged_other_elements": z3.Implies(z3.Not(focus_x), z3.And([
            a == b for a, b in zip(active, old_active, strict=True)
        ])),
        "PSE": z3.Implies(focus_x, z3.Or(
            z3.And(z3.Not(maximal_x), seen_later, old_lookup == lookup),
            z3.And(maximal_x, z3.Not(related_max), z3.Or(z3.Not(active_x), add_x == lookup)),
        )),
        "PPE_figure_E3": z3.Implies(focus_x, z3.Or(
            z3.Implies(z3.And(add_x, old_lookup), z3.And(old_lookup, lookup)),
            z3.Implies(z3.And(z3.Not(add_x), old_removes), z3.And(z3.Not(old_lookup), z3.Not(lookup))),
        )),
        "PCS": z3.Implies(focus_x, z3.Or(z3.And(z3.Not(removes), lookup),
                                         z3.And(z3.Not(contains(active, add)), z3.Not(lookup)))),
        "only_one_kind_in_history": z3.Implies(z3.Or(following), z3.And(
            z3.Implies(z3.Not(contains(following, remove)), lookup),
            z3.Implies(z3.Not(contains(following, add)), z3.Not(lookup)),
        )),
    }
    results = {}
    for name, holds in obligations.items():
        solver.push()
        solver.add(z3.Not(holds))
        result = solver.check()
        results[name] = str(result)
        solver.pop()
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("replicas", type=int)
    parser.add_argument("operations_per_replica", type=int)
    parser.add_argument("--policy-file", type=Path, help="check a supported edit to the policy module")
    args = parser.parse_args()
    if min(args.replicas, args.operations_per_replica) < 1:
        parser.error("replicas and operations per replica must be positive")
    size = args.replicas * args.operations_per_replica
    for filename, expected in BOUND_SOURCES.items():
        if hashlib.sha256(Path(__file__).with_name(filename).read_bytes()).hexdigest() != expected:
            parser.error(f"bound reconstruction changed: {filename}")
    base = Path(__file__).with_name("TunableSetPolicies.tla")
    path = args.policy_file or base
    if args.policy_file:
        # The extending module must resolve the same parent beside the supplied copy.
        parent = path.with_name("TunableSetHistory.tla")
        if not parent.is_file() or hashlib.sha256(parent.read_bytes()).hexdigest() != BOUND_SOURCES[parent.name]:
            parser.error("missing or changed parent transition module")
    source = path.read_bytes()
    try:
        modes = read_policy(base.read_text(), source.decode("utf-8"))
    except ValueError as exc:
        parser.error(str(exc))
    results = policy_obligations(size, modes)
    print(json.dumps({"bound": size, "solver": z3.get_version_string(),
                      "policy_sha256": hashlib.sha256(source).hexdigest(), "branches": modes,
                      "counterexample_queries": results}))
    return int(any(v != "unsat" for v in results.values()))


if __name__ == "__main__":
    raise SystemExit(main())
