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
            distinct += int(self.b.count(subset, len(self.variables)))
            generated += sum(
                int(self.b.count(subset & guard, len(self.variables))) for guard, _ in self.actions
            )
        if distinct % len(perms) or generated % len(perms):
            raise ArithmeticError("incomplete symmetry orbit")
        return distinct // len(perms), 1 + generated // len(perms)
