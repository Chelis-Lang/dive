#!/usr/bin/env python3
"""Generate mutants of TLA+ modules for differential testing against SANY.

Usage: mutate.py OUT_DIR SEED N_PER_FILE FILE...

Each mutant is written to OUT_DIR/NNNNN/<original basename> so the module
name still matches the file name. Mutations operate on a rough tokenisation
(they need not be exact; any text is a valid test input):

  delete      remove one token
  duplicate   repeat one token
  swap        swap two adjacent tokens
  replace-op  replace an operator with another operator
  insert      insert a token from a TLA+ vocabulary
  indent      shift one line left or right by 1-3 columns
  join        join a line with the next one
"""
import os
import random
import re
import sys

TOKEN = re.compile(
    r'\(\*|\*\)|\\\*|"(?:[^"\\\n]|\\.)*"|<<|>>_|>>|\]_|<\d+>[\w.]*|[A-Za-z0-9_]+'
    r'|\\[A-Za-z]+|==|=>|<=>|->|<-|\|->|::|/\\|\\/|\.\.|[^\sA-Za-z0-9_]'
)

OPS = ["+", "-", "*", "/", "^", "=", "#", "/=", "<", ">", "<=", ">=", "\\in", "\\notin",
       "\\cup", "\\cap", "\\", "\\X", "\\times", "..", "/\\", "\\/", "=>", "<=>", "~>",
       "-+->", "\\o", "\\cdot", "%", "&", "|", "++", "**", "@@", ":>", "\\subseteq",
       "\\prec", "~", "[]", "<>", "'", "^+", "ENABLED", "UNCHANGED", "SUBSET", "UNION",
       "DOMAIN", "-.", "\\lnot", "≜", "∧", "∨", "∈", "□", "◇", "⟨", "⟩", "×", "∃", "∀"]

VOCAB = OPS + ["(", ")", "[", "]", "{", "}", "<<", ">>", ",", ":", "::", ".", "!", "@",
               "LET", "IN", "IF", "THEN", "ELSE", "CASE", "OTHER", "[]", "->", "|->",
               "\\A", "\\E", "\\AA", "\\EE", "CHOOSE", "LAMBDA", "EXCEPT", "WF_", "SF_",
               "]_", ">>_", "==", "x", "1", "2.5", '"s"', "TRUE", "<1>1.", "<1>", "<2>1.",
               "QED", "BY", "OBVIOUS", "PROOF", "DEF", "USE", "HIDE", "ASSUME", "PROVE",
               "NEW", "SUFFICES", "PICK", "TAKE", "WITNESS", "HAVE", "DEFINE", "THEOREM",
               "LOCAL", "INSTANCE", "WITH", "RECURSIVE", "CONSTANT", "VARIABLE", "_",
               "----", "====", "\\b101", "\\h1F", "lab", "-|-", "(* c *)", "\\* c\n"]


def mutate(src, rng):
    lines = src.split("\n")
    kind = rng.choice(["delete", "delete", "duplicate", "swap", "replace-op", "replace-op",
                       "replace-op", "insert", "insert", "indent", "indent", "indent", "join"])
    if kind in ("indent", "join"):
        idx = [i for i, l in enumerate(lines) if l.strip() and not l.startswith("----")
               and not l.startswith("====")]
        if not idx:
            return None
        i = rng.choice(idx)
        if kind == "indent":
            d = rng.choice([-3, -2, -1, 1, 2, 3])
            if d < 0:
                k = len(lines[i]) - len(lines[i].lstrip(" "))
                lines[i] = lines[i][min(k, -d):]
            else:
                lines[i] = " " * d + lines[i]
        else:
            if i + 1 >= len(lines):
                return None
            lines[i] = lines[i] + " " + lines[i + 1].lstrip()
            del lines[i + 1]
        return "\n".join(lines)
    toks = [m for m in TOKEN.finditer(src)]
    # Avoid the module header line.
    hdr = re.search(r"^-{4,}\s*MODULE.*$", src, re.M)
    header_end = hdr.end() if hdr else src.find("\n")
    toks = [m for m in toks if m.start() > header_end]
    if len(toks) < 3:
        return None
    m = rng.choice(toks[:-1])
    s, e = m.start(), m.end()
    if kind == "delete":
        return src[:s] + src[e:]
    if kind == "duplicate":
        return src[:e] + " " + m.group() + src[e:]
    if kind == "swap":
        j = toks.index(m)
        n = toks[j + 1]
        return src[:s] + n.group() + src[e:n.start()] + m.group() + src[n.end():]
    if kind == "replace-op":
        ops = [t for t in toks if t.group() in OPS]
        if not ops:
            return None
        m = rng.choice(ops)
        return src[:m.start()] + rng.choice(OPS) + src[m.end():]
    # insert
    sep = rng.choice([" ", "", "\n  "])
    return src[:s] + rng.choice(VOCAB) + sep + src[s:]


def main():
    out, seed, per = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    rng = random.Random(seed)
    n = 0
    for f in sys.argv[4:]:
        try:
            src = open(f, encoding="utf-8").read()
        except (UnicodeDecodeError, OSError):
            continue
        made = 0
        tries = 0
        while made < per and tries < per * 5:
            tries += 1
            m = mutate(src, rng)
            if m is None or m == src:
                continue
            if rng.random() < 0.3:
                m2 = mutate(m, rng)
                if m2:
                    m = m2
            d = os.path.join(out, "%05d" % n)
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, os.path.basename(f)), "w", encoding="utf-8") as fh:
                fh.write(m)
            n += 1
            made += 1
    print(n)


if __name__ == "__main__":
    main()
