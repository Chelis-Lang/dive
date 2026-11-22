"""A sound finite instantiation of universal assumptions for each refutation query."""

from itertools import product
from functools import lru_cache
import z3
from values import require


def visit(root):
    seen = set()
    todo = [root]
    while todo:
        expr = todo.pop()
        if expr.get_id() in seen:
            continue
        seen.add(expr.get_id())
        yield expr
        if z3.is_quantifier(expr):
            todo.append(expr.body())
        elif z3.is_app(expr):
            todo.extend(expr.children())


@lru_cache(maxsize=65536)
def quantify_free(expr):
    return z3.is_app(expr) and all(map(quantify_free, expr.children()))


def nnf(expr):
    # Skolemize only Boolean quantifiers. Preserve term-level ITEs used in
    # canonical record keys; introducing fresh, uninstantiated choice axioms
    # obscures their equality.
    cache = {}

    def transform(node, positive, universal):
        key = (node, positive, tuple(universal))
        if key in cache:
            return cache[key]
        if quantify_free(node):
            result = node if positive else z3.Not(node)
        elif z3.is_quantifier(node):
            forall = node.is_forall() == positive
            bound = []
            for i in range(node.num_vars()):
                name = str(node.var_name(i))
                sort = node.var_sort(i)
                if forall:
                    arg = z3.FreshConst(sort, prefix=name)
                elif universal:
                    symbol = z3.FreshFunction(*(v.sort() for v in universal), sort)
                    arg = symbol(*universal)
                else:
                    arg = z3.FreshConst(sort, prefix=name)
                bound.append(arg)
            body = z3.substitute_vars(node.body(), *reversed(bound))
            body = transform(
                body, positive, [*universal, *bound] if forall else universal
            )
            result = z3.ForAll(bound, body, qid=str(node.qid())) if forall else body
        elif z3.is_not(node):
            result = transform(node.arg(0), not positive, universal)
        elif z3.is_and(node) or z3.is_or(node):
            combine = z3.And if z3.is_and(node) == positive else z3.Or
            result = combine(
                *(transform(ch, positive, universal) for ch in node.children())
            )
        elif z3.is_implies(node):
            result = (z3.Or if positive else z3.And)(
                transform(node.arg(0), not positive, universal),
                transform(node.arg(1), positive, universal),
            )
        else:
            result = z3.BoolVal(True)
        cache[key] = result
        return result

    return z3.simplify(transform(z3.simplify(expr), True, []))


def flattened(root):
    if z3.is_and(root):
        for child in root.children():
            yield from flattened(child)
    elif not z3.is_true(root):
        yield root


class Ground:
    def __init__(self, types, base):
        self.types = types
        self.members = {v.predicate.get_id(): k for k, v in types.items()}
        self.universal = []
        self.solver = z3.Solver()
        self.solver.set(timeout=20000)
        self.samples = {n: {} for n in self.types}
        for expr in base:
            for part in flattened(nnf(expr)):
                self.collect(part, self.samples)
                if quantify_free(part):
                    self.solver.add(part)
                else:
                    self.universal.append(part)
        self.instantiated = {}
        self.binders = {}
        self.total = 0
        self.maximum = 0

    def collect(self, expr, pool):
        for x in visit(expr):
            if not z3.is_app(x):
                continue
            name = self.members.get(x.decl().get_id())
            if name is None:
                continue
            args = tuple(x.children())
            if not all(quantify_free(a) for a in args):
                continue
            pool[name][tuple(a.get_id() for a in args)] = args

    def weaken(self, expr, pool, cache):
        key = expr
        if key in cache:
            return cache[key]
        if quantify_free(expr):
            cache[key] = expr
            return expr
        if z3.is_quantifier(expr):
            require(expr.is_forall(), "expected Skolemized existential")
            groups = {}
            positions = []
            for i in range(expr.num_vars()):
                name = str(expr.var_name(i)).split("!")[0]
                prefix, _, field = name.partition(".")
                channel = prefix.partition("_q")[0]
                if channel not in self.types:
                    value = z3.BoolVal(True)
                    cache[key] = value
                    return value
                typ = self.types[channel]
                if prefix not in self.binders:
                    raw = typ.arbitrary(prefix)
                    self.binders[prefix] = [str(v) for v in typ.variables(raw)]
                columns = self.binders[prefix]
                if name not in columns:
                    value = z3.BoolVal(True)
                    cache[key] = value
                    return value
                groups[prefix] = channel
                positions.append((prefix, columns.index(name)))
            sub = []
            for instances in product(*(pool[n].values() for n in groups.values())):
                points = dict(zip(groups, instances, strict=True))
                args = [points[n][col] for n, col in positions]
                subkey = (expr, tuple(args))
                if subkey not in self.instantiated:
                    self.instantiated[subkey] = z3.simplify(
                        z3.substitute_vars(expr.body(), *reversed(args))
                    )
                sub.append(self.weaken(self.instantiated[subkey], pool, cache))
            value = z3.And(*sub)
        elif z3.is_and(expr) or z3.is_or(expr):
            value = expr.decl()(*(self.weaken(x, pool, cache) for x in expr.children()))
        else:
            require(quantify_free(expr), "universal outside positive Boolean context")
            value = expr
        cache[key] = value
        return value

    def reject(self, condition, label, guard=True):
        condition = z3.simplify(
            z3.BoolVal(condition) if isinstance(condition, bool) else condition
        )
        if z3.is_false(condition):
            return
        failure = nnf(z3.And(guard, condition))
        pool = {n: dict(rows) for n, rows in self.samples.items()}
        self.collect(failure, pool)
        cache = {}
        s = self.solver
        s.push()
        try:
            s.add(self.weaken(failure, pool, cache))
            for premise in self.universal:
                s.add(self.weaken(premise, pool, cache))
            self.maximum = max(self.maximum, sum(map(len, pool.values())))
            self.total += 1
            outcome = s.check()
            if outcome != z3.unsat:
                raise ValueError(
                    f"{label}: {outcome} {s.reason_unknown()}; {[(n, len(r)) for n, r in pool.items()]}"
                )
        finally:
            s.pop()
