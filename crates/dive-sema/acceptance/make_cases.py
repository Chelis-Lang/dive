#!/usr/bin/env python3
"""Writes the minimal SANY-behaviour cases of `tests/sany-cases/`.

Usage: make_cases.py OUT

Each case is a module `NAME.tla` whose body is given below. Their SANY
verdicts (run_sany.py, then verdicts.py) are recorded in
`tests/sany-cases/verdicts.txt`, which the `sany_cases` test checks dive
against. Cases that show a SANY behaviour worth knowing are described in
`tests/sany-cases/README.md`.
"""
import os
import sys

CASES = {
    # Declarations and definitions.
    "DupVariable": "VARIABLES x, x\nInit == x = 0\n",
    "DupConstant": "CONSTANT c\nCONSTANT c\n",
    "DupConstVar": "CONSTANT c\nVARIABLE c\n",
    "DupTheoremName": "THEOREM T == TRUE\nTHEOREM T == FALSE\n",
    "DupAssumeTheoremName": "ASSUME A == TRUE\nTHEOREM A == TRUE\n",
    "DefThenTheoremSameName": "T == 1\nTHEOREM T == TRUE\n",
    "DupDefinition": "F == 1\nF == 2\n",
    "ShadowBound": "VARIABLE x\nP == \\E x \\in {1} : x = 1\n",
    "ShadowParam": "F(a) == LET a == 1 IN a\n",
    "SiblingBoundsSameName": "P == (\\E y \\in {1} : y = 1) /\\ (\\E y \\in {2} : y = 2)\n",
    "RedefineBuiltin": "BOOLEAN == 1\n",
    "RedefineSetOp": "a \\cup b == a\n",
    "PlusWithoutNaturals": "x == 1 + 2\n",
    "UnicodeNat": "EXTENDS Naturals\nx == 1 \\in ℕ\n",
    # Nested modules and instances.
    "InnerRedefinesOuter": "x == 1\n---- MODULE Inner ----\nx == 2\n====\n",
    "InnerSeesOuter": "x == 1\n---- MODULE Inner ----\ny == x\n====\n",
    "LetInstanceInner": "---- MODULE Inner ----\nz == 1\n====\nF == LET I == INSTANCE Inner IN I!z\n",
    "InstanceBeforeInner": "I == INSTANCE Inner2\n---- MODULE Inner2 ----\nz == 1\n====\nG == I!z\n",
    "InstanceMissingSubst": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nI == INSTANCE P\n",
    "InstanceImplicit": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nN == 3\nI == INSTANCE P\n",
    "InstanceImplicitArity": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nN(a) == 3\nI == INSTANCE P\n",
    "InstanceBadTarget": "---- MODULE P ----\nz == 1\n====\nI == INSTANCE P WITH z <- 2\n",
    "InstanceIncomplete": "---- MODULE P ----\nz == 1\n====\nI == INSTANCE P\nG == I\n",
    "InstanceVarLevel": "---- MODULE P ----\nCONSTANT N\nVARIABLE y\nz == N\n====\n"
                        "VARIABLE v\nI == INSTANCE P WITH N <- v, y <- v\n",
    "InstanceConstLevelOk": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nVARIABLE v\nI == INSTANCE P WITH N <- v\n",
    "ConstantModuleAnyLevel": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nVARIABLE v\nI == INSTANCE P WITH N <- []v\n",
    "ExtendsConflictKinds": "---- MODULE A1 ----\nx == 1\n====\n---- MODULE B1 ----\nCONSTANT x\n====\n"
                            "---- MODULE C1 ----\nEXTENDS A1, B1\n====\n",
    "ExtendsTwoDefs": "---- MODULE A2 ----\nx == 1\n====\n---- MODULE B2 ----\nx == 2\n====\n"
                      "---- MODULE C2 ----\nEXTENDS A2, B2\n====\n",
    "LocalNotExported": "---- MODULE A3 ----\nLOCAL x == 1\n====\n---- MODULE C3 ----\nEXTENDS A3\ny == x\n====\n",
    # Levels.
    "BoxAction": "EXTENDS Naturals\nVARIABLE x\nP == [](x' = x + 1)\n",
    "BoxLetAction": "EXTENDS Naturals\nVARIABLE x\nP == [](LET y == 1 IN x' = x + y)\n",
    "BoxDefAction": "EXTENDS Naturals\nVARIABLE x\nA == x' = x + 1\nP == []A\n",
    "EnabledOfPrimed": "VARIABLE x\nP == ENABLED (x' = 1)\nQ == ENABLED P\n",
    "PrimeOfPrimed": "VARIABLE x\nP == (x')'\n",
    "AssumeVariable": "VARIABLE x\nASSUME x = 1\n",
    "AssumePrimedConst": "CONSTANT c\nASSUME c' = c\n",
    "MixedTemporalAction": "VARIABLE x\nP == []TRUE /\\ x' = x\n",
    "LeadsToAction": "VARIABLE x\nP == (x' = x) ~> TRUE\n",
    "TemporalExistsAction": "VARIABLE x\nP == \\EE y : [](y' = x)\n",
    "UnusedBadLevelDef": "VARIABLE x\nF(a) == a'\nG == F(x')\n",
    "UnusedBadLevelInLet": "VARIABLE x\nF(a) == a'\nG == LET H == F(x') IN TRUE\n",
    # Recursion, functions, higher-order operators.
    "RecursiveUndefined": "RECURSIVE F(_)\n",
    "RecursiveOk": "EXTENDS Naturals\nRECURSIVE F(_)\nF(n) == IF n = 0 THEN 0 ELSE F(n - 1)\n",
    "RecursivePrimed": "VARIABLE x\nRECURSIVE F(_)\nF(n) == IF n = 0 THEN 0 ELSE F(n')\n",
    "FunctionArity": "f[a \\in {1}, b \\in {2}] == a\ng == f[1, 2, 3]\n",
    "FunctionTupleArity": "f[<<a, b>> \\in {<<1,2>>}] == a\ng == f[1, 2]\n",
    "HigherOrderBad": "F(G(_)) == G(1)\nH(a, b) == a\nX == F(H)\n",
    "HigherOrderOk": "F(G(_)) == G(1)\nH(a) == a\nX == F(H)\n",
    "HigherOrderOfHigherOrder": "F(G(_)) == G(1)\nH(K(_)) == 1\nX == F(H)\n",
    "LambdaArity": "F(G(_)) == G(1)\nX == F(LAMBDA a, b : a)\n",
    # Labels, @ and subexpressions.
    "LabelOutside": "ASSUME lab :: TRUE\n",
    "LabelOk": "F == lab :: TRUE\nG == F!lab\n",
    "LabelMissingParam": "F == \\A x \\in {1} : lab :: x = 1\n",
    "AtOutsideExcept": "F == @\n",
    "RecordDupField": "F == [a |-> 1, a |-> 2]\n",
    "SubexprOk": "EXTENDS Naturals\nF == 1 + 2\nG == F!1\n",
    "SubexprBad": "EXTENDS Naturals\nF == 1 + 2\nG == F!3\n",
    "CaseOther": "F == CASE TRUE -> 1 [] OTHER -> 2\nG == F!1!2\n",
    # Proofs.
    "DupStepNames": "THEOREM TRUE\n<1>1. TRUE\n  OBVIOUS\n<1>1. TRUE\n  OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "ImplicitNamedStep": "THEOREM TRUE\n<*>1. TRUE\n  OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "UseUndefined": "USE Undefined\n",
    "HideExpr": "EXTENDS Naturals\nHIDE 1 = 1\n",
    "HideDefOk": "F == 1\nHIDE DEF F\n",
    "BoxAssumeInAssume": "THEOREM ASSUME TRUE PROVE TRUE\n<1>1. ASSUME []ASSUME TRUE []PROVE TRUE PROVE TRUE\n"
                         "  OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "NewTemporalSet": "THEOREM ASSUME NEW x \\in []TRUE PROVE TRUE\n",
    "DefStepUse": "THEOREM TRUE\n<1> DEFINE F == 1\n<1>1. QED\n  BY DEF F\n",
    "StepAsExpression": "THEOREM TRUE\n<1>1. TAKE x\n<1>2. <1>1\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "PickScope": "THEOREM \\E x : TRUE\n<1>1. PICK y : y = 1\n  <2>1. y = 1\n    OBVIOUS\n  <2>2. QED\n"
                 "    OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "PickAfter": "THEOREM \\E x : TRUE\n<1>1. PICK y : y = y\n  OBVIOUS\n<1>2. y = y\n  OBVIOUS\n<1>3. QED\n"
                 "  OBVIOUS\n",
    # PlusCal translation check (pcal.Validator during SANY's parse phase).
    "PcalUnterminatedString": "(* --algorithm a\nbegin x := \"abc;\nend algorithm *)\n"
                              "\\* BEGIN TRANSLATION\n\\* END TRANSLATION\n",
    "PcalStringOk": "(* --algorithm a\nbegin x := \"abc\";\nend algorithm *)\n"
                    "\\* BEGIN TRANSLATION\n\\* END TRANSLATION\n",
    "PcalEndWithoutBegin": "(* --algorithm a\nbegin skip;\nend algorithm *)\n\\* END TRANSLATION\n",
    "PcalNoTranslation": "(* --algorithm a\nbegin x := \"abc;\nend algorithm *)\n",
    "TopImplicitStepLevel": "THEOREM TRUE\n<*>a. TRUE\n  OBVIOUS\n<*>. QED\n  BY <0>a\n",
}


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, body in CASES.items():
        with open(os.path.join(out, name + ".tla"), "w", encoding="utf-8") as fh:
            fh.write("---- MODULE %s ----\n%s====\n" % (name, body))
    print(len(CASES))


if __name__ == "__main__":
    main()
