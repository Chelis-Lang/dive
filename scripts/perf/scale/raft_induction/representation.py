"""The bounded state representation, checked on Init and every retained destination."""

import z3 as z
from values import Seq, Rec, Tab, Set, require, neg, con, dis, concrete, card

sets = (
    "requestVoteRequestMsgs",
    "requestVoteResponseMsgs",
    "appendEntriesRequestMsgs",
    "appendEntriesResponseMsgs",
)
schemas = [
    {
        "mtype": "RequestVoteRequest",
        "mterm": "term",
        "mlastLogTerm": "term",
        "mlastLogIndex": "index",
        "msource": "server",
        "mdest": "server",
    },
    {
        "mtype": "RequestVoteResponse",
        "mterm": "term",
        "mvoteGranted": "bool",
        "msource": "server",
        "mdest": "server",
    },
    {
        "mtype": "AppendEntriesRequest",
        "mterm": "term",
        "mprevLogIndex": "index",
        "mprevLogTerm": "term",
        "mentries": "entries",
        "mcommitIndex": "index",
        "msource": "server",
        "mdest": "server",
    },
    {
        "mtype": "AppendEntriesResponse",
        "mterm": "term",
        "msuccess": "bool",
        "mmatchIndex": "index",
        "msource": "server",
        "mdest": "server",
    },
]


def representation(model, channel=None):
    constants = {
        d["name"]: model.constants[d["constant"]]
        for d in model.ir["decls"]
        if d["constant"] is not None
    }
    limit = constants["MaxMsgCount"]
    require(
        type(limit) is int and (limit in (2, 3) or channel and limit >= 0),
        "unsupported bound message capacity",
    )
    require(
        constants["MaxTerm"] in ((2, 3) if channel else (2,))
        and constants["MaxLogLen"] in ((2, 3) if channel else (2,)),
        "unsupported bound term or log limit",
    )
    max_term = constants["MaxTerm"]
    max_log = constants["MaxLogLen"]
    servers = concrete(constants["Server"])
    symbols = model.symbols

    def field(shape, name):
        if shape in symbols:
            return symbols[shape]
        if shape == "bool":
            return z.Bool(name)
        if shape in ("log", "entries"):
            return Seq(
                z.Int(name + ".len"),
                [z.Int(f"{name}.{k}") for k in range(max_log if shape == "log" else 1)],
            )
        return z.Int(name)

    def table(fn):
        return Tab(servers, tuple(fn(i) for i in servers))

    def server_function(v):
        return table(lambda i: field(v, str(i) + "." + v))

    before = {
        "currentTerm": server_function("term"),
        "state": server_function("role"),
        "votedFor": server_function("vote"),
        "log": server_function("log"),
        "commitIndex": server_function("commitIndex"),
        "votesGranted": table(
            lambda i: Set([(z.Bool(f"vg.{i}.{j}"), j) for j in servers])
        ),
        **{
            n: table(lambda i: table(lambda j: z.Int(f"{n}.{i}.{j}")))
            for n in ("nextIndex", "matchIndex")
        },
    }

    def span(x, lo, hi):
        return con(x >= lo, x <= hi)

    def among(x, keys):
        return dis(*(x == key for key in keys))

    def allowed(x, shape):
        if shape in symbols:
            return x == symbols[shape]
        if shape == "bool":
            return isinstance(x, bool) or z.is_bool(x)
        if shape == "server":
            return among(x, servers)
        if shape == "role":
            return among(x, [symbols[t] for t in ("Leader", "Candidate", "Follower")])
        if shape == "vote":
            return among(x, [*servers, symbols["Nil"]])
        if shape in ("log", "entries"):
            return con(
                span(x.length, 0, max_log if shape == "log" else 1),
                *(
                    dis(x.length <= i, span(v, 0, max_term))
                    for i, v in enumerate(x.cells)
                ),
            )
        return (
            span(x, 1, max_log + 1)
            if shape == "nextIndex"
            else span(x, 0, max_term if shape == "term" else max_log)
        )

    def valid_record(n, rec):
        fields = schemas[sets.index(n)]
        d = rec.fields
        require(d.keys() == fields.keys(), "unexpected message fields")
        bounded = con(
            *(allowed(d[k], sh) for k, sh in fields.items()), d["msource"] != d["mdest"]
        )
        if n == "appendEntriesRequestMsgs":
            bounded = con(bounded, d["mprevLogIndex"] + d["mentries"].length <= max_log)
        return bounded

    for n, fields in zip(sets, schemas, strict=True):
        if channel is not None:
            before[n] = channel(n, fields, field, valid_record)
            continue
        before[n] = Set(
            [
                (
                    z.Bool(f"{n}.{q}.used"),
                    Rec({k: field(sh, f"{n}.{q}.{k}") for k, sh in fields.items()}),
                )
                for q in range(limit)
            ]
        )

    def domain(state):
        predicates = []
        for name in model.names:
            v = state[name]
            if name in sets:
                if channel is not None and hasattr(v, "new_records"):
                    cells = v.new_records
                else:
                    cells = v.cells
                predicates.extend(
                    dis(neg(h), valid_record(name, rec)) for h, rec in cells
                )
                if channel is None:
                    predicates.append(card(v) <= limit)
            else:
                require(v.keys == servers, "outer server function domain")
                for x in v.cells:
                    if name == "votesGranted":
                        predicates.extend(
                            dis(neg(b), among(k, servers)) for b, k in x.cells
                        )
                    elif name in ("nextIndex", "matchIndex"):
                        require(x.keys == servers, "inner server function domain")
                        predicates.extend(allowed(y, name) for y in x.cells)
                    else:
                        predicates.append(
                            allowed(
                                x,
                                {
                                    "currentTerm": "term",
                                    "state": "role",
                                    "votedFor": "vote",
                                    "commitIndex": "index",
                                    "log": "log",
                                }[name],
                            )
                        )
        return con(*predicates)

    return before, domain
