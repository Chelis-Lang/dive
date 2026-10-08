#!/usr/bin/env python3
"""Semantic mutation campaign.

Usage: mutate.py OUT SEED N [--ops op,op,...] FILE...

For every FILE, writes up to N mutants into OUT/<k>/, one directory per
source directory. Each directory holds symbolic links to the source
directory's other `.tla` files (so `EXTENDS`/`INSTANCE` of siblings still
resolve) and the mutants, renamed `<Stem>_m<i>.tla` with their module header
renamed to match. A mutant applies one or two semantic edits:

  undefined   rename an identifier occurrence to an undefined name
  swap        replace an identifier occurrence by another name from the file
  arity       drop or add an argument of an application `F(a, b)`
  prime       prime an identifier occurrence (`c` -> `c'`)
  temporal    wrap an identifier occurrence in `[]`, `<>`, `ENABLED` or
              `UNCHANGED`
  duplicate   duplicate a definition or declaration line
  local       add or remove `LOCAL` on a top-level definition
  shadow      rename a bound identifier to a declared name
  extends     drop a module from `EXTENDS`

`--ops` restricts the edits, e.g. `--ops prime,temporal` for a
level-checking wave. Edits are made on the text outside comments and strings. Writes
OUT/index.tsv: mutant path, source path, edits.
"""
import os
import random
import re
import sys

KEYWORDS = set("""ASSUME ASSUMPTION AXIOM BOOLEAN CASE CHOOSE CONSTANT CONSTANTS ELSE
EXCEPT EXTENDS FALSE IF IN INSTANCE LET LOCAL MODULE OTHER STRING SUBSET THEN THEOREM
TRUE UNCHANGED UNION VARIABLE VARIABLES WITH WF_ SF_ ENABLED DOMAIN LAMBDA RECURSIVE
PROOF BY OBVIOUS OMITTED QED SUFFICES PICK TAKE WITNESS HAVE NEW USE HIDE DEF DEFS
DEFINE PROVE ACTION STATE TEMPORAL ONLY LEMMA PROPOSITION COROLLARY""".split())
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def code_mask(src):
    """True for characters outside comments and strings."""
    mask = [True] * len(src)
    i, n, depth = 0, len(src), 0
    while i < n:
        if depth == 0 and src.startswith("\\*", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                mask[k] = False
            i = j
            continue
        if src.startswith("(*", i):
            depth += 1
            mask[i] = mask[i + 1] = False
            i += 2
            continue
        if depth > 0:
            if src.startswith("*)", i):
                depth -= 1
                mask[i] = mask[i + 1] = False
                i += 2
                continue
            mask[i] = False
            i += 1
            continue
        if src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            for k in range(i, min(j + 1, n)):
                mask[k] = False
            i = j + 1
            continue
        i += 1
    return mask


def body_range(src):
    """(start, end) of the module body: after the header line, before the
    first closing `====` line."""
    m = re.search(r"^-{4,}\s*MODULE\s+\S+.*$", src, re.M)
    start = m.end() if m else 0
    e = re.search(r"^={4,}", src[start:], re.M)
    end = start + e.start() if e else len(src)
    return start, end


def idents(src, mask):
    s, e = body_range(src)
    out = []
    for m in IDENT.finditer(src, s, e):
        if not mask[m.start()] or m.group() in KEYWORDS:
            continue
        if m.start() > 0 and (src[m.start() - 1] in "\\<" or src[m.start() - 1].isalnum()):
            continue
        line_start = src.rfind("\n", 0, m.start()) + 1
        if src[line_start:m.start()].lstrip().startswith("EXTENDS"):
            continue
        out.append(m)
    return out


def mut_undefined(src, mask, rnd):
    ids = idents(src, mask)
    if not ids:
        return None
    m = rnd.choice(ids)
    return src[:m.start()] + "ZzUndef" + src[m.end():], "undefined %s" % m.group()


def mut_swap(src, mask, rnd):
    ids = idents(src, mask)
    names = sorted({m.group() for m in ids})
    if len(names) < 2:
        return None
    m = rnd.choice(ids)
    other = rnd.choice([n for n in names if n != m.group()])
    return src[:m.start()] + other + src[m.end():], "swap %s->%s" % (m.group(), other)


def matching(src, i):
    """Index of the bracket closing the one at i."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = []
    j = i
    while j < len(src):
        c = src[j]
        if c in pairs:
            stack.append(pairs[c])
        elif c in ")]}":
            if not stack or stack.pop() != c:
                return None
            if not stack:
                return j
        j += 1
    return None


def mut_arity(src, mask, rnd):
    cands = [m for m in idents(src, mask) if src[m.end():m.end() + 1] == "("]
    if not cands:
        return None
    m = rnd.choice(cands)
    close = matching(src, m.end())
    if close is None:
        return None
    inner = src[m.end() + 1:close]
    if rnd.random() < 0.5:
        return src[:close] + ", 0" + src[close:], "arity +1 %s" % m.group()
    depth = 0
    for k, c in enumerate(inner):
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "," and depth == 0:
            return src[:m.end() + 1] + inner[k + 1:] + src[close:], "arity -1 %s" % m.group()
    return None


def mut_prime(src, mask, rnd):
    ids = idents(src, mask)
    if not ids:
        return None
    m = rnd.choice(ids)
    if src[m.end():m.end() + 1] in ("(", "[", "'", "!"):
        return None
    return src[:m.end()] + "'" + src[m.end():], "prime %s" % m.group()


def mut_temporal(src, mask, rnd):
    ids = idents(src, mask)
    if not ids:
        return None
    m = rnd.choice(ids)
    if src[m.end():m.end() + 1] in ("(", "[", "!"):
        return None
    op = rnd.choice(["[]", "<>", "ENABLED ", "UNCHANGED "])
    return src[:m.start()] + "(" + op + m.group() + ")" + src[m.end():], "temporal %s%s" % (op, m.group())


DEF_LINE = re.compile(r"^(LOCAL\s+)?([A-Za-z_]\w*)(\([^)]*\))?\s*==", re.M)
DECL_LINE = re.compile(r"^(VARIABLES?|CONSTANTS?)\s+([A-Za-z_]\w*)", re.M)


def mut_duplicate(src, mask, rnd):
    s, e = body_range(src)
    defs = [m for m in DEF_LINE.finditer(src, s, e) if mask[m.start()]]
    decls = [m for m in DECL_LINE.finditer(src, s, e) if mask[m.start()]]
    if decls and rnd.random() < 0.3:
        m = rnd.choice(decls)
        kw = m.group(1).rstrip("S")
        line = "%s %s\n" % (kw, m.group(2))
        return src[:m.start()] + line + src[m.start():], "duplicate decl %s" % m.group(2)
    if not defs:
        return None
    m = rnd.choice(defs)
    line = "%s%s == TRUE\n" % (m.group(2), m.group(3) or "")
    return src[:m.start()] + line + src[m.start():], "duplicate def %s" % m.group(2)


def mut_local(src, mask, rnd):
    s, e = body_range(src)
    defs = [m for m in DEF_LINE.finditer(src, s, e) if mask[m.start()]]
    if not defs:
        return None
    m = rnd.choice(defs)
    if m.group(1):
        return src[:m.start()] + src[m.start() + len(m.group(1)):], "unlocal %s" % m.group(2)
    return src[:m.start()] + "LOCAL " + src[m.start():], "local %s" % m.group(2)


def mut_shadow(src, mask, rnd):
    s, e = body_range(src)
    names = [m.group(2) for m in DECL_LINE.finditer(src, s, e)] + [
        m.group(2) for m in DEF_LINE.finditer(src, s, e)]
    binders = [m for m in re.finditer(r"(\\A|\\E|CHOOSE|\\in)\s+([A-Za-z_]\w*)", src[:e])
               if m.start() > s and mask[m.start()] and m.group(2) not in KEYWORDS]
    binders = [m for m in binders if m.group(1) != "\\in"]
    if not binders or not names:
        return None
    m = rnd.choice(binders)
    new = rnd.choice(names)
    return src[:m.start(2)] + new + src[m.end(2):], "shadow %s->%s" % (m.group(2), new)


def mut_extends(src, mask, rnd):
    m = re.search(r"^EXTENDS\s+(.*)$", src, re.M)
    if not m:
        return None
    mods = [x.strip() for x in m.group(1).split(",") if x.strip()]
    if len(mods) < 1:
        return None
    drop = rnd.choice(mods)
    rest = [x for x in mods if x != drop]
    line = ("EXTENDS " + ", ".join(rest)) if rest else ""
    return src[:m.start()] + line + src[m.end():], "extends -%s" % drop


OPS = [mut_undefined, mut_swap, mut_arity, mut_arity, mut_prime, mut_temporal,
       mut_temporal, mut_duplicate, mut_local, mut_shadow, mut_extends, mut_swap]


def main():
    out, seed, n = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    files = sys.argv[4:]
    ops = OPS
    if files and files[0] == "--ops":
        names = files[1].split(",")
        ops = [globals()["mut_" + x] for x in names]
        files = files[2:]
    rnd = random.Random(seed)
    os.makedirs(out, exist_ok=True)
    dirs = {}
    index = []
    for f in files:
        f = os.path.abspath(f)
        try:
            src = open(f, encoding="utf-8").read()
        except (UnicodeDecodeError, OSError):
            continue
        stem = os.path.basename(f)[:-4]
        if not re.search(r"^-{4,}\s*MODULE\s+" + re.escape(stem) + r"\b", src, re.M):
            continue
        d = os.path.dirname(f)
        if d not in dirs:
            md = os.path.join(out, str(len(dirs)))
            os.makedirs(md, exist_ok=True)
            for sib in os.listdir(d):
                if sib.endswith(".tla"):
                    link = os.path.join(md, sib)
                    if not os.path.lexists(link):
                        os.symlink(os.path.join(d, sib), link)
            dirs[d] = md
        md = dirs[d]
        made = 0
        tries = 0
        while made < n and tries < n * 6:
            tries += 1
            cur, edits = src, []
            for _ in range(1 if rnd.random() < 0.7 else 2):
                mask = code_mask(cur)
                r = rnd.choice(ops)(cur, mask, rnd)
                if r:
                    cur = r[0]
                    edits.append(r[1])
            if not edits or cur == src:
                continue
            name = "%s_m%d" % (stem, made)
            cur = re.sub(r"^(-{4,}\s*MODULE\s+)" + re.escape(stem) + r"\b", r"\g<1>" + name, cur,
                         count=1, flags=re.M)
            p = os.path.join(md, name + ".tla")
            with open(p, "w", encoding="utf-8") as fh:
                fh.write(cur)
            index.append((p, f, "; ".join(edits)))
            made += 1
    with open(os.path.join(out, "index.tsv"), "w") as fh:
        for row in index:
            fh.write("\t".join(row) + "\n")
    print("mutants: %d in %d directories" % (len(index), len(dirs)))


if __name__ == "__main__":
    main()
