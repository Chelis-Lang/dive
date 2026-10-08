"""Writes the dive-config mutant fixtures: <out>/<Name>/<Name>.tla and .cfg.

The base module violates `Inv` at depth 3, so a cfg that silently drops its
INVARIANT is visible as a success.
"""
import os, sys

out = sys.argv[1]

BASE = """---- MODULE {name} ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == x < 2
====
"""

# A module that defines every name the accepted-keyword cfg refers to.
ALL = """---- MODULE {name} ----
EXTENDS Naturals, TLC
CONSTANTS N, S, R, B, E, M, T, U
VARIABLE x
Tdef == 7
Init == x = 0
Next == x' = (x + 1) % 3
Inv1 == x \\in 0..2
Inv2 == N = 3
Inv3 == T = 7
Prop == []Inv1
Prop2 == [][x' /= x]_x
Con == x < 10
Con2 == TRUE
ACon == x' /= x
ACon2 == TRUE
V == x
Al == [x |-> x, s |-> S]
Post == TLCGet("distinct") = 3
Post2 == TLCGet("distinct") > 0 /\\ U = "a\\\\nb"
ASSUME M = M /\\ S = S /\\ R = {"s", "t"} /\\ B /\\ E = {}
====
"""

CASES = {
    # --- accepted ---------------------------------------------------------
    "CfgAllKeywords": (ALL, r"""\* A line comment.
(* A block (* nested *) comment. *)
CONSTANTS
  N = 3
  S = {a, b}
  R = {"s", "t"}
  B = TRUE
  E = {}
  M = m
  U = "a\nb"
CONSTANT T <- Tdef
INIT Init
NEXT Next
INVARIANT Inv1 Inv2
INVARIANTS Inv3
PROPERTY Prop
PROPERTIES Prop2
CONSTRAINT Con
CONSTRAINTS Con2
ACTION_CONSTRAINT ACon
ACTION_CONSTRAINTS ACon2
VIEW V
ALIAS Al
POSTCONDITION Post
POSTCONDITIONS Post2
CHECK_DEADLOCK FALSE
"""),
    # Two INVARIANT sections append; the second invariant is violated.
    "CfgRepeatedSection": (BASE.replace("Inv == x < 2", "Ok == TRUE\nInv == x < 2"),
                           "CONSTANT N = 1\nINIT Init\nNEXT Next\nINVARIANT Ok\nINVARIANT Inv\n"),
    # A trailing \* comment without a newline is fine (TLC appends one).
    "CfgTrailingComment": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nINVARIANT Inv\n\\* no newline"),
    # String values keep their escapes verbatim: "a\nb" is four characters.
    "CfgStringRaw": (BASE.replace("Inv == x < 2", "Inv == x < 2\nASSUME N = \"a\\\\nb\""),
                     "CONSTANT N = \"a\\nb\"\nINIT Init\nNEXT Next\n"),
    # A keyword is a valid value for a single-name keyword: INIT's value is NEXT.
    "CfgKeywordAsValue": (BASE, "CONSTANT N = 1\nINIT NEXT\nNEXT Next\n"),
    # Commas are not separators: `,` becomes an invariant name.
    "CfgCommaList": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nINVARIANT Inv, Inv\n"),
    # `{1,}}` is the set {1, `}`}: the token after a comma is always a value.
    "CfgSetTrailingComma": (BASE, "CONSTANT N = {1,}}\nINIT Init\nNEXT Next\n"),
    # Later assignments to the same constant.
    "CfgDuplicateConstant": (BASE.replace("Inv == x < 2", "Inv == x < 2\nASSUME N = 2"),
                             "CONSTANT N = 1\nCONSTANT N = 2\nINIT Init\nNEXT Next\n"),
    # An empty cfg parses; TLC then reports the missing INIT.
    "CfgEmpty": (BASE, ""),
    # --- rejected by the cfg parser ----------------------------------------
    "CfgLexBadChar": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\n;\nINVARIANT Inv\n"),
    "CfgLexString": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\n\"open\nINVARIANT Inv\n"),
    "CfgLexComment": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nINVARIANT Inv\n(* open"),
    "CfgLexBom": (BASE, "\ufeffCONSTANT N = 1\nINIT Init\nNEXT Next\n"),
    "CfgBadKeyword": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nINVARIANTZ Inv\n"),
    "CfgLowercaseKeyword": (BASE, "CONSTANT N = 1\ninit Init\nNEXT Next\n"),
    "CfgMissingName": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT"),
    "CfgTwiceInit": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nINIT Init\n"),
    "CfgTwiceDeadlock": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nCHECK_DEADLOCK FALSE\n"),
    "CfgDeadlockValue": (BASE, "CONSTANT N = 1\nINIT Init\nNEXT Next\nCHECK_DEADLOCK false\n"),
    "CfgNegativeInt": (BASE, "CONSTANT N = -1\nINIT Init\nNEXT Next\n"),
    "CfgBigInt": (BASE, "CONSTANT N = 2147483648\nINIT Init\nNEXT Next\n"),
    "CfgHexInt": (BASE, "CONSTANT N = \\h1F\nINIT Init\nNEXT Next\n"),
    "CfgMissingValue": (BASE, "INIT Init\nNEXT Next\nCONSTANT N ="),
    "CfgMissingEquals": (BASE, "CONSTANT N 3\nINIT Init\nNEXT Next\n"),
    "CfgUnclosedSet": (BASE, "CONSTANT N = {1, 2\nINIT Init\nNEXT Next\n"),
    "CfgUnclosedArgs": (BASE, "CONSTANT N(1 = 2\nINIT Init\nNEXT Next\n"),
    "CfgTuple": (BASE, "CONSTANT N = <<1, 2>>\nINIT Init\nNEXT Next\n"),
    "CfgUnicodeArrow": (BASE.replace("Inv == x < 2", "Inv == x < 2\nOne == 1"),
                        "CONSTANT N \u2190 One\nINIT Init\nNEXT Next\n"),
    "CfgMissingOverride": (BASE, "INIT Init\nNEXT Next\nCONSTANT N <-"),
    "CfgModOverrideBracket": (BASE, "CONSTANT N <- [M X\nINIT Init\nNEXT Next\n"),
}

for name, (tla, cfg) in CASES.items():
    d = os.path.join(out, name)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, name + ".tla"), "w", encoding="utf-8", newline="") as f:
        f.write(tla.replace("{name}", name))
    with open(os.path.join(d, name + ".cfg"), "w", encoding="utf-8", newline="") as f:
        f.write(cfg)
print(len(CASES), "cases")
