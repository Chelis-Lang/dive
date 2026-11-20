"""Finite relational image and symmetry-counting primitives for CUDD."""

import itertools


def equal(a, b):
    return a.equiv(b)


class FiniteRelation:
    """Operations shared by the two independent relation constructors."""

    def conjunction(self, terms):
        value = self.b.true
        for term in terms:
            value &= term
        return value

    def encoding(self, bits, value):
        return self.conjunction(bit if value & (1 << i) else ~bit for i, bit in enumerate(bits))

    def domain(self, bits, maximum):
        allowed = self.b.false
        for i in range(maximum + 1):
            allowed |= self.encoding(bits, i)
        return allowed

    def assignment(self, updates):
        relation = self.conjunction(equal(self.b.var(x.var + "_next"), expr) for x, expr in updates.items())
        return relation, [x.var for x in updates], {x.var + "_next": x.var for x in updates}

    def state_count(self, root):
        """Count current-state assignments. CUDD interleaves next bits in its order."""
        width = len(self.variables)
        if width <= 52:
            return int(self.b.count(root, width))

        # A memo entry counts assignments from its node's current-state level
        # downwards. Skipped current bits are free. Primed bits never contribute.
        positions = {name: i for i, name in enumerate(
            sorted(self.variables, key=self.b.level_of_var)
        )}
        memo = {self.b.true: (width, 1)}

        def below(edge, start):
            regular = ~edge if edge.negated else edge
            level, count = memo[regular]
            if edge.negated:
                count = (1 << (width - level)) - count
            return count << (level - start)

        stack = [~root if root.negated else root]
        while stack:
            node = stack[-1]
            if node in memo:
                stack.pop()
                continue
            low, high = node.low, node.high
            children = [~edge if edge.negated else edge for edge in (low, high)]
            missing = [child for child in children if child not in memo]
            if missing:
                stack.extend(missing)
                continue
            level = positions[node.var]
            memo[node] = (level, below(low, level + 1) + below(high, level + 1))
            stack.pop()
        return below(root, 0)

    def counts(self, states, symmetry):
        """Count complete state orbits and outgoing witnesses, including the single Init."""
        # Burnside also works for the sum of outdegrees: outdegree is invariant
        # under renaming, so each orbit contributes its representative's degree.
        distinct = generated = 0
        roles = range(len(self.roles))
        perms = list(itertools.permutations(roles)) if symmetry else [tuple(roles)]
        for perm in perms:
            fixed = self.conjunction(
                equal(self.b.var(x), self.b.var(y))
                for t, u in enumerate(perm)
                if t != u
                for x, y in zip(self.roles[t], self.roles[u])
            )
            subset = states & fixed
            distinct += self.state_count(subset)
            generated += sum(
                self.state_count(subset & guard) for guard, _ in self.actions
            )
        if distinct % len(perms) or generated % len(perms):
            raise ArithmeticError("incomplete symmetry orbit")
        return distinct // len(perms), 1 + generated // len(perms)
