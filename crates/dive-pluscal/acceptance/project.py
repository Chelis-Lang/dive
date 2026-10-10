"""Shared comparison view, distinct from either parser's implementation.

Only source structure is projected. Explicit C blocks, P elsif chains, with
lists and call tails are unfolded to compare Java's pre-translation grammar
AST without erasing branches, operands, declarations or statement order.
Unknown Java/native variants raise instead of disappearing.
"""


def java_expr(value):
    return [token["string"] if token["type"] != 3 else '"' + token["string"] + '"'
            for line in value["tokens"] for token in line]


def java_decl(value):
    expression = java_expr(value["val"])
    return [value["var"], "Equal" if value.get("isEq", True) else "Member",
            expression if expression != ["defaultInitValue"] else None]


def java_sequence(values):
    output = []
    for value in values:
        kind = value["type"]
        label = value.get("lbl", "")
        if kind == "Assign":
            body = [[a["lhs"]["var"], java_expr(a["lhs"]["sub"]), java_expr(a["rhs"])] for a in value["ass"]]
            node = ["Assign", body]
        elif kind == "While":
            node = ["While", java_expr(value["test"]), java_sequence(value["unlabDo"])]
        elif kind == "LabelIf":
            node = ["If", java_expr(value["test"]), java_sequence(value["unlabThen"]), java_sequence(value["unlabElse"])]
        elif kind == "LabelEither":
            node = ["Either", [java_sequence(c["unlabOr"]) for c in value["clauses"]]]
        elif kind == "With":
            node = ["With", value["var"], "Equal" if value["isEq"] else "Member", java_expr(value["exp"]), java_sequence(value["Do"])]
        elif kind in ("When", "Assert", "PrintS"):
            node = [kind, java_expr(value["exp"])]
        elif kind in ("Skip", "Return"):
            node = [kind]
        elif kind == "Goto":
            node = [kind, value["to"].strip('"')]
        elif kind in ("Call", "CallReturn", "CallGoto"):
            node = ["Call", value["to"], [java_expr(e) for e in value["args"]]]
        elif kind == "MacroCall":
            node = [kind, value["name"], [java_expr(e) for e in value["args"]]]
        else:
            raise ValueError("unaccounted Java source node: " + kind)
        output.append([label, node])
        if kind == "CallReturn":
            output.append(["", ["Return"]])
        if kind == "CallGoto":
            output.append(["", ["Goto", value["after"].strip('"')]])
    return output


def java_project(value):
    def procedure(p):
        return [p["name"], [java_decl(d) for d in p["params"]], [java_decl(d) for d in p["decls"]], java_sequence(p["body"])]

    def process(p):
        return [p["name"], ["Unfair", "Weak", "Strong"][p["fairness"]], "Equal" if p["isEq"] else "Member",
                java_expr(p["id"]), [java_decl(d) for d in p["decls"]], java_sequence(p["body"])]

    return {"name": value["name"], "syntax": value["syntax"], "fair": value["fair"],
            "declarations": [java_decl(d) for d in value["decls"]],
            "definitions": java_expr(value["defs"]) if "defs" in value else None,
            "macros": [[m["name"], m["params"], java_sequence(m["body"])] for m in value["macros"]],
            "procedures": [procedure(p) for p in value["prcds"]],
            "body": ["Processes", [process(p) for p in value["procs"]]] if "procs" in value else ["Sequential", java_sequence(value["body"])]}


def native_project(parsed):
    def expr(value):
        if value is None:
            return []
        span = value["span"]
        return [t["text"] for t in parsed["tokens"] if span["start"] <= t["span"]["start"] < span["end"]]

    def decl(value):
        initial = value["initial"]
        return [value["name"]["text"], initial["kind"] if initial else "Equal", expr(initial["expression"]) if initial else None]

    def sequence(block):
        output = []
        for ident in block["statements"]:
            value = parsed["statements"][ident]
            label = value["label"]["name"]["text"] if value["label"] else ""
            variant = value["kind"]
            kind = variant if isinstance(variant, str) else next(iter(variant))
            data = None if isinstance(variant, str) else variant[kind]
            if kind == "Block":
                children = sequence(data)
                if label:
                    if children[0][0]:
                        raise ValueError("duplicate native block labels")
                    children[0][0] = label
                output.extend(children)
                continue
            if kind == "Assign":
                node = [kind, [[a["variable"]["text"], expr(a["selectors"]), expr(a["value"])] for a in data]]
            elif kind == "While":
                node = [kind, expr(data["test"]), sequence(data["body"])]
            elif kind == "If":
                tail = sequence(data["otherwise"]) if data["otherwise"] else []
                for branch in reversed(data["branches"]):
                    tail = [["", ["If", expr(branch["test"]), sequence(branch["body"]), tail]]]
                node = tail[0][1]
            elif kind == "Either":
                node = [kind, [sequence(b) for b in data]]
            elif kind == "With":
                tail = sequence(data["body"])
                for binder in reversed(data["bindings"]):
                    tail = [["", ["With", binder["name"]["text"], binder["binding"]["kind"], expr(binder["binding"]["expression"]), tail]]]
                node = tail[0][1]
            elif kind == "Await":
                node = ["When", expr(data["condition"])]
            elif kind in ("Assert", "Print"):
                node = ["PrintS" if kind == "Print" else kind, expr(data)]
            elif kind in ("Skip", "Return"):
                node = [kind]
            elif kind == "Goto":
                node = [kind, data["text"].strip('"')]
            elif kind in ("Call", "MacroCall"):
                node = [kind, data["name"]["text"], [expr(a) for a in data["arguments"]]]
            else:
                raise ValueError("unaccounted native source node: " + kind)
            output.append([label, node])
        return output

    algorithm = parsed["algorithm"]
    def procedure(p):
        return [p["name"]["text"], [decl(d) for d in p["parameters"]], [decl(d) for d in p["variables"]], sequence(p["body"])]

    def process(p):
        return [p["name"]["text"], p["fairness"], p["id"]["kind"], expr(p["id"]["expression"]), [decl(d) for d in p["variables"]], sequence(p["body"])]

    body = algorithm["body"]
    return {"name": algorithm["name"]["text"], "syntax": algorithm["syntax"], "fair": algorithm["fairness"] == "Weak",
            "declarations": [decl(d) for d in algorithm["variables"]], "definitions": expr(algorithm["definitions"]) if algorithm["definitions"] else None,
            "macros": [[m["name"]["text"], [p["text"] for p in m["parameters"]], sequence(m["body"])] for m in algorithm["macros"]],
            "procedures": [procedure(p) for p in algorithm["procedures"]],
            "body": ["Processes", [process(p) for p in body["Processes"]]] if "Processes" in body else ["Sequential", sequence(body["Sequential"])]}
