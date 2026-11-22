"""Lower the proved finite relation to packed Rust guards and updates."""

from collections import defaultdict
import itertools
from math import comb

from engine import require


class Rust:
    """Put symmetry lanes next to one another and lower the Boolean DAG."""

    def __init__(self, model):
        roles = set(itertools.chain.from_iterable(model.roles))
        fields = [field for field in model.variables if field not in roles]
        self.fixed = len(fields)
        fields.extend(itertools.chain.from_iterable(model.roles))
        self.bits = {name: index for index, name in enumerate(fields)}
        self.word_size = 32 if len(fields) <= 32 else 64
        self.model = model
        self.total_nodes = 0

    def edge(self, edge, nodes, lines):
        manager = self.model.b
        if edge == manager.true:
            return "true"
        if edge == manager.false:
            return "false"
        if edge.negated:
            return "!" + self.edge(~edge, nodes, lines)
        if edge in nodes:
            return nodes[edge]
        mask = 1 << self.bits[edge.var]
        low, high = self.edge(edge.low, nodes, lines), self.edge(edge.high, nodes, lines)
        name = f"bdd_{len(nodes)}"
        nodes[edge] = name
        self.total_nodes += 1
        require(self.total_nodes <= 8192, "generated decision DAG exceeds 8192 nodes")
        expression = (f"row & {mask:#x} != 0" if (low, high) == ("false", "true")
                      else f"if row & {mask:#x} != 0 {{ {high} }} else {{ {low} }}")
        lines.append(f"let {name} = {expression};")
        return name

    def action(self, guard, updates):
        # Adjacent destination bits that copy a contiguous source field can
        # move together, even when other bits in the row use general BDDs.
        nodes = {}
        before_guard = []
        condition = self.edge(guard, nodes, before_guard)
        after_guard = []
        copies = defaultdict(int)
        set_bits = 0
        compute = []
        written = 0
        for target, root in updates:
            index = self.bits[target]
            written |= 1 << index
            if root == self.model.b.true:
                set_bits |= 1 << index
            elif root == self.model.b.false:
                pass
            elif root == self.model.b.var(root.var):
                src = self.bits[root.var]
                copies[(index - src) % self.word_size] |= 1 << src
            else:
                compute.append(f"((({self.edge(root, nodes, after_guard)}) as u{self.word_size}) << {index})")
        terms = [f"row & {((1 << self.word_size) - 1) ^ written:#x}"]
        if set_bits:
            terms.append(f"{set_bits:#x}")
        terms.extend(f"(row & {mask:#x}).rotate_left({shift})" for shift, mask in sorted(copies.items()))
        terms.extend(compute)
        body = "\n                ".join([*after_guard, f"self.emit({' | '.join(terms)});"])
        guard = "\n            ".join(before_guard)
        return f"{{\n            {guard}\n            if {condition} {{\n                {body}\n            }}\n        }}"

    def rank(self, point):
        return sum(1 << bit for name, bit in self.bits.items() if point[name])

    def orbit_ranks(self):
        model = self.model
        roles = model.roles
        require(model.symmetry and len(roles) >= 2, "wide compiled states require proved index symmetry")
        require(self.fixed < 16 and 0 < len(roles[0]) < 16,
                "wide compiled states require fixed fields and symmetry lanes narrower than 16 bits")
        legal = model.conjunction(prop for _, prop in model.properties)

        def projected_words(fields):
            projected = model.b.exist(set(self.bits) - set(fields), legal)
            return sorted(sum(1 << i for i, name in enumerate(fields) if point[name])
                          for point in model.b.pick_iter(projected, set(fields)))

        def table(words, width):
            entries = ["INVALID"] * (1 << width)
            for rank, word in enumerate(words):
                entries[word] = str(rank)
            return ", ".join(entries)

        fixed_words = projected_words(list(self.bits)[:self.fixed])
        lane_words = projected_words(roles[0])
        require(all(projected_words(lane) == lane_words for lane in roles[1:]),
                "asymmetric projected compiled ranks")
        # Sorted lane ordinals form a multiset of the projected lane values.
        capacity = len(fixed_words) * comb(len(lane_words) + len(roles) - 1, len(roles))
        require(capacity <= 1 << 36, "exact dense orbit membership exceeds 8 GiB")
        return dict(
            FIXED_RANKS=table(fixed_words, self.fixed),
            LANE_RANKS=table(lane_words, len(roles[0])),
            FIXED_SPACE=len(fixed_words),
            LANE_RANGE=len(lane_words),
            MEMBERSHIP_BITS=capacity,
        )

    def emit(self, template):
        model = self.model
        require(model.constraint == model.b.true, "state constraints in compiled checking")
        require(0 < len(self.bits) < 64, "expected one to 63 state bits")
        require(len(model.roles) <= 6, "too many symmetry lanes")
        require(not model.roles or len({len(lane) for lane in model.roles}) == 1,
                "unequal compiled symmetry lanes")
        root_words = [self.rank(point) for point in model.b.pick_iter(model.initial, self.bits)]
        require(model.effects.total() <= 512, "generated worker exceeds 512 action witnesses")
        expand = "\n        ".join(
            self.action(guard, sorted(updates))
            for (guard, updates), count in model.effects.items()
            for _ in range(count)
        )
        properties = []
        for _, root in model.properties:
            lines = []
            holds = self.edge(root, {}, lines)
            if holds == "true":
                continue
            properties.append("{ " + " ".join(lines) + f" {holds} }}")
        condition = " && ".join(f"({part})" for part in properties) or "true"
        ranked = self.word_size == 64
        ranks = self.orbit_ranks() if ranked else dict(
            FIXED_RANKS="", LANE_RANKS="", FIXED_SPACE=0, LANE_RANGE=0,
            MEMBERSHIP_BITS=1 << len(self.bits),
        )
        values = dict(
            **ranks,
            WIDTH=len(self.bits),
            WORD=f"u{self.word_size}",
            RANKED=str(ranked).lower(),
            FIXED=self.fixed,
            ROLES=len(model.roles),
            LANE=len(model.roles[0]) if model.roles else 0,
            CHECK_DEADLOCK=str(model.doc["check_deadlock"]).lower(),
            INITIAL_GENERATED=model.initial_generated,
            INITIALS=", ".join(map(str, root_words)),
            EXPAND=expand,
            PROPERTIES=condition,
        )
        worker = template
        for key, value in values.items():
            worker = worker.replace(f"@{key}@", str(value))
        self.metadata = {
            "width": len(self.bits),
            "membership_bits": ranks["MEMBERSHIP_BITS"],
            "symmetry_lanes": len(model.roles),
            "decision_nodes": self.total_nodes,
            "actions": model.effects.total(),
        }
        return worker
