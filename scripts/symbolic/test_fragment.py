"""Small independent Core comparisons and rejection controls for the finite compiler."""

from itertools import product
import json


def check_fragment(folder, run, explicit, compare):
    module = folder / "Paint.tla"
    config = folder / "Paint.cfg"
    base = "CONSTANTS Colors={red,blue} A=red B=blue\nINIT Init\nNEXT Next\nINVARIANT TypeInvariant\n"
    config.write_text(base)
    template = r"""---- MODULE Paint ----
CONSTANTS Colors, A, B
VARIABLES x, got
TypeInvariant == x \in Colors /\ got \subseteq Colors
Init == x = A /\ got = {}
Next == \/ \E pick \in Colors : GUARD /\ x' = UPDATE /\ got' = STORE
        \/ UNCHANGED <<x, got>>
NeverBlue == B \notin got
Outside == x \notin Colors
====
"""
    check_initial_choices(folder, run, explicit, compare)

    model = template.replace("GUARD", "TRUE").replace("UPDATE", "x").replace("STORE", "got")
    module.write_text(model.replace("x = A", r"x \in Colors /\ x \in Colors"))
    (member,) = run(module, config)
    compare(member, explicit(module, config))
    # Change guards, value flow, and dynamic set membership independently.
    # All eight comparisons include overlapping self-loops and duplicate targets.
    for guard, step, store in product(
        ("TRUE", r"pick \notin got"),
        ("pick", "x"),
        (r"got \cup {pick}", "{x}"),
    ):
        module.write_text(template.replace("GUARD", guard).replace("UPDATE", step).replace("STORE", store))
        (row,) = run(module, config)
        compare(row, explicit(module, config))

    # TLC can enumerate overlapping arms inside a universal action guard.
    # When pick is A both arms hold. Evaluating the guard as one Boolean
    # would lose a generated witness. Only atomic bodies are admitted here.
    overlap = template.replace("GUARD", r"(\A q \in Colors : x = q \/ x \in Colors)")
    module.write_text(overlap.replace("UPDATE", "x").replace("STORE", "got"))
    (refused,) = run(module, config, code=2)
    assert refused["verdict"] == "unsupported"
    module.write_text(template.replace("GUARD", r"pick \notin got").replace("UPDATE", "x")
                      .replace("STORE", r"got \cup {pick}"))

    edited = folder / "bad-paint.cfg"
    edited.write_text(base + "INVARIANT Outside\n")
    _, violation = run(module, config, "--recheck", module, edited, code=1)
    ordinary = explicit(module, edited)
    assert ordinary["verdict"] == violation["verdict"] == "invariant-violation"
    assert violation["depth"] == len(violation["trace"]["states"]) == 1
    assert violation["trace_validated"] and violation["ending"]["name"] == "Outside"
    assert "generated" not in violation and "distinct" not in violation

    # A supplied Boolean tautology is a candidate, never a trusted result.
    candidate = folder / "predicate.json"
    candidate.write_text(json.dumps({"format": "finite-state-predicate-v1", "root": 1, "nodes": []}))
    (result,) = run(module, edited, "--verify-predicate", candidate, code=2)
    assert result["proof"] == "inconclusive"
    candidate.write_text(json.dumps({"format": "finite-state-predicate-v1", "root": 2, "nodes": [[0, 2, 1]]}))
    (result,) = run(module, config, "--verify-predicate", candidate, code=2)
    assert result["verdict"] == "unsupported" and "edge" in result["reason"]

    (stopped,) = run(module, config, "--time-limit", "0.000001", code=4)
    assert stopped["verdict"] == "stopped" and "proof" not in stopped
    (stopped,) = run(module, config, "--max-memory", "1", code=4)
    assert stopped["verdict"] == "stopped" and "proof" not in stopped
    (bounded,) = run(module, config, "--max-memory", "2048")
    compare(bounded, explicit(module, config))
    for cfg, reason in [
        (base + "CONSTRAINT NeverBlue\n", "safety invariants and deadlock"),
        (base + "ACTION_CONSTRAINT NeverBlue\n", "safety invariants and deadlock"),
        (base + "VIEW NeverBlue\n", "safety invariants and deadlock"),
        (base + "PROPERTY NeverBlue\n", "safety invariants and deadlock"),
    ]:
        edited.write_text(cfg)
        (refusal,) = run(module, edited, "--prove-inductive", code=2)
        assert refusal["verdict"] == "unsupported" and reason in refusal["reason"]


    check_action_guards(module, config, run, explicit, compare)


def check_action_guards(module, config, run, explicit, compare):
    # The pinned TLC and single-worker Core counts differ for a Boolean
    # choice before/after the last assignment or behind a defined operator.
    # Keep ordinary action alternatives, refuse pure choices in action guards.
    source = r"""---- MODULE Paint ----
CONSTANTS Colors, A, B
VARIABLES x, got
TypeInvariant == x \in Colors /\ got \subseteq Colors
Init == x = A /\ got = {}
G == TRUE \/ TRUE
Identity(p) == p
Twice(p) == p /\ p
Next == ACTION
====
"""
    keep = "UNCHANGED <<x, got>>"
    guard = r"(TRUE \/ TRUE)"
    some = r"(\E i \in Colors : i \in Colors)"
    for action, count, supported in [
        (rf"{keep} \/ {keep}", 3, True),
        (rf"\E i \in Colors : {keep}", 3, True),
        (rf"{keep} /\ ((TRUE \/ TRUE) = TRUE)", 2, True),
        (rf"{guard} /\ {keep}", 3, False),
        (rf"{some} /\ {keep}", 3, False),
        (rf"{keep} /\ {guard}", 2, False),
        (rf"{keep} /\ {some}", 2, False),
        (rf"G /\ {keep}", 2, False),
        (rf"([i \in Colors |-> {guard}])[A] /\ {keep}", 3, False),
        (rf"Identity({guard}) /\ {keep}", 3, False),
        (rf"Twice((x=x \/ x=x)) /\ {keep}", 5, False),
        (rf"(IF x=x THEN {guard} ELSE FALSE) /\ {keep}", 3, False),
        (rf"(x=x => {guard}) /\ {keep}", 3, False),
        (rf"(\A i \in Colors : {guard}) /\ {keep}", 5, False),
        (rf"UNCHANGED x /\ {guard} /\ UNCHANGED got", 3, False),
        (rf"UNCHANGED x /\ UNCHANGED got /\ {guard}", 2, False),
    ]:
        module.write_text(source.replace("ACTION", action))
        ordinary = explicit(module, config)
        assert ordinary["statistics"]["generated"] == count, (action, ordinary)
        (symbolic,) = run(module, config, code=0 if supported else 2)
        if supported:
            compare(symbolic, ordinary)
        else:
            assert symbolic["verdict"] == "unsupported" and "action guard" in symbolic["reason"]


def check_initial_choices(folder, run, explicit, compare):
    module = folder / "Roles.tla"
    module.write_text(r"""---- MODULE Roles ----
EXTENDS TLC
CONSTANTS Roleset, Colors, A, B
VARIABLE chosen
TypeInvariant == chosen \in [Roleset -> Colors]
Init == \E r \in Roleset : chosen = [u \in Roleset |-> IF u=r THEN A ELSE B]
Duplicates == Init \/ Init
Next == UNCHANGED chosen
Sym == Permutations(Roleset)
NoA == \A u \in Roleset: chosen[u] = B
====
""")
    base = "CONSTANTS Roleset={r1,r2} Colors={red,blue} A=red B=blue\nNEXT Next\nINVARIANT TypeInvariant\n"
    for symmetry in (False, True):
        paths = []
        for init in ("Init", "Duplicates"):
            cfg = folder / f"Roles-{init}-{symmetry}.cfg"
            cfg.write_text(base + f"INIT {init}\n" + ("SYMMETRY Sym\n" if symmetry else ""))
            paths.append(cfg)
            (row,) = run(module, cfg)
            compare(row, explicit(module, cfg))
            # Pinned TLC counts Init before deduplication, even with symmetry.
            distinct = 1 if symmetry else 2
            initial = 2 if init == "Init" else 4
            assert (row["distinct"], row["generated"], row["depth"]) == (distinct, initial + distinct, 1)
        first, second = run(module, paths[0], "--recheck", module, paths[1])
        assert not second["reused"] and second["generated"] == first["generated"] + 2
    cfg = folder / "Roles-bad.cfg"
    cfg.write_text(base + "INIT Init\nINVARIANT NoA\n")
    (bad,) = run(module, cfg, code=1)
    assert bad["depth"] == 1 and bad["trace_validated"] and bad["ending"]["name"] == "NoA"
