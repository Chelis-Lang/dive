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
    functions = []
    kinds = {}

    def function(body):
        if len(functions) >= LIMIT:
            raise ValueError(f"generated worker exceeds {LIMIT} continuations")
        name = f"next_{len(functions)}"
        functions.append(f"fn {name}(m:&mut Machine,c:&Code,s:&mut dyn Sink)->Result<(),Error>{{{body}}}")
        return name

    def call(name):
        return f"{name}(m,c,s)"

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
            return function(f"assign(m,c,s,{field('var')},{field('rhs')},{following})")
        if kind == "Scope":
            slots = operand_list(re.search(r"\[.*?\]", node).group())
            return function(f"scope(m,c,s,{rust_list(slots)},{branch(field('body'),pending)})")
        if kind == "Exists":
            slots = [slot for group in re.findall(r"slots: (\[.*?\])", node) for slot in operand_list(group)]
            return function(f"exists(m,c,s,{g},{rust_list(slots)},{branch(field('body'),pending)})")
        if kind == "Unchanged" and "Verify" not in node:
            columns = [natural(int(value)) for value in re.findall(r"var: (\d+)", node)]
            if len(columns) == len(set(columns)):
                return function(f"copy(m,c,s,{rust_list(columns)},{g},{continuations(pending)},{following})")
        return function(f"generic(m,c,s,{g},{continuations(pending)})")

    arms = [f"{root}=>{{observe({root});{call(branch(root, ()))}}}, " for root in dict.fromkeys(roots)]
    dispatch = "pub(super) fn run(m:&mut Machine,c:&Code,g:u32,s:&mut dyn Sink)->Result<(),Error>{match g{" + "".join(arms) + "_=>m.run_action(c,g,s),}}"
    prefix = (HERE / "runtime.rs").read_text()
    source = "\n".join([prefix, (HERE / "binding.rs").read_text(), *functions, dispatch])
    return source, "".join(node + "\n" for node in nodes), {"roots": len(set(roots)), "generator_nodes": len(nodes), "continuations": len(functions), "kinds": kinds}
