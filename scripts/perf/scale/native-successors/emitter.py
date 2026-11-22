"""Lower Core's bound generator plan to Rust continuations in a private build."""
import ast
from functools import cache
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
LIMIT = 4096


def natural(value):
    if type(value) is not int or not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("expected a generator operand index")
    return value


def operand_list(text):
    parsed = ast.literal_eval(text)
    if not isinstance(parsed, list):
        raise ValueError("expected a generator operand list")
    return [natural(value) for value in parsed]


def rust_list(values):
    return "&[" + ",".join(str(natural(x)) for x in values) + "]"


def continuations(values):
    return "&[" + ",".join(f"({natural(g)},{natural(p)})" for g, p in values) + "]"


def lower(plan):
    roots, nodes = [], []
    for line in plan.splitlines():
        tag, value = line.split(" ", 1)
        if tag == "root":
            roots.append(natural(int(value)))
        elif tag == "node":
            nodes.append(value)
        else:
            raise ValueError("unrecognized generator export record")
    if not roots or not nodes:
        raise ValueError("input has no Next generator")
    functions = {}
    kinds = {}

    def function(body):
        if len(functions) >= LIMIT:
            raise ValueError(f"generated worker exceeds {LIMIT} continuations")
        name = f"next_{len(functions)}"
        functions[name] = body
        return name

    def call(name):
        return f"{name}(m,c,s,r)"

    @cache
    def tail(pending):
        if not pending:
            return "end"
        if len(pending) > 64:
            raise ValueError("native continuation depth exceeds 64")
        first, _ = pending[0]
        following = branch(first, pending[1:])
        tests = " && ".join(f"m.eval_bool(c,{p})?" for _, p in pending)
        return function(f"if m.warn && m.assigned==m.s1.len(){{if {tests}{{s.emit(m,c)}}else{{Ok(())}}}}else{{{call(following)}}}")

    def parts(node):
        raw = ast.literal_eval(re.search(r"\(\s*(\[.*?\])", node).group(1))
        return [(natural(g), natural(p)) for g, p in raw]

    def alternatives(g):
        # Preserve disjunction order without expanding an exponentially shared
        # source DAG. Sites over the limit retain ordinary continuations.
        pending, leaves = [g], []
        visits = 0
        while pending:
            child = pending.pop()
            node = nodes[child]
            visits += 1
            if visits > 128:
                return []
            if node.startswith("Disj("):
                pending.extend(reversed(operand_list(node[5:-1])))
            else:
                leaves.append(child)
            if len(pending) + len(leaves) > 32:
                return []
        return leaves

    def leading(g):
        todo = [(g, 0)]
        predicates = []
        visits = 0
        while todo:
            visits += 1
            if visits > 128 or len(todo) > 64 or len(predicates) > 4:
                return [], ()
            first, _ = todo[0]
            node = nodes[first]
            if node.startswith(("Conj(", "LabelArm(")):
                todo = parts(node) + todo[1:]
                continue
            label = re.fullmatch(r"LabelGuard\(\d+, LabelTest \{ table: (\d+), slot: (\d+), field: (Empty|Str#\d+), selector: \d+, label: Str#(\d+) \}\)", node)
            if not label:
                break
            table, slot, field, value = label.groups()
            if predicates and (table, slot) != predicates[0][:2]:
                break
            predicates.append((table, slot, field, natural(int(value))))
            todo = todo[1:]
        return predicates, tuple(todo)

    def routed(g, pending):
        leaves = alternatives(g)
        if not 3 <= len(leaves) <= 32:
            return None
        arms = [leading(arm) for arm in leaves]
        bases = {test[:2] for tests, _ in arms for test in tests}
        if len(bases) != 1 or sum(bool(t) for t, _ in arms) < 3:
            return None
        table, slot = next(iter(bases))
        fields = {}
        for arm, (tests, _) in enumerate(arms):
            for _, _, field, value in tests:
                match, restricted = fields.setdefault(field, ({}, set()))
                if arm in restricted:
                    return None
                match[value] = match.get(value, 0) | (1 << arm)
                restricted.add(arm)
        if not fields or len(fields)>4:
            return None
        code = f"if c.native_blocks.enabled && m.assigned==0 && !m.s1.is_empty() && let Some(row)=r.actor(m,{table},{slot})"
        masks = []
        for i,(field,(values,restricted)) in enumerate(fields.items()):
            arg = "Handle::EMPTY" if field=="Empty" else f"Handle::string({natural(int(field[4:]))})"
            code += f" && let Some(label{i})=r.field(m,row,{arg}).and_then(Handle::as_str_id)"
            kept = sum(1 << j for j in range(len(arms)) if j not in restricted)
            choices = "".join(f"{key}=>{mask|kept}u32," for key,mask in values.items())
            masks.append(f"(match label{i}{{{choices}_=>{kept}u32}})")
        code += "{c.native_blocks.record(crate::native_blocks::Event::Route);let active=" + "&".join(masks) + ";"
        for i,(tests,rest) in enumerate(arms):
            next_fn = tail(rest + pending) if tests else branch(leaves[i],pending)
            code += f"if active & {1<<i}u32!=0{{{call(next_fn)}?;}}"
        # Unknown carriers or field classes need the original first failing
        # expression. All successful arms retain their order and multiplicity.
        fallback = "".join(call(branch(arm,pending))+"?;" for arm in leaves)
        return function(code+"Ok(())}else{"+fallback+"Ok(())}")

    @cache
    def branch(g, pending):
        node = nodes[g]
        kind = re.match(r"[A-Za-z]+", node).group()
        kinds[kind] = kinds.get(kind, 0) + 1

        def field(name):
            return natural(int(re.search(rf"\b{name}: (\d+)", node).group(1)))

        following = tail(pending)
        done = call(following)
        if kind in {"Conj", "LabelArm"}:
            raw = ast.literal_eval(re.search(r"\(\s*(\[.*?\])", node).group(1))
            parts = [(natural(child), natural(predicate)) for child, predicate in raw]
            target = branch(parts[0][0], tuple(parts[1:]) + pending) if parts else following
            if kind == "Conj":
                return target
            return function(f"if let GNode::LabelArm(_,x)=&c.gnodes[{g}]{{if m.known_label(*x,m.slot(x.slot)).is_some_and(|v|v!=x.label){{return Ok(())}}}} {call(target)}")
        if kind == "Disj":
            specialized = routed(g, pending)
            if specialized is not None:
                return specialized
            arms = operand_list(node.removeprefix("Disj(").removesuffix(")"))
            return function("".join(call(branch(child, pending)) + "?;" for child in arms) + "Ok(())")
        if kind == "If":
            yes, no = branch(field("then"), pending), branch(field("other"), pending)
            return function(f"if m.eval_bool(c,{field('cond')})?{{{call(yes)}}}else{{{call(no)}}}")
        if kind == "Implies":
            rhs = branch(field("rhs"), pending)
            return function(f"if m.eval_bool(c,{field('lhs')})?{{{call(rhs)}}}else{{{done}}}")
        if kind == "Guard":
            p = natural(int(re.fullmatch(r"Guard\((\d+)\)", node).group(1)))
            return function(f"if m.eval_bool(c,{p})?{{{done}}}else{{Ok(())}}")
        if kind == "LabelGuard":
            return function(f"let GNode::LabelGuard(p,x)=&c.gnodes[{g}]else{{return Err(Error::internal(\"native label shape\"))}};if m.label_matches(c,*p,*x)?{{{done}}}else{{Ok(())}}")
        if kind == "Assign":
            return function(f"assign(m,c,s,r,{g},{field('var')},{field('rhs')},{following})")
        if kind == "Scope":
            slots = operand_list(re.search(r"\[.*?\]", node).group())
            return function(f"scope(m,c,s,r,{rust_list(slots)},{branch(field('body'),pending)})")
        if kind == "Exists":
            slots = [slot for group in re.findall(r"slots: (\[.*?\])", node) for slot in operand_list(group)]
            return function(f"exists(m,c,s,r,{g},{rust_list(slots)},{branch(field('body'),pending)})")
        if kind == "Unchanged" and "Verify" not in node:
            columns = [natural(int(value)) for value in re.findall(r"var: (\d+)", node)]
            if len(columns) == len(set(columns)):
                return function(f"copy(m,c,s,r,{rust_list(columns)},{g},{continuations(pending)},{following})")
        return function(f"generic(m,c,s,r,{g},{continuations(pending)})")

    arms = [f"{root}=>{{observe({root});{call(branch(root, ()))}}}, " for root in dict.fromkeys(roots)]
    dispatch = "pub(super) fn run(m:&mut Machine,c:&Code,g:u32,s:&mut dyn Sink)->Result<(),Error>{let mut local=Register::default();let r=&mut local;match g{" + "".join(arms) + "_=>m.run_action(c,g,s),}}"
    # Some generic fallback sites consume the whole pending continuation.
    # Emit only routines reachable from the bound action roots.
    used = set()
    pending = re.findall(r"next_\d+", dispatch)
    while pending:
        name = pending.pop()
        if name not in used:
            used.add(name)
            pending.extend(re.findall(r"next_\d+", functions[name]))
    bodies = [f"fn {name}(m:&mut Machine,c:&Code,s:&mut dyn Sink,r:&mut Register)->Result<(),Error>{{{body}}}"
              for name, body in functions.items() if name in used]
    prefix = (HERE / "runtime.rs").read_text()
    source = "\n".join([prefix, (HERE / "binding.rs").read_text(), *bodies, dispatch])
    return source, "".join(node + "\n" for node in nodes), {"roots": len(set(roots)), "generator_nodes": len(nodes), "continuations": len(bodies), "kinds": kinds}
