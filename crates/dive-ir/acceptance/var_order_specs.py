"""Writes the variable-order fixture specifications (see README.md):

    python3 -I var_order_specs.py tests/fixtures/var-order/specs
"""
import os
import sys

out = sys.argv[1]

# Names: lower, upper, underscores, digits, long, and Java-hash collisions
# (Aa/BB blocks all share one hash).
POOL = [
    "x", "y", "z", "pc", "msgs", "Q", "state", "a1", "b_2", "votes", "rmState",
    "tmPrepared", "AaAa", "BBBB", "AaBB", "BBAa", "counter", "q", "aVeryLongVariableName_01",
    "chan", "ballot", "maxBal", "maxVBal", "maxVal", "sent", "rcvd", "inbox", "outbox",
    "leader", "term", "log", "commitIndex", "nextIndex", "matchIndex", "votesGranted",
    "votedFor", "elections", "allLogs", "messages", "currentTerm", "clock", "N1", "N2",
    "flag", "turn", "lock", "owner", "waiting", "cs", "done", "Aa", "BB", "C_", "d9",
]


def value(k, step):
    """The value of the k-th variable (0-based) after `step` steps."""
    kind = k % 4
    if kind == 0:
        return str(10 * (k + 1) + step)
    if kind == 1:
        return "TRUE" if (k + step) % 2 == 0 else "FALSE"
    if kind == 2:
        return '"s%d_%d"' % (k, step)
    return '<<%d, "t", %s>>' % (k + step, "TRUE" if step % 2 else "FALSE")


def body(names, extends=None, pre="", post="", counter=None):
    """A module body over `names` (declared here or extended) with 4 states."""
    c = counter or names[0]
    ci = names.index(c)
    lines = []
    for step in range(3):
        conj = []
        for k, n in enumerate(names):
            if n == c:
                continue
            conj.append("  /\\ %s' = %s" % (n, value(k, step + 1)))
        lines.append("  \\/ /\\ %s = %d\n     /\\ %s' = %d\n   %s" % (
            c, 10 * (ci + 1) + step, c, 10 * (ci + 1) + step + 1,
            "\n   ".join(conj)))
    init = "\n".join("  /\\ %s = %s" % (n, value(k, 0)) for k, n in enumerate(names))
    return "%sInit ==\n%s\nNext ==\n%s\n%s" % (pre, init, "\n".join(lines), post)


def write(name, text, cfg="INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n", extra=None):
    d = os.path.join(out, name)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, name + ".tla"), "w") as f:
        f.write(text)
    with open(os.path.join(d, name + ".cfg"), "w") as f:
        f.write(cfg)
    for fname, t in (extra or {}).items():
        with open(os.path.join(d, fname), "w") as f:
            f.write(t)


def module(name, ext, decl_names, pre="", post="", all_names=None, counter=None):
    head = "---- MODULE %s ----\n" % name
    if ext:
        head += "EXTENDS %s\n" % ", ".join(ext)
    if decl_names:
        head += "VARIABLES %s\n" % ", ".join(decl_names)
    return head + body(all_names or decl_names, pre=pre, post=post, counter=counter) + "====\n"


# 1. Variable counts across the Hashtable's resize thresholds, with and
#    without standard modules (which change the context's size).
for n in (1, 2, 5, 11, 12, 13, 25, 50):
    names = POOL[:n] if n <= len(POOL) else POOL + ["v%d" % i for i in range(n - len(POOL))]
    write("Vars%d" % n, module("Vars%d" % n, ["Naturals"], names))
    rev = list(reversed(names))
    write("VarsBare%d" % n, module("VarsBare%d" % n, [], rev))
    write("VarsStd%d" % n, module("VarsStd%d" % n,
                                   ["Integers", "Sequences", "FiniteSets", "TLC", "Bags"], names))

# 2. Colliding hashes: the Aa/BB family, in both declaration orders.
fam = ["AaAa", "BBBB", "AaBB", "BBAa"]
write("Collide", module("Collide", ["Naturals"], ["n"] + fam))
write("CollideRev", module("CollideRev", ["Naturals"], ["n"] + list(reversed(fam))))

# 3. Variables inserted in different resize epochs: an extended module's
#    variables come first, then many definitions, then the root's variables.
defs = "".join("D%d == %d\n" % (i, i) for i in range(300))
base = "---- MODULE EpochBase ----\nEXTENDS Naturals\nVARIABLES AaAa, x, msgs\n====\n"
write("Epochs", "---- MODULE Epochs ----\nEXTENDS EpochBase\n" + defs +
      "VARIABLES BBBB, y, AaBB, pc\n" +
      body(["x", "AaAa", "msgs", "BBBB", "y", "AaBB", "pc"], counter="x") + "====\n",
      extra={"EpochBase.tla": base})

# 4. INSTANCE (named and unnamed), LOCAL INSTANCE, and an inner module (a
#    module-name key in the context).
inner = ("---- MODULE Inst ----\nEXTENDS Naturals\nVARIABLES x, y\n"
         "IA == x + 1\nIB == y\nIC(a) == a + x\nID == <<x, y>>\n====\n")
write("Instances",
      "---- MODULE Instances ----\nEXTENDS Naturals\nVARIABLES x, y, BBBB, state\n"
      "I == INSTANCE Inst\nJ == INSTANCE Inst WITH x <- y, y <- x\n"
      "INSTANCE Inst\nLOCAL INSTANCE Sequences\n"
      "---- MODULE Sub ----\nVARIABLE AaAa\nS1 == AaAa\n====\n"
      "VARIABLES AaAa, msgs\n" +
      body(["x", "y", "BBBB", "state", "AaAa", "msgs"]) + "====\n",
      extra={"Inst.tla": inner})

# 5. Variables from several extended modules, with a diamond.
ma = "---- MODULE ExtA ----\nEXTENDS Naturals\nVARIABLES a1, pc\nA1 == a1\n====\n"
mb = "---- MODULE ExtB ----\nEXTENDS ExtA, Sequences\nVARIABLES msgs, BBBB\nB1 == msgs\n====\n"
mc = "---- MODULE ExtC ----\nEXTENDS ExtA, FiniteSets\nVARIABLES AaAa, q\nC1 == q\n====\n"
write("Extends",
      "---- MODULE Extends ----\nEXTENDS ExtB, ExtC, TLC\nVARIABLES z, votes\n" +
      body(["a1", "pc", "msgs", "BBBB", "AaAa", "q", "z", "votes"], counter="a1") + "====\n",
      extra={"ExtA.tla": ma, "ExtB.tla": mb, "ExtC.tla": mc})
