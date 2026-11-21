"""Eliminate existential successor variables in the pinned c1cs fair action."""

from itertools import product


def module(raw, source):
    """Read Apalache's SANY AST and retain the enabling guards of total updates."""
    decls = {d["name"]: d for d in raw["modules"][0]["declarations"] if "name" in d}
    lines = source.splitlines(keepends=True)
    variables = {d["name"] for d in decls.values() if d["kind"] == "TlaVarDecl"}

    def text(node):
        if node["kind"] == "NameEx":
            return node["name"]
        loc = node["source"]
        assert loc["filename"] == "c1cs"
        a, b = loc["from"], loc["to"]
        part = lines[a["line"] - 1:b["line"]]
        part[-1] = part[-1][:b["column"]]
        part[0] = part[0][a["column"] - 1:]
        return "".join(part)

    def refs(node, stack=()):
        """Collect primed variable dependencies, including called definitions."""
        op, args = node.get("oper"), node.get("args", [])
        if op == "PRIME":
            assert len(args) == 1 and args[0]["kind"] == "NameEx"
            name = args[0]["name"]
            assert name in variables
            return {name}
        if op == "UNCHANGED":
            raise ValueError("UNCHANGED in state expression")
        deps = set().union(*(refs(a, stack) for a in args))
        if op == "OPER_APP":
            name = args[0]["name"]
            assert name not in stack
            deps.update(refs(decls[name]["body"], (*stack, name)))
        return deps

    def state(node):
        assert not refs(node), "primed expression in an enabling guard"
        return text(node)

    def combine(maps):
        merged = {}
        for mapping in maps:
            assert merged.keys().isdisjoint(mapping), "multiply constrained successor variable"
            merged.update(mapping)
        return merged

    def branch(node):
        """Return a state guard and the assignment graph on every syntactic path."""
        op, args = node.get("oper"), node.get("args", [])
        if op == "AND":
            children = [branch(a) for a in args]
            terms = [guard for guard, _ in children if guard != "TRUE"]
            guard = " /\\ ".join("("+t+")" for t in terms) or "TRUE"
            return guard, [combine(m) for m in product(*(maps for _, maps in children))]
        if op == "IF_THEN_ELSE":
            cond = state(args[0])
            yes, ypaths = branch(args[1])
            no, npaths = branch(args[2])
            guard = yes if yes == no else f"(IF ({cond}) THEN ({yes}) ELSE ({no}))"
            return guard, ypaths + npaths
        if op == "EXISTS3":
            name, domain, body = args
            guard, paths = branch(body)
            return f"(\\E {state(name)} \\in {state(domain)} : ({guard}))", paths
        if op == "UNCHANGED":
            assert len(args) == 1 and args[0]["oper"] == "TUPLE"
            names = [a["name"] for a in args[0]["args"]]
            assert len(set(names)) == len(names)
            return "TRUE", [{k: set() for k in names}]
        if op == "EQ" and args[0].get("oper") == "PRIME":
            name, = refs(args[0])
            return "TRUE", [{name: refs(args[1])}]
        return state(node), [{}]

    def total(updates):
        assert updates.keys() == variables, "underconstrained successor tuple"
        assigned = set()
        while assigned != variables:
            ready = {n for n, deps in updates.items() if deps <= assigned} - assigned
            assert ready, "cyclic successor assignments"
            assigned |= ready

    spec = decls["Spec"]["body"]
    assert spec["oper"] == "AND" and len(spec["args"]) == 2
    safety, wf = spec["args"]
    assert safety["oper"] == "AND" and len(safety["args"]) == 2
    init, always = safety["args"]
    assert text(init) == "Init" and always["oper"] == "GLOBALLY"
    square, = always["args"]
    assert square["oper"] == "STUTTER" and text(square["args"][0]) == "Next"
    assert [a["name"] for a in square["args"][1]["args"]] == [
        a["name"] for a in decls["vars"]["body"]["args"]
    ]
    assert {a["name"] for a in decls["vars"]["body"]["args"]} == variables
    assert wf["oper"] == "WEAK_FAIRNESS" and text(wf["args"][0]) == "vars"
    fair = wf["args"][1]
    assert fair["oper"] == "EXISTS3"
    actor, domain, disjunction = fair["args"]
    assert disjunction["oper"] == "OR"
    names, definitions = [], []
    for call in disjunction["args"]:
        assert call["oper"] == "OPER_APP" and len(call["args"]) == 2
        name = call["args"][0]["name"]
        assert text(call["args"][1]) == text(actor)
        decl = decls[name]
        formal, = decl["formalParams"]
        guard, paths = branch(decl["body"])
        for updates in paths:
            total(updates)
        definitions.append(f"Can{name}({formal['name']}) == {guard}")
        names.append(f"Can{name}({text(actor)})")
    # Apalache checks a typed finite domain. In both original CHOOSE assignments,
    # prove the chosen set is inhabited whenever that branch can execute.
    def choices(node):
        return ([node] if node.get("oper") == "CHOOSE3" else []) + [
            c for a in node.get("args", []) for c in choices(a)
        ]

    def witness(node):
        bound, domain, pred = node["args"]
        return f"(\\E {state(bound)} \\in {state(domain)} : {state(pred)})"

    outer, = [a for a in decls["Core_T1"]["body"]["args"] if a.get("oper") == "IF_THEN_ELSE"]
    unanimous, then, elsewhere = outer["args"]
    inner, = [a for a in elsewhere["args"] if a.get("oper") == "IF_THEN_ELSE"]
    guard, supported, unchanged = inner["args"]
    choice1, = choices(then)
    choice2, = choices(supported)
    assert not choices(unchanged)
    user_defs = [d for d in decls.values() if d.get("source", {}).get("filename") == "c1cs"]
    assert sum(len(choices(d.get("body", {}))) for d in user_defs) == 2
    definitions.append(
        "ChoicesDefined == \\A i \\in Proc: CanCore_T1(i) => "
        f"(IF ({state(unanimous)}) THEN {witness(choice1)} "
        f"ELSE (({state(guard)}) => {witness(choice2)}))"
    )
    prop = decls["Termination"]["body"]
    assert prop["oper"] == "EVENTUALLY"
    can = " \\/ ".join(names)
    return "\n\n".join([
        "---- MODULE C1CSEnabled ----\nEXTENDS c1cs",
        *definitions,
        f"Fair == {text(fair)}",
        f"FairEnabled == \\E {text(actor)} \\in {text(domain)} : {can}",
        f"Goal == {text(prop['args'][0])}",
        "====\n",
    ])


def all_choices(raw, source):
    """Widen each of the two vote assignments to every satisfying chosen value."""
    decl = next(d for d in raw["modules"][0]["declarations"] if d.get("name") == "Core_T1")
    lines = source.splitlines(keepends=True)

    def span(node):
        loc = node["source"]
        assert loc["filename"] == "c1cs"
        a, b = loc["from"], loc["to"]
        return (sum(map(len, lines[:a["line"] - 1])) + a["column"] - 1,
                sum(map(len, lines[:b["line"] - 1])) + b["column"])

    def nodes(tree):
        yield tree
        for arg in tree.get("args", []):
            yield from nodes(arg)

    substitutions = []
    for equation in nodes(decl["body"]):
        if equation.get("oper") != "EQ" or equation["args"][0].get("oper") != "PRIME":
            continue
        rhs = equation["args"][1]
        chosen = [x for x in nodes(rhs) if x.get("oper") == "CHOOSE3"]
        if not chosen:
            continue
        choice, = chosen
        # The binder scopes only over the replaced assignment. In both source
        # clauses the RHS is a single EXCEPT, with the chosen value as its leaf.
        assert rhs["oper"] == "EXCEPT" and rhs["args"][-1] is choice
        bound, domain, predicate = choice["args"]
        name = bound["name"]
        left, right = span(equation)
        begin, end = span(choice)
        assert left <= begin < end <= right
        before, after = source[left:begin], source[end:right]
        assert name not in before + after
        dom = source[slice(*span(domain))]
        pred = source[slice(*span(predicate))]
        widened = f"(\\E {name} \\in {dom}: ({pred}) /\\ ({before}{name}{after}))"
        substitutions.append((left, right, widened))
    assert len(substitutions) == 2
    for left, right, replacement in sorted(substitutions, reverse=True):
        source = source[:left] + replacement + source[right:]
    return source
