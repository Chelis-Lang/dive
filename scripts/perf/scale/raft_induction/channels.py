"""Evaluate source channel operations as extensional predicates on complete records."""

from copy import copy
from itertools import combinations

import z3

from source import Model
from values import Rec, Seq, Set, con, dis, eq, ite, member, neg, require


class Channel:
    """An arbitrary subset of the finite wire carrier, followed by source edits."""

    def __init__(self, name, fields, field, valid):
        self.name = name
        self.fields = fields
        self.field = field
        self.valid = valid
        self.operations = ()
        self.new_records = []
        schema = self.arbitrary("signature")
        self.predicate = z3.Function(
            name, *(v.sort() for v in self.key(schema)), z3.BoolSort()
        )

    def arbitrary(self, prefix=None):
        if prefix is None:
            prefix = z3.FreshInt("selected_message").decl().name()
        return Rec(
            {
                name: self.field(shape, prefix + "." + name)
                for name, shape in self.fields.items()
            }
        )

    def key(self, record):
        result = []
        for name in self.fields:
            if name == "mtype":
                continue  # Each channel admits exactly one checked mtype.
            value = record.fields[name]
            if isinstance(value, Seq):
                # The delivery format transmits zero or one entry. Equal empty
                # payloads have identical keys regardless of their unused cell.
                result.extend(
                    [
                        value.length,
                        ite(value.length > 0, value.cells[0] if value.cells else 0, 0),
                    ]
                )
            else:
                result.append(value)
        return result

    def variables(self, record):
        """Unnormalized variables bound by a record quantifier, in key order."""
        fields = []
        for name in self.fields:
            if name == "mtype":
                continue
            item = record.fields[name]
            fields.extend(
                [item.length, *item.cells] if isinstance(item, Seq) else [item]
            )
        return fields

    def contains(self, record):
        if self.fields.keys() != record.fields.keys():
            return False
        included = con(self.valid(self.name, record), self.predicate(*self.key(record)))
        for insert, change in self.operations:
            edited = member(record, change)
            included = dis(included, edited) if insert else con(included, neg(edited))
        return included

    def update(self, insert, finite):
        image = copy(self)
        image.operations = (*self.operations, (insert, finite))
        if insert:
            image.new_records = self.new_records + finite.cells
        return image

    def cardinality_equals(self, size, index):
        """Construct an exact small-cardinality guard for the gated source control."""
        require(0 <= size <= 3, "unsupported explicit channel-cardinality guard")
        witnesses = [self.arbitrary(f"{self.name}_q{index}_{i}") for i in range(size)]
        probe = self.arbitrary(f"{self.name}_q{index}_remaining")
        coverage = z3.ForAll(
            self.variables(probe),
            z3.Implies(self.contains(probe), dis(*(eq(probe, w) for w in witnesses))),
        )
        if not witnesses:
            return coverage
        return z3.Exists(
            [x for w in witnesses for x in self.variables(w)],
            con(
                coverage,
                *(self.contains(w) for w in witnesses),
                *(neg(eq(a, b)) for a, b in combinations(witnesses, 2)),
            ),
        )


class Combined:
    """Heterogeneous union of channels in the original UpdateTerm action binder."""

    def __init__(self, parts):
        self.parts = parts


class ChannelModel(Model):
    def is_channel_domain(self, index):
        kind, node = self.exprs[index]
        return (
            kind == "Var"
            and not node["primed"]
            and self.names[node["var"]].endswith("Msgs")
        )

    def exists_record(self, body, slot, channel, environment, path_guard):
        # GrantedVoteSet asks for a vote with EVERY dynamic field specified.
        # Solve those equalities. Evaluating every independent RHS also
        # conservatively checks its definedness if the channel is empty.
        kind, junction = self.exprs[body]
        require(
            kind == "Junction" and junction["kind"] == "And",
            "unsupported existential message predicate",
        )
        fields = {"mtype": self.symbols[channel.fields["mtype"]]}
        outer = {k: v for k, v in environment.items() if k != slot}
        for item in junction["items"]:
            kind, node = self.exprs[item]
            equality = kind == "Builtin" and node["op"] == "Eq"
            if equality:
                left, right = node["args"]
                kind, node = self.exprs[left]
                value = self.eval(right, outer, path_guard)
            else:
                value = True
            require(
                kind == "RecordSelect"
                and self.exprs[node["record"]] == ("Local", {"up": 0, "slot": slot}),
                "existential does not select its bound record",
            )
            name = node["field"]
            require(
                name not in fields and (equality or channel.fields[name] == "bool"),
                "unsupported existential field test",
            )
            fields[name] = value
        require(
            fields.keys() == channel.fields.keys(),
            "existential does not identify one complete message",
        )
        return channel.contains(Rec(fields))

    def universally(self, index, node, bound, channel, environment, path_guard):
        records = [
            channel.arbitrary(f"{channel.name}_q{index}_{slot}")
            for slot in bound["vars"]
        ]
        symbols = [x for record in records for x in channel.variables(record)]
        active = con(*(channel.contains(record) for record in records))
        env = {**environment, **dict(zip(bound["vars"], records, strict=True))}
        outer_errors = self.errors
        self.errors = []
        try:
            body = self.eval(node["body"], env, con(path_guard, active))
            undefined = z3.simplify(z3.Or(*self.errors))
        finally:
            self.errors = outer_errors
        if not z3.is_false(undefined):
            self.errors.append(z3.Exists(symbols, undefined))
        return z3.ForAll(symbols, z3.Implies(active, body))

    def eval(self, index, environment, path_guard=True):
        kind, node = self.exprs[index]
        if kind == "Quant" and any(
            self.is_channel_domain(b["domain"]) for b in node["bounds"]
        ):
            require(len(node["bounds"]) == 1, "multiple channel binder groups")
            bound = node["bounds"][0]
            channel = self.eval(bound["domain"], environment, path_guard)
            if isinstance(channel, Channel):
                require(not bound["tuple"], "unsupported tuple message binder")
                if node["kind"] == "Exists":
                    (slot,) = bound["vars"]
                    return self.exists_record(
                        node["body"], slot, channel, environment, path_guard
                    )
                require(node["kind"] == "Forall", "unsupported message quantifier")
                return self.universally(
                    index, node, bound, channel, environment, path_guard
                )
        if kind == "Builtin" and node["op"] == "Eq":
            left, right = node["args"]
            tag, call = self.exprs[left]
            if tag == "Apply" and call["callee"] == {"Std": "FiniteSetsCardinality"}:
                ref = call["args"][0]["Expr"]["expr"]
                if self.is_channel_domain(ref):
                    channel = self.eval(ref, environment, path_guard)
                    if isinstance(channel, Channel):
                        tag, size = self.exprs[right]
                        require(
                            tag == "Num", "unsupported channel cardinality comparison"
                        )
                        return channel.cardinality_equals(int(size), index)
        if kind == "Apply" and node["callee"] == {"Std": "FiniteSetsCardinality"}:
            ref = node["args"][0]["Expr"]["expr"]
            if self.is_channel_domain(ref) and isinstance(
                self.eval(ref, environment, path_guard), Channel
            ):
                raise ValueError(
                    "unsupported channel cardinality outside the state constraint or a small equality guard"
                )
        if kind == "Builtin" and node["op"] in ("Cup", "SetMinus", "In", "NotIn"):
            a, b = [self.eval(ref, environment, path_guard) for ref in node["args"]]
            if node["op"] in ("In", "NotIn"):
                present = b.contains(a) if isinstance(b, Channel) else member(a, b)
                return present if node["op"] == "In" else neg(present)
            if isinstance(a, (Channel, Combined)) or isinstance(b, (Channel, Combined)):
                if isinstance(a, Channel) and isinstance(b, Set):
                    return a.update(node["op"] == "Cup", b)
                require(node["op"] == "Cup", "unsupported channel difference")
                return Combined(
                    [
                        *(a.parts if isinstance(a, Combined) else [a]),
                        *(b.parts if isinstance(b, Combined) else [b]),
                    ]
                )
            if node["op"] == "Cup":
                return Set(a.cells + b.cells)
            return Set([(con(h, neg(member(v, b))), v) for h, v in a.cells])
        return super().eval(index, environment, path_guard)

    def scopes(self, bounds, environment, guard):
        if len(bounds) == 1 and len(bounds[0]["vars"]) == 1:
            bound = bounds[0]
            tag, body = self.exprs[bound["domain"]]
            if (
                self.is_channel_domain(bound["domain"])
                or tag == "Builtin"
                and body["op"] == "Cup"
            ):
                value = self.eval(bound["domain"], environment, guard)
                if isinstance(value, (Combined, Channel)):
                    require(not bound["tuple"], "unsupported tuple action binder")
                    result = []
                    for channel in (
                        value.parts if isinstance(value, Combined) else [value]
                    ):
                        require(
                            isinstance(channel, Channel),
                            "mixed scalar and message domain",
                        )
                        record = channel.arbitrary()
                        result.append(
                            (
                                con(guard, channel.contains(record)),
                                {**environment, bound["vars"][0]: record},
                            )
                        )
                    return result
        return super().scopes(bounds, environment, guard)
