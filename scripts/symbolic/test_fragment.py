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
    check_record_permutations(folder, run, explicit, compare)
    check_initial_choices(folder, run, explicit, compare)
    check_initial_enumeration(folder, run)
    check_constraints(folder, run, explicit, compare)
    check_refined_domains(folder, run, explicit, compare)
    check_refined_edits(folder, run, explicit, compare)
    check_layer_edits(folder, run, explicit, compare)

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
        (base + "ACTION_CONSTRAINT NeverBlue\n", "safety invariants and deadlock"),
        (base + "VIEW NeverBlue\n", "safety invariants and deadlock"),
        (base + "PROPERTY NeverBlue\n", "safety invariants and deadlock"),
    ]:
        edited.write_text(cfg)
        (refusal,) = run(module, edited, "--prove-inductive", code=2)
        assert refusal["verdict"] == "unsupported" and reason in refusal["reason"]


    check_action_guards(module, config, run, explicit, compare)
    check_nested_tables(folder, run, explicit, compare)


def check_action_guards(module, config, run, explicit, compare):
    # The pinned TLC counts depend on assignment progress and on whether
    # a disjunction is evaluated as a predicate or expanded as an action.
    source = r"""---- MODULE Paint ----
CONSTANTS Colors, A, B
VARIABLES x, got
TypeInvariant == x \in Colors /\ got \subseteq Colors
Init == x = A /\ got = {}
G == TRUE \/ TRUE
StateGuard == x = x \/ x = x
Identity(p) == p
Forced(p) == /\ p = TRUE
             /\ p
             /\ UNCHANGED <<x, got>>
Twice(p) == p /\ p
Next == ACTION
====
"""
    keep = "UNCHANGED <<x, got>>"
    guard = r"(TRUE \/ TRUE)"
    state_guard = r"(x = A \/ x \in Colors)"
    some = r"(\E i \in Colors : i \in Colors)"
    for action, count, supported in [
        (rf"{keep} \/ {keep}", 3, True),
        (rf"{state_guard} /\ {keep}", 3, True),
        (rf"UNCHANGED x /\ ({state_guard} /\ UNCHANGED got)", 3, True),
        (rf"({state_guard} /\ {keep}) \/ {keep}", 4, True),
        (rf"{keep} /\ {state_guard}", 2, False),
        (rf"StateGuard /\ {keep}", 3, False),
        (rf"({state_guard} = TRUE) /\ {keep}", 2, True),
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


    # TLC counts one successor after forcing p, even though both actual arms
    # hold. Core's count for this lazy formal is tracked separately (#1166).
    module.write_text(source.replace("ACTION", rf"Forced({state_guard})"))
    for flags in ([], ["--refine-domains"]):
        (refusal,) = run(module, config, *flags, code=2)
        assert "unsupported action guard: Local" in refusal["reason"]

    double = module.with_name("Repeated.tla")
    double.write_text(source.replace("MODULE Paint", "MODULE Repeated")
                      .replace("ACTION", rf"{state_guard} /\ {keep}"))
    module.write_text(source.replace("ACTION", keep))
    for flags in ([], ["--refine-domains"]):
        session = run(module, config, *flags, "--recheck", double, config, "--recheck", module, config)
        assert [(row["distinct"], row["generated"], row["depth"]) for row in session] == [
            (1, 2, 1), (1, 3, 1), (1, 2, 1)
        ]


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


def check_initial_enumeration(folder, run):
    module = folder / "Choice.tla"
    cfg = folder / "Choice.cfg"
    cfg.write_text("CONSTANTS C = {1,0,1} Replacement = {0,1,0}\n"
                   "INIT Init\nNEXT Next\nINVARIANT TypeInvariant\n")
    source = r"""---- MODULE Choice ----
CONSTANT C
VARIABLE x
Replacement == {99}
Product == [a : {0,1}]
Lazy == [a : {0}] \cup Product
Identity(p) == p
TypeInvariant == x \in TYPE
Init == x \in (DOMAIN)
Next == ACTION
====
"""
    double = r"(x = x \/ x = x) /\ x' = x"
    # With the overlapping action, pinned TLC reports 7/2/1 for Lazy and
    # 6/2/1 for the explicit two-record set (generated/distinct/depth).
    # Each safe domain below enumerates two distinct elements exactly once.
    for typ, domain in [
        ("Product", "{[a |-> 0], [a |-> 1]}"),
        ("Product", "{[a |-> 0], [a |-> 1], [a |-> 0]}"),
        ("Product", r"[a : {1,0,1}]"),
        ("Product", r"[a : {0}] \cup [a : {1}]"),
        ("Product", r"{v : v \in Lazy}"),
        ("Product", r"Product \cap Lazy"),
        ("{0,1}", r"Identity(LET S == {1,1,0} IN S)"),
        ("{0,1}", r"C \cup Replacement"),
        ("{0,1}", r"{v \in {1,0,1} : TRUE}"),
    ]:
        module.write_text(source.replace("TYPE", typ).replace("DOMAIN", domain).replace("ACTION", double))
        for flags in ([], ["--refine-domains"]):
            (row,) = run(module, cfg, *flags)
            assert (row["generated"], row["distinct"], row["depth"]) == (6, 2, 1), (domain, row)

    for typ, domain, action in [
        ("Product", r"[a : {0}] \cup Product", "UNCHANGED x"),  # pinned TLC 5/2/1
        ("Product", "Lazy", double),
        ("Product", "Identity(LET S == Lazy IN S)", double),
        ("Product", "IF TRUE THEN Lazy ELSE Product", double),
        ("Product", r"[i \in {0} |-> Lazy][0]", double),
        ("Product", r"{v \in Lazy : TRUE}", double),
        ("Product", r"Lazy \cap Product", double),
        ("Product", r"Lazy \ {}", double),
        ("[z : Product]", "[z : Lazy]", double),
    ]:
        module.write_text(source.replace("TYPE", typ).replace("DOMAIN", domain).replace("ACTION", action))
        for flags in ([], ["--refine-domains"]):
            (row,) = run(module, cfg, *flags, code=2)
            assert "Init membership may enumerate duplicate witnesses" in row["reason"], (domain, row)


def check_constraints(folder, run, explicit, compare):
    # A filtered state counts as generated and is checked for invariant
    # violations, although it will never be a source or a deadlock.
    src = folder / "Limits.tla"
    cfg = folder / "Limits.cfg"
    src.write_text(r"""---- MODULE Limits ----
EXTENDS Integers, FiniteSets
CONSTANT Cap
VARIABLE x
ASSUME Cap <= 2
TypeInvariant == x \in 0..2
Init == x \in {0, 1}
Single == x = 0
Next == x' = IF x = 0 THEN 1 ELSE IF x = 1 THEN 2 ELSE x
Keep == x = Cap
Bad == x = 0
BelowTwo == x < 2
Wider == x <= 1
NoneKept == x > 1
Mixed == x # "zero"
====
""")
    def config(init="Init", constraint="Keep", prop="", cap=0):
        return (f"CONSTANT Cap = {cap}\nINIT {init}\nNEXT Next\nINVARIANT TypeInvariant\n"
                f"CONSTRAINT {constraint}\n" + (f"INVARIANT {prop}\n" if prop else ""))

    cfg.write_text(config())
    (passed,) = run(src, cfg)
    compare(passed, explicit(src, cfg))
    assert (passed["generated"], passed["distinct"], passed["depth"]) == (3, 1, 1)
    empty = folder / "empty-limit.cfg"
    empty.write_text(config(constraint="NoneKept"))
    _, changed = run(src, cfg, "--recheck", src, empty)
    compare(changed, explicit(src, empty))
    assert not changed["reused"] and (changed["generated"], changed["distinct"], changed["depth"]) == (2, 0, 1)
    # The BDD for both initial states is smaller than the BDD for x=0.
    # Expanding the excluded x=1 would incorrectly visit x=2 and falsify BelowTwo.
    cfg.write_text(config(prop="BelowTwo"))
    (bounded,) = run(src, cfg)
    compare(bounded, explicit(src, cfg))
    assert bounded["distinct"] == 1 and bounded["generated"] == 3
    for init, depth in [("Init", 1), ("Single", 2)]:
        cfg.write_text(config(init))
        edited = folder / "bad-limit.cfg"
        edited.write_text(config(init, prop="Bad"))
        _, failed = run(src, cfg, "--recheck", src, edited, code=1)
        assert failed["reused"] and failed["trace_validated"]
        assert failed["ending"]["name"] == "Bad" and failed["depth"] == depth
        assert explicit(src, edited)["verdict"] == "invariant-violation"
        (proof,) = run(src, edited, "--prove-inductive", code=2)
        assert proof["proof"] == "inconclusive"
    predicate = folder / "boundary.json"
    cfg.write_text(config("Single", prop="BelowTwo"))
    run(src, cfg, "--write-predicate", predicate)
    (proved,) = run(src, cfg, "--verify-predicate", predicate)
    assert proved["proof"] == "inductive-safety"
    cfg.write_text(config("Single", constraint="Wider", prop="BelowTwo"))
    (inconclusive,) = run(src, cfg, "--verify-predicate", predicate, code=2)
    assert inconclusive["proof"] == "inconclusive"
    (failed,) = run(src, cfg, code=1)
    assert failed["trace_validated"] and failed["depth"] == 3
    cfg.write_text(config(prop="Mixed"))
    (mixed,) = run(src, cfg, code=2)
    assert mixed["verdict"] == "unsupported" and "incomparable" in mixed["reason"]
    cfg.write_text(config(cap=3))
    (refusal,) = run(src, cfg, code=2)
    assert refusal["verdict"] == "unsupported" and "ASSUME" in refusal["reason"]

    src.write_text(r"""---- MODULE Limits ----
EXTENDS Naturals, FiniteSets
VARIABLE net
Msg == [n : 0..1, key : {"a"}]
TypeInvariant == net \subseteq Msg
Init == net = {}
Next == \/ \E m \in Msg : net' = net \cup {m}
        \/ UNCHANGED net
Capacity == Cardinality(net) <= 1
NoOne == [key |-> "a", n |-> 1] \notin net
====
""")
    cfg.write_text("INIT Init\nNEXT Next\nINVARIANT TypeInvariant\nCONSTRAINT Capacity\n")
    (net,) = run(src, cfg)
    compare(net, explicit(src, cfg))
    assert (net["distinct"], net["generated"], net["depth"]) == (3, 10, 2)
    cfg.write_text(cfg.read_text() + "INVARIANT NoOne\n")
    (bad,) = run(src, cfg, code=1)
    assert bad["ending"]["name"] == "NoOne" and bad["trace_validated"]


def check_refined_domains(folder, run, explicit, compare):
    module = folder / "Mail.tla"
    config = folder / "Mail.cfg"
    module.write_text(r"""---- MODULE Mail ----
EXTENDS Naturals, FiniteSets
VARIABLES mail, step
Letter == [key : 0..299, body : {"v"}]
TypeInvariant == /\ mail \subseteq Letter /\ step \in 0..2
Message(k) == [key |-> k, body |-> "v"]
Init == /\ mail = {Message(0)} /\ step = 0
Next == \/ /\ step < 2
           /\ LET k == step + 1 IN mail' = mail \cup {Message(k)}
           /\ step' = step + 1
        \/ /\ step = 2 /\ UNCHANGED <<mail, step>>
Small == Cardinality(mail) <= 2
Safe == Message(2) \notin mail
Escapes == /\ mail' = {Message(300)} /\ UNCHANGED step
====
""")
    common = "INIT Init\nINVARIANT TypeInvariant\nCONSTRAINT Small\n"
    config.write_text(common + "NEXT Next\n")
    flags = ("--refine-domains",)
    (row,) = run(module, config, *flags)
    compare(row, explicit(module, config))
    assert row["learned_members"] == 3 and row["refinement_rounds"] >= 3
    assert (row["generated"], row["distinct"], row["depth"]) == (3, 2, 2)
    edited = folder / "edited.cfg"
    edited.write_text(common + "NEXT Next\nINVARIANT Safe\n")
    cold, warm, bad, fresh = run(module, config, *flags, "--recheck", module, config,
                                  "--recheck", module, edited, "--recheck", module, config, code=1)
    for success in (cold, warm, fresh):
        compare(success, explicit(module, config))
        assert "domain_members" not in success
    assert not cold["reused"] and warm["reused"] and not fresh["reused"] and not bad["reused"]
    assert warm["refinement_rounds"] == 1 and warm["initial_hint_members"] == 3
    assert fresh["initial_hint_members"] == 0 and fresh["refinement_rounds"] == cold["refinement_rounds"]
    assert bad["ending"]["name"] == "Safe" and bad["depth"] == 3 and bad["trace_validated"]
    first_trace = folder / "first-failure.json"
    initial_bad = folder / "initial-failure.cfg"
    initial_bad.write_text(common + "NEXT Next\nINVARIANT NoZero\n")
    module.write_text(module.read_text().replace("====", r"NoZero == Message(0) \notin mail" + "\n===="))
    first, second = run(module, edited, *flags, "--recheck", module, initial_bad,
                        "--trace", first_trace, code=1)
    assert first["trace_file"] == str(first_trace) and first["ending"]["name"] == "Safe"
    assert json.loads(first_trace.read_text())["ending"]["name"] == "Safe"
    assert second["trace"]["ending"]["name"] == "NoZero" and "trace_file" not in second
    assert first["trace_validated"] and second["trace_validated"]
    shifted = folder / "Shifted.tla"
    shifted.write_text(module.read_text().replace("MODULE Mail", "MODULE Shifted")
                       .replace("0..299", "300..599").replace("Message(0)", "Message(300)")
                       .replace("step + 1 IN", "step + 301 IN"))
    _, disjoint = run(module, config, *flags, "--recheck", shifted, config)
    compare(disjoint, explicit(shifted, config))
    assert disjoint["initial_hint_members"] == 0 and disjoint["learned_members"] == 3
    edited.write_text(common + "NEXT Escapes\n")
    _, refused = run(module, config, *flags, "--recheck", module, edited, code=2)
    assert refused["verdict"] == "unsupported" and "outside finite layout" in refused["reason"]
    config.write_text(common + "NEXT Next\nINVARIANT Safe\n")
    (bad,) = run(module, config, *flags, code=1)
    assert bad["ending"]["name"] == "Safe" and bad["depth"] == 3 and bad["trace_validated"]
    config.write_text(common + "NEXT Escapes\n")
    (outside,) = run(module, config, *flags, code=2)
    assert outside["verdict"] == "unsupported" and "outside finite layout" in outside["reason"]
    (incompatible,) = run(module, config, *flags, "--prove-inductive", code=2)
    assert incompatible["verdict"] == "unsupported"


def check_refined_edits(folder, run, explicit, compare):
    module = folder / "Revisions.tla"
    module.write_text(r"""---- MODULE Revisions ----
EXTENDS Naturals
VARIABLES x, mail
TypeInvariant == /\ x \in 0..1 /\ mail \subseteq 0..299
Init == /\ x \in 0..1 /\ mail = {0}
Twice == Init \/ Init
Keep == x = 0
Allow == x \in 0..1
Safe == 2 \notin mail
OnlyZero == x = 0
Stay == UNCHANGED <<x, mail>>
Duplicate == Stay \/ Stay
Sink == /\ x = 0 /\ x' = 1 /\ UNCHANGED mail
SinkTwice == Sink \/ Sink
StutterOnZero == /\ x' = 0 /\ UNCHANGED mail
Blocked == /\ x = 1 /\ UNCHANGED <<x, mail>>
Add == /\ mail' = mail \cup {1} /\ UNCHANGED x
Escape == /\ mail' = mail \cup {399} /\ UNCHANGED x
====
""")

    def cfg(name, init="Init", step="Stay", constraint="Keep", prop="", deadlock=True):
        path = folder / (name + ".cfg")
        path.write_text(f"INIT {init}\nNEXT {step}\nCONSTRAINT {constraint}\nINVARIANT TypeInvariant\n"
                        + (f"INVARIANT {prop}\n" if prop else "")
                        + ("" if deadlock else "CHECK_DEADLOCK FALSE\n"))
        return path

    base = cfg("base")
    stronger = cfg("stronger", prop="Safe")
    duplicates = cfg("duplicate-actions", step="Duplicate")
    double_init = cfg("duplicate-init", init="Twice", step="Duplicate")
    all_states = cfg("all-states", init="Twice", step="Duplicate", constraint="Allow")
    extra = cfg("new-emission", step="Add")
    # Each edit with the same closure but changed multiplicities still needs its own counts.
    edits = [base, stronger, duplicates, double_init, all_states, extra]
    rows = run(module, base, "--refine-domains", *(arg for path in edits[1:] for arg in ("--recheck", module, path)))
    for row, config in zip(rows, edits, strict=True):
        compare(row, explicit(module, config))
    assert [r["reused"] for r in rows] == [False, True, True, True, False, False]
    assert rows[-1]["learned_members"] == 2

    # The admitted x=0 makes the assignment x'=0 a self-loop. Sink emits the
    # excluded x=1; its second witness changes only the generated count.
    sequence = [base, cfg("guarded-loop", step="StutterOnZero"),
                cfg("outgoing", step="Sink"), cfg("twice-outgoing", step="SinkTwice")]
    rows = run(module, sequence[0], "--refine-domains",
               *(arg for path in sequence[1:] for arg in ("--recheck", module, path)))
    for row, config in zip(rows, sequence, strict=True):
        compare(row, explicit(module, config))
    assert [row["reused"] for row in rows] == [False, True, False, True]
    _, removed_last = run(module, base, "--refine-domains", "--recheck", module,
                          cfg("lose-last-loop", step="Blocked"), code=1)
    assert removed_last["verdict"] == "deadlock" and removed_last["trace_validated"]
    assert not removed_last["reused"]

    # No admitted predecessor reaches the excluded initial x=1.
    _, excluded = run(module, base, "--refine-domains", "--recheck", module, cfg("initial-bad", prop="OnlyZero"), code=1)
    assert excluded["trace_validated"] and excluded["depth"] == 1 and not excluded["reused"]
    for step, code in [("Sink", 0), ("Blocked", 1)]:
        _, row = run(module, cfg(step + "-off", step=step, deadlock=False), "--refine-domains",
                     "--recheck", module, cfg(step + "-on", step=step), code=code)
        if code:
            assert row["verdict"] == "deadlock" and row["trace_validated"] and not row["reused"]
        else:
            assert row["verdict"] == "success" and row["reused"]

    _, escaped = run(module, base, "--refine-domains", "--recheck", module, cfg("escape", step="Escape"),
                     "--recheck", module, base, code=2)
    assert escaped["verdict"] == "unsupported" and "outside finite layout" in escaped["reason"]
    expanded = folder / "Expanded.tla"
    expanded.write_text(module.read_text().replace("MODULE Revisions", "MODULE Expanded").replace("0..299", "0..300"))
    _, row = run(module, base, "--refine-domains", "--recheck", expanded, base)
    compare(row, explicit(expanded, base))
    assert not row["reused"] and row["initial_hint_members"] == 1


def check_layer_edits(folder, run, explicit, compare):
    module = folder / "LayerEdits.tla"
    module.write_text(r"""---- MODULE LayerEdits ----
EXTENDS Integers
VARIABLES x, mail
TypeInvariant == /\ x \in 0..6 /\ mail \subseteq 0..1
Init == /\ x = 0 /\ mail = {0}
Step == /\ x # 5 /\ x' = IF x=0 THEN 1 ELSE IF x=1 THEN 2 ELSE IF x=2 THEN 3 ELSE IF x=3 THEN 4 ELSE 5
Reset == x' = 0
Late == /\ x=3 /\ x'=6
Shortcut == /\ x=1 /\ x'=4
Walk == (Step \/ UNCHANGED x) /\ UNCHANGED mail
Back == Walk \/ (Reset /\ UNCHANGED mail)
Jump == Back \/ (Shortcut /\ UNCHANGED mail)
Grow == Jump \/ (Late /\ UNCHANGED mail)
EmitSix == Walk \/ (x=0 /\ x'=6 /\ UNCHANGED mail)
Keep == x # 6
Safe == x # 6
BeforeFive == x # 5
====
""")
    configs = []
    for name in ("Walk", "Back", "Jump", "Grow", "Back", "Walk"):
        cfg = folder / (name + "-layers.cfg")
        cfg.write_text(f"INIT Init\nNEXT {name}\nINVARIANT TypeInvariant\n")
        configs.append(cfg)
    rows = run(module, configs[0], *(arg for cfg in configs[1:] for arg in ("--recheck", module, cfg)))
    for row, cfg in zip(rows, configs, strict=True):
        compare(row, explicit(module, cfg))
    assert [r["depth"] for r in rows] == [6, 6, 4, 5, 6, 6]
    assert rows[1]["reused_layer_images"] == 6 and rows[2]["reused_layer_images"] == 4
    assert rows[3]["distinct"] == 7 and rows[4]["distinct"] == 6
    assert rows[4]["reused_layer_images"] == 1 and rows[5]["reused_layer_images"] == 6

    passed = folder / "FilteredWalk.cfg"
    failed = folder / "FilteredGrow.cfg"
    for cfg, action in ((passed, "Walk"), (failed, "Grow")):
        cfg.write_text(f"INIT Init\nNEXT {action}\nINVARIANT TypeInvariant Safe\nCONSTRAINT Keep\n")
    _, row = run(module, passed, "--recheck", module, failed, code=1)
    assert row["ending"]["name"] == "Safe" and row["depth"] == 5
    assert row["trace_validated"] and row["reused_layer_images"] == 4

    # State 6 is an excluded emission. Expanding its old Step edge would
    # invent a three-state path to 5. The real shortest path has six states.
    clean = folder / "CleanWalk.cfg"
    clean.write_text("INIT Init\nNEXT Walk\nINVARIANT TypeInvariant\nCONSTRAINT Keep\n")
    shortcut = folder / "ExcludedShortcut.cfg"
    shortcut.write_text("INIT Init\nNEXT EmitSix\nINVARIANT TypeInvariant BeforeFive\nCONSTRAINT Keep\n")
    def check_excluded_shortcut(*extra):
        _, bad = run(module, clean, *extra, "--recheck", module, shortcut, code=1)
        assert bad["ending"]["name"] == "BeforeFive" and bad["depth"] == 6
        assert bad["trace_validated"] and bad["reused_layer_images"] == 5

    check_excluded_shortcut()
    module.write_text(module.read_text().replace("0..1", "0..299"))
    flags = ("--refine-domains",)
    check_excluded_shortcut(*flags)
    revisions = (arg for cfg in configs[1:] for arg in ("--recheck", module, cfg))
    rows = run(module, configs[0], *flags, *revisions)
    for row, cfg in zip(rows, configs, strict=True):
        compare(row, explicit(module, cfg))
    assert rows[1]["reused"] and rows[5]["reused"]  # Reset lands in Init.
    assert rows[2]["reused_layer_images"] == 4 and rows[4]["reused_layer_images"] == 1
    assert [row["distinct"] for row in rows] == [6, 6, 6, 7, 6, 6]
    _, bad = run(module, passed, *flags, "--recheck", module, failed, code=1)
    assert bad["ending"]["name"] == "Safe" and bad["depth"] == 5 and bad["trace_validated"]
    assert bad["reused_layer_images"] == 4


def check_record_permutations(folder, run, explicit, compare):
    module = folder / "PacketRoles.tla"
    module.write_text(r"""---- MODULE PacketRoles ----
EXTENDS TLC
CONSTANTS Senders, Receivers, Preferred
VARIABLES last, messages
Packets == [sender : Senders, receiver : Receivers]
TypeInvariant == last \in Packets /\ messages \subseteq Packets
Init == /\ last \in Packets /\ messages = {}
Next == \E s \in Senders, r \in Receivers :
          /\ last' = [sender |-> s, receiver |-> r]
          /\ messages' = messages \cup {last}
Both == Permutations(Senders) \cup Permutations(Receivers)
One == Permutations(Senders)
Empty == messages = {}
OnlyPreferred == last.sender = Preferred
====
""")
    common = "CONSTANTS Senders={p,q} Receivers={a,b} Preferred=p\nINIT Init\nNEXT Next\nINVARIANT TypeInvariant\n"
    def config(name, extra):
        path = folder / ("packet-" + name + ".cfg")
        path.write_text(common + extra)
        return path
    both = config("both", "SYMMETRY Both\n")
    one = config("one", "SYMMETRY One\n")
    failure = config("nonempty", "SYMMETRY Both\nINVARIANT Empty\n")
    unsafe = config("preferred", "SYMMETRY Both\nINVARIANT OnlyPreferred\n")
    for flags in ([], ["--refine-domains"]):
        pair = run(module, both, *flags, "--recheck", module, one)
        for row, cfg in zip(pair, (both, one)):
            compare(row, explicit(module, cfg))
        assert pair[0]["distinct"] < pair[1]["distinct"] and not pair[1]["reused"]
        _, bad = run(module, both, *flags, "--recheck", module, failure, code=1)
        assert bad["trace_validated"] and bad["depth"] == 2 and bad["ending"]["name"] == "Empty"
        (refused,) = run(module, unsafe, *flags, code=2)
        assert refused["verdict"] == "unsupported" and "asymmetric" in refused["reason"]

def check_nested_tables(folder, run, explicit, compare):
    # Each reachable pair of table entries has equal bits. Equalities between
    # neighboring bits have a smaller decision diagram than separated tables.
    module = folder / "Tables.tla"
    config = folder / "Tables.cfg"
    module.write_text(r"""---- MODULE Tables ----
CONSTANTS Owners, Keys
VARIABLES a, b
TypeInvariant == a \in [Owners -> SUBSET Keys] /\ b \in [Owners -> SUBSET Keys]
Init == a = [o \in Owners |-> {}] /\ b = [o \in Owners |-> {}]
Next == \E o \in Owners, k \in Keys :
    /\ a' = [a EXCEPT ![o] = @ \cup {k}]
    /\ b' = [b EXCEPT ![o] = @ \cup {k}]
====
""")
    config.write_text("CONSTANTS Owners = {u, v} Keys = {x, y, z}\nINIT Init\nNEXT Next\nINVARIANT TypeInvariant\n")
    normal, = run(module, config)
    separated, = run(module, config, "--order-by-path")
    expected = explicit(module, config)
    compare(normal, expected)
    compare(separated, expected)
    assert normal["reachable_bdd_nodes"] < separated["reachable_bdd_nodes"]
