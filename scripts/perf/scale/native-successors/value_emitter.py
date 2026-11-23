"""Emit one Rust routine per checked expression shape; operands bind at startup."""
import re


def body(shape):
    tokens = iter(shape.split())
    operand = 0

    def visit(depth=0):
        nonlocal operand
        if depth > 16:
            raise ValueError("operator exceeds the native nesting bound")
        tag = next(tokens, None)
        if tag in {"c", "s", "v", "p", "field", "x"}:
            arg = f"a[{operand}]"
            operand += 1
            if tag == "c":
                return arg + ".value"
            if tag == "field":
                child = visit(depth + 1)
                return f"{{let v={child}; op::field(m,v,{arg})?}}"
            if tag in ("v", "p"):
                return f"op::state(m,{arg},{str(tag == 'p').lower()})?"
            if tag == "s":
                return f"op::slot(m,{arg})?"
            return f"op::residual(m,c,{arg})?"
        if tag == "not":
            return f"Handle::bool(!op::boolean({visit(depth + 1)})?)"
        if tag and (group := re.fullmatch(r"(and|or)([1-9]|1[0-2])", tag)):
            join = "&&" if group[1] == "and" else "||"
            parts = [f"op::boolean({visit(depth + 1)})?" for _ in range(int(group[2]))]
            return "Handle::bool(" + join.join(parts) + ")"
        if tag not in ("eq", "ne", "in", "ni", "at", "imp"):
            raise ValueError(f"invalid native operator instruction {tag!r}")
        left, right = visit(depth + 1), visit(depth + 1)
        if tag == "imp":
            return f"Handle::bool(!op::boolean({left})? || op::boolean({right})?)"
        if tag == "at":
            return f"{{let x={left};let y={right};let y=m.work_norm(y)?;m.apply(x,y)?}}"
        same = f"let x={left};let x=m.work_norm(x)?;let y={right};"
        invert = "!" if tag in ("ne", "ni") else ""
        if tag in ("eq", "ne"):
            return "{" + same + f"let y=m.work_norm(y)?;Handle::bool({invert}crate::compare::equals(&m.heap,x,y)?)" + "}"
        return "{" + same + f"Handle::bool({invert}m.member(x,y)?)" + "}"

    if len(shape.split()) > 80:
        raise ValueError("operator exceeds the native size bound")
    rust = visit()
    if next(tokens, None) is not None:
        raise ValueError("trailing native operator instruction")
    return rust


def emit(shapes):
    # Both caps bound compiler input. The runtime plan continues to evaluate
    # every shape not selected here with Core.
    shapes = sorted(set(shapes))[:192]
    lines = ["use crate::eval::operators as op;"]
    choices = []
    for i, shape in enumerate(shapes):
        rust = body(shape)
        lines.append(f"#[allow(unused_variables)] fn value_{i}(m:&mut Machine,c:&Code,a:&[op::Arg])->Result<Handle,Error>{{Ok({rust})}}")
        choices.append(f'"{shape}" => Some(value_{i} as op::Body),')
    lines.append("pub(crate) fn operator(shape:&str)->Option<op::Body>{match shape{" + "".join(choices) + "_ => None}}")
    return "\n".join(lines), len(shapes)
