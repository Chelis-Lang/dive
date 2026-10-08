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
    # Subexpressions and labels (second batch).
    "SelLabelArgs": "F == \\A x \\in {1} : lab(x) :: x = 1\nG == F!lab(2)\n",
    "SelLabelArgsWrong": "F == \\A x \\in {1} : lab(x) :: x = 1\nG == F!lab\n",
    "SelLabelParamRepeat": "F == \\A x \\in {1} : lab(x, x) :: x = 1\n",
    "SelColonEnd": "F(a) == a = 1\nG == F!:\n",
    "SelColonBad": "EXTENDS Naturals\nF == 1 + 2\nG == F!1!:\n",
    "SelLeftRight": "EXTENDS Naturals\nF == 1 + 2\nG == F!<< + F!>>\n",
    "SelQuantifierBody": "F == \\A x \\in {1, 2} : x = 1\nG == F!(1)\n",
    "SelQuantifierBodyArity": "F == \\A x \\in {1, 2} : x = 1\nG == F!(1, 2)\n",
    "SelQuantifierBound": "F == \\A x \\in {1, 2} : x = 1\nG == F!1\n",
    "SelLetName": "F == LET A == 1 IN A\nG == F!A\n",
    "SelLetNotLet": "F == 1\nG == F!A\n",
    "SelCaseOther": "F == CASE TRUE -> 1 [] OTHER -> 2\nG == F!2!1\n",
    "SelExceptBody": "F == [f EXCEPT ![1] = 2]\nf == [x \\in {1} |-> x]\nG == 1\n",
    "SelExceptSub": "f == [x \\in {1} |-> x]\nF == [f EXCEPT ![1] = 2]\nG == F!2\n",
    "SelOpArg": "F(a) == a\nG == F!1\n",
    "SelInsideOwnDef": "F == 1 /\\ F!1\n",
    "SelThmAssumeProve": "THEOREM T == ASSUME NEW x, x = 1 PROVE x = 1\nG == T!2\n",
    "SelThmAssumeProveAll": "THEOREM T == ASSUME TRUE PROVE TRUE\nG == T\n",
    "SelInstanceSub": "---- MODULE P ----\nCONSTANT N\nz == N + N\n====\nEXTENDS Naturals\nI == INSTANCE P WITH N <- 1\nG == I!z!1\n",
    "SelParamInstance": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nI(n) == INSTANCE P WITH N <- n\nG == I(1)!z\n",
    "SelParamInstanceArity": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nI(n) == INSTANCE P WITH N <- n\nG == I!z\n",
    "LabelTwiceSameName": "F == /\\ lab :: TRUE\n     /\\ lab :: FALSE\n",
    "LabelInExcept": "f == [x \\in {1} |-> x]\nF == [f EXCEPT ![1] = lab :: 2]\n",
    "LabelOnAssumeProve": "THEOREM ASSUME TRUE, lab :: ASSUME TRUE PROVE TRUE PROVE TRUE\n",
    "LabelNestedOnAssumeProve": "THEOREM ASSUME TRUE, l1 :: l2 :: ASSUME TRUE PROVE TRUE PROVE TRUE\n",
    # Instances, recursion and higher order (second batch).
    "InstanceOperatorParam": "---- MODULE P ----\nCONSTANT Op(_)\nz == Op(1)\n====\nF(a) == a\nI == INSTANCE P WITH Op <- F\n",
    "InstanceOperatorParamExpr": "---- MODULE P ----\nCONSTANT Op(_)\nz == Op(1)\n====\nI == INSTANCE P WITH Op <- 1\n",
    "InstanceOperatorLambda": "---- MODULE P ----\nCONSTANT Op(_)\nz == Op(1)\n====\nI == INSTANCE P WITH Op <- LAMBDA a : a\n",
    "InstanceNonLeibniz": "---- MODULE P ----\nCONSTANT Op(_)\nVARIABLE v\nz == Op(v)\n====\nVARIABLE v\nF(a) == ENABLED (a' = 1)\nI == INSTANCE P WITH Op <- F\n",
    "InstanceDupSubst": "---- MODULE P ----\nCONSTANT N\nz == N\n====\nI == INSTANCE P WITH N <- 1, N <- 2\n",
    "InstanceLocalHides": "---- MODULE P ----\nz == 1\n====\nLOCAL INSTANCE P\n---- MODULE Q ----\nEXTENDS LocalHidesOuterPlaceholder\n====\n",
    "UnnamedInstanceConflict": "---- MODULE P ----\nz == 1\n====\nz == 2\nINSTANCE P\n",
    "UnnamedInstanceSameDef": "EXTENDS Naturals\nINSTANCE Naturals\n",
    "RecursiveInLet": "EXTENDS Naturals\nF(n) == LET RECURSIVE G(_)\n            G(m) == IF m = 0 THEN 0 ELSE G(m - 1)\n        IN G(n)\n",
    "RecursiveWrongLetLevel": "EXTENDS Naturals\nRECURSIVE G(_)\nF == LET G(m) == m IN 1\nG(m) == m\n",
    "RecursiveArity": "RECURSIVE G(_)\nG(a, b) == a\n",
    "RecursiveFunction": "EXTENDS Naturals\nRECURSIVE f\nf[n \\in Nat] == IF n = 0 THEN 0 ELSE f[n - 1]\n",
    "RecursiveSectionVariable": "RECURSIVE G(_)\nVARIABLE x\nG(a) == a\n",
    "RecursiveMutual": "EXTENDS Naturals\nRECURSIVE A(_), B(_)\nA(n) == IF n = 0 THEN 0 ELSE B(n - 1)\nB(n) == IF n = 0 THEN 1 ELSE A(n - 1)\n",
    "RecursiveLevel": "VARIABLE x\nRECURSIVE G(_)\nG(n) == IF n = 0 THEN x ELSE G(0)\nH == G(1)'\n",
    "FcnRecursiveSelfArg": "f[n \\in {1}] == f\n",
    "HigherOrderLevel": "VARIABLE x\nF(G(_)) == G(x)\nH(a) == a'\nK == F(H)\n",
    "HigherOrderPrimedArg": "VARIABLE x\nF(G(_), b) == G(b)\nH(a) == a'\nK == F(H, x')\n",
    "OpArgBuiltin": "F(G(_,_)) == G(1, 2)\nK == F(\\cup)\n",
    "OpArgBuiltinArity": "F(G(_)) == G(1)\nK == F(\\cup)\n",
    "OpArgToNonOpParam": "F(a) == a\nK == F(\\cup)\n",
    "ExprToOpParam": "F(G(_)) == G(1)\nK == F(1 + 1)\n",
    # Levels (second batch).
    "EnabledTemporal": "VARIABLE x\nP == ENABLED []TRUE\n",
    "UnchangedPrimed": "VARIABLE x\nP == UNCHANGED x'\n",
    "PrimeTemporal": "P == ([]TRUE)'\n",
    "SquareActTemporal": "VARIABLE x\nP == [[]TRUE]_x\n",
    "WFTemporalSubscript": "VARIABLE x\nP == WF_([]x)(x' = x)\n",
    "ActionInChoose": "VARIABLE x\nP == CHOOSE y \\in {x'} : TRUE\n",
    "TemporalInSet": "P == {[]TRUE}\n",
    "TemporalInIf": "VARIABLE x\nP == IF []TRUE THEN x' = x ELSE TRUE\n",
    "TemporalBoundExists": "VARIABLE x\nP == \\E y \\in {x'} : []TRUE\n",
    "TemporalBoundRange": "P == \\E y \\in {[]TRUE} : TRUE\n",
    "ConstantModuleTheoremLevel": "VARIABLE x\nTHEOREM []TRUE\n",
    "AssumeTemporal": "ASSUME []TRUE\n",
    "LeadsToStatePredicates": "VARIABLE x\nP == (x = 1) ~> (x = 2)\n",
    "NotOfAction": "VARIABLE x\nP == ~(x' = x)\n",
    "BoxBoxAction": "VARIABLE x\nP == [][][x' = x]_x\n",
    "EventuallyAngle": "VARIABLE x\nP == <><<x' = x>>_x\n",
    "EventuallyAction": "VARIABLE x\nP == <>(x' = x)\n",
    # Proofs (second batch).
    "ProofAssumeScope": "THEOREM ASSUME NEW x PROVE x = x\n<1>1. x = x\n  OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "ProofSufficesScope": "THEOREM TRUE\n<1>1. SUFFICES ASSUME NEW y PROVE y = y\n  OBVIOUS\n<1>2. y = y\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "ProofAssumeNotAfter": "THEOREM TRUE\n<1>1. ASSUME NEW y PROVE y = y\n  OBVIOUS\n<1>2. y = y\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "ProofStepRefLater": "THEOREM TRUE\n<1>1. TRUE\n  BY <1>2\n<1>2. QED\n  OBVIOUS\n",
    "ProofStepRefEarlier": "THEOREM TRUE\n<1>1. TRUE\n  OBVIOUS\n<1>2. QED\n  BY <1>1\n",
    "ProofStepScopeEnds": "THEOREM TRUE\n<1>1. TRUE\n  <2>1. TRUE\n    OBVIOUS\n  <2>2. QED\n    OBVIOUS\n<1>2. QED\n  BY <2>1\n",
    "ProofDefineUse": "THEOREM TRUE\n<1>1. DEFINE F == 1\n<1>2. F = 1\n  BY DEF F\n<1>3. QED\n  OBVIOUS\n",
    "ProofDefStepAsDef": "THEOREM TRUE\n<1>1. DEFINE F == 1\n<1>2. QED\n  BY DEF <1>1\n",
    "ProofDefOfTheoremStep": "THEOREM TRUE\n<1>1. TRUE\n  OBVIOUS\n<1>2. QED\n  BY DEF <1>1\n",
    "ProofUseModule": "EXTENDS Naturals\nUSE MODULE Naturals\n",
    "ProofUseModuleSelf": "F == 1\nUSE MODULE ProofUseModuleSelf\n",
    "ProofUseModuleUnknown": "USE MODULE Sequences\n",
    "ProofHideTheorem": "THEOREM T == TRUE\nHIDE T\n",
    "ProofTakeWitness": "THEOREM \\A x \\in {1} : \\E y \\in {1} : y = x\n<1>1. TAKE x \\in {1}\n<1>2. WITNESS 1 \\in {1}\n<1>3. QED\n  OBVIOUS\n",
    "ProofCaseHave": "THEOREM TRUE => TRUE\n<1>1. HAVE TRUE\n<1>2. CASE TRUE\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "ProofAtStep": "EXTENDS Naturals\nTHEOREM 1 = 1\n<1>1. 1 = 1 + 0\n  OBVIOUS\n<1>2. @ = 1\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "ProofTemporalTake": "VARIABLE x\nTHEOREM \\A y : []TRUE\n<1>1. TAKE y\n<1>2. QED\n  OBVIOUS\n",
    "ProofTemporalHaveState": "VARIABLE x\nTHEOREM x = 1 => []TRUE\n<1>1. HAVE x = 1\n<1>2. QED\n  OBVIOUS\n",
    "ProofStepNameAsExpr": "THEOREM TRUE\n<1>1. TRUE\n  OBVIOUS\n<1>2. <1>1 /\\ TRUE\n  OBVIOUS\n<1>3. QED\n  OBVIOUS\n",
    "ProofQedAsExpr": "THEOREM TRUE\n<1>1. QED\n  <2>1. <1>1\n    OBVIOUS\n  <2>2. QED\n    OBVIOUS\n",
    "ProofNewInNested": "THEOREM ASSUME ASSUME NEW x PROVE lab :: x = x PROVE TRUE\n",
    "ProofBoxAssume": "THEOREM []ASSUME TRUE []PROVE TRUE\n",
    "ProofBoxAssumeMismatch": "THEOREM []ASSUME TRUE PROVE TRUE\n",
    "ProofPickTemporalBound": "VARIABLE x\nTHEOREM []TRUE\n<1>1. PICK y \\in {x} : []TRUE\n  OBVIOUS\n<1>2. QED\n  OBVIOUS\n",
    "ProofInstanceStep": "---- MODULE P ----\nz == 1\n====\nTHEOREM TRUE\n<1>1. INSTANCE P\n<1>2. QED\n  BY DEF z\n",
    "ProofUseOnlyDef": "F == 1\nTHEOREM F = 1\n  BY ONLY DEF F\n",
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
