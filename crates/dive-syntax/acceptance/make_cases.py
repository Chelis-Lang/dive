#!/usr/bin/env python3
"""Write the minimal SANY-behaviour specs: make_cases.py OUT_DIR

Each case is a module whose name equals its file name.
"""
import os
import sys

CASES = [
    # --- precedence and associativity ---
    ("PrecTimesNeg", "x == 1 * -2"),
    ("PrecPowNeg", "x == 2 ^ -1"),
    ("PrecDivNeg", "x == 4 / -2"),
    ("PrecPlusNeg", "x == 1 + -2"),
    ("PrecMinusNeg", "x == 1 - -2"),
    ("PrecModNeg", "x == 1 % -2"),
    ("PrecNegPow", "x == -2 ^ 2"),
    ("PrecNegNeg", "x == - - 1"),
    ("PrecNotNot", "x == ~ ~ TRUE"),
    ("PrecEnabledPrime", "VARIABLE y\nx == ENABLED y'"),
    ("PrecUnchangedPrime", "VARIABLE y\nx == UNCHANGED y'"),
    ("PrecBoxPrime", "VARIABLE y\nx == []y'"),
    ("PrecBoxPrimeParen", "VARIABLE y\nx == [](y')"),
    ("PrecEqChain", "x == 1 = 2 = 3"),
    ("PrecLtEq", "x == 1 < 2 = TRUE"),
    ("PrecCdotEq", "x == 1 \\cdot 2 = 3"),
    ("PrecAndOr", "x == TRUE /\\ FALSE \\/ TRUE"),
    ("PrecAndAnd", "x == TRUE /\\ FALSE \\land TRUE"),
    ("PrecImpliesChain", "x == TRUE => FALSE => TRUE"),
    ("PrecTimesDiv", "x == 1 * 2 / 3"),
    ("PrecDivDiv", "x == 1 / 2 / 3"),
    ("PrecCupCap", "x == {} \\cup {} \\cap {}"),
    ("PrecSetminusChain", "x == {} \\ {} \\ {}"),
    ("PrecInChain", "x == 1 \\in {} \\in {}"),
    ("PrecDotDotChain", "x == 1 .. 2 .. 3"),
    ("PrecPowChain", "x == 2 ^ 3 ^ 4"),
    ("PrecCircChain", "x == 1 \\o 2 \\o 3"),
    ("PrecSubsetSubset", "x == SUBSET SUBSET {}"),
    ("PrecUnionCup", "x == UNION {} \\cup {}"),
    ("PrecSubsetTimes", "x == SUBSET {} \\X {}"),
    ("PrecBoxImplies", "x == []TRUE => FALSE"),
    ("PrecNotEq", "x == ~ 1 = 2"),
    ("PrecPlusNot", "x == 1 + ~ 2 = 3"),
    ("PrecTimesChain", "x == {} \\X {} \\times {} \\X {}"),
    ("PrecPostfixAfterInfix", "x == 1 + '"),
    ("PrecDomainFcn", "x == DOMAIN f[1]"),
    ("PrecLeadsTo", "x == TRUE ~> FALSE ~> TRUE"),
    # --- nonfix and operator arguments ---
    ("NonfixMinus", "x == -(1, 2)"),
    ("NonfixPlus", "x == +(1, 2)"),
    ("NonfixMinusOne", "x == -(1)"),
    ("OpArgTimes", "F(G(_, _)) == 1\nx == F(\\X)"),
    ("OpArgTimesUnicode", "F(G(_, _)) == 1\nx == F(×)"),
    ("OpArgPlus", "F(G(_, _)) == 1\nx == F(+)"),
    ("OpArgLambda", "F(G(_)) == G(1)\nx == F(LAMBDA y : y)"),
    ("OpArgTimesSpaced", "F(G(_, _)) == 1\nx == F( \\X )"),
    ("OpArgTimesWordSpaced", "F(G(_, _)) == 1\nx == F( \\times )"),
    ("OpArgPlusSpaced", "F(G(_, _)) == 1\nx == F( + )"),
    ("OpArgPrefixSpaced", "F(G(_)) == 1\nx == F( -. )"),
    ("OpArgPostfixSpaced", "F(G(_)) == 1\nx == F( ' , 1)"),
    ("OplusSymbol", "x == 1 (+) 2"),
    # --- sets, functions, records ---
    ("SetMapIn", "x == {f(a) \\in b : a \\in {}}"),
    ("SetMapInParen", "x == {(f(a) \\in b) : a \\in {}}"),
    ("SetMapInUnicode", "x == {f(a) ∈ b : a ∈ {}}"),
    ("SetFilterIn", "x == {a \\in {} : a \\in {}}"),
    ("SetEnumCommaIn", "x == {a, b \\in {}}"),
    ("SetTupleIn", "x == {<<1, 2>> \\in {}}"),
    ("SetIdentTupleIn", "x == {<<a, b>> \\in {}}"),
    ("FcnConstNoMapsTo", "x == [y \\in {}]"),
    ("BareBrackets", "x == [1, 2]"),
    ("RecordKeyword", "x == [ELSE |-> 1]"),
    ("RecordKeywordSecond", "x == [a |-> 1, ELSE |-> 2]"),
    ("RecordSetKeyword", "x == [ELSE : {}]"),
    ("FieldKeyword", "x == r.ELSE"),
    ("FieldChain", "x == a.b.c"),
    ("PrimeThenField", "VARIABLE a\nx == a'.b"),
    ("FcnPrime", "VARIABLE f\nx == f[1]' = f'[1]"),
    ("ExceptAt", "x == [f EXCEPT ![1] = @ + 1, !.a.b = 2]"),
    ("ExceptDotAt", "x == [f EXCEPT !.@ = 1]"),
    ("ExceptKeywordField", "x == [f EXCEPT !.THEN = 1]"),
    ("ChooseTuple", "x == CHOOSE <<a, b>> \\in {} : TRUE"),
    ("QuantTuple", "x == \\E <<a, b>> \\in {} : TRUE"),
    ("QuantEmptyTuple", "x == \\A <<>> \\in {} : TRUE"),
    ("QuantMulti", "x == \\A a \\in {}, b, c \\in {} : TRUE"),
    ("TemporalQuant", "x == \\EE y : TRUE"),
    # --- actions and fairness ---
    ("ActionTuple", "VARIABLE a, b\nx == [a' = a]_<<a, b>>"),
    ("ActionArgs", "VARIABLE a\nx == [a' = a]_f(a)"),
    ("AngleAction", "VARIABLE a\nx == <<a' = a>>_a"),
    ("FairnessOk", "VARIABLE v\nx == WF_v(v' = v)"),
    ("FairnessNoArgs", "VARIABLE v\nx == WF_v"),
    ("FairnessTwoArgs", "VARIABLE v\nx == WF_v(v, v)"),
    ("FairnessTuple", "VARIABLE v\nx == SF_<<v>>(v' = v)"),
    ("FairnessBracket", "VARIABLE v\nx == WF_[v](v' = v)"),
    ("FairnessDoubleArgs", "VARIABLE v\nf(a) == a\nx == WF_f(v)(v' = v)"),
    # --- labels ---
    ("LabelOk", "x == lab :: 1 + 2"),
    ("LabelChangesParse", "x == 2 * lab :: 1 + 2"),
    ("LabelNotLabel", "x == f[1] :: 2"),
    ("LabelArgs", "x == \\A a : lab(a) :: a"),
    ("LabelQualified", "x == M!lab :: 1"),
    # --- junction lists ---
    ("JunctionAligned", "x == /\\ TRUE\n     /\\ FALSE"),
    ("JunctionRightBullet", "x == /\\ TRUE\n      /\\ FALSE"),
    ("JunctionLeftBullet", "x == /\\ TRUE\n    /\\ FALSE"),
    ("JunctionThenDisj", "x == /\\ TRUE\n     \\/ FALSE"),
    ("JunctionElseLeft", "x == /\\ IF TRUE\n        THEN 1\n    ELSE 2"),
    ("JunctionElseAtBullet", "x == /\\ IF TRUE\n        THEN 1\n     ELSE 2"),
    ("JunctionParenLeft", "x == /\\ (TRUE\n)"),
    ("JunctionOuterParen", "x == (/\\ TRUE\n)"),
    ("JunctionCaseSep", "x == /\\ CASE TRUE -> 1\n     [] FALSE -> 2"),
    ("JunctionCaseSepRight", "x == /\\ CASE TRUE -> 1\n        [] FALSE -> 2"),
    ("JunctionArgLeft", "f(a, b) == a\nx == /\\ f(1,\n2)"),
    ("JunctionNested", "x == /\\ \\/ TRUE\n        \\/ FALSE\n     /\\ TRUE"),
    ("JunctionUnicode", "x == ∧ TRUE\n     ∧ FALSE"),
    ("JunctionMixedSpelling", "x == /\\ TRUE\n     ∧ FALSE"),
    ("JunctionTabAtBullet", "x ==\n        /\\ IF TRUE\n\tTHEN 1 ELSE 2"),
    ("JunctionTabRight", "x ==\n        /\\ IF TRUE\n\t THEN 1 ELSE 2"),
    ("JunctionAstralAtBullet", "x == <<\"𝔸\", /\\ IF TRUE\n             THEN 1 ELSE 2>>"),
    ("JunctionAstralRight", "x == <<\"𝔸\", /\\ IF TRUE\n              THEN 1 ELSE 2>>"),
    # --- definitions and belchDEF ---
    ("DefInfix", "a + b == 1"),
    ("DefPostfix", "a ^+ == 1"),
    ("DefPrefixNegDot", "-. a == 1"),
    ("DefPrefixMinus", "- a == 1"),
    ("DefPrefixNot", "~ a == 1"),
    ("DefLocal", "LOCAL x == 1"),
    ("DefLocalInLet", "x == LET LOCAL y == 1 IN y"),
    ("DefParams", "F(_ + _, -. _, G(_), a) == 1"),
    ("DefFunction", "f[a \\in {}, <<b, c>> \\in {}] == 1"),
    ("DefLiteralDefbreak", "-|- x == 1"),
    ("DefAfterTheorem", "THEOREM TRUE\nx == 1"),
    ("DefRecursive", "RECURSIVE F(_)\nF(n) == IF n = 0 THEN 0 ELSE F(n - 1)"),
    ("DefRecursiveInLet", "x == LET RECURSIVE G(_)\n         G(n) == n\n     IN G(1)"),
    ("DeclConstants", "CONSTANT C(_, _), _ ** _, ~ _, -. _, _ ^+"),
    ("AssumeNamed", "ASSUME A == TRUE"),
    ("AxiomNamed", "AXIOM A == TRUE"),
    ("TheoremNamed", "THEOREM T == TRUE"),
    ("LemmaNamed", "LEMMA T == TRUE"),
    ("UseOnlyTop", "x == 1\nUSE ONLY x"),
    ("UseTop", "x == 1\nUSE x DEF x"),
    ("HideTop", "x == 1\nHIDE DEF x"),
    ("InstanceKeywordName", "INSTANCE ACTION"),
    ("LocalInstanceKeywordName", "LOCAL INSTANCE ACTION"),
    ("InstanceWithOps", "I == INSTANCE Naturals WITH a <- +, b <- LAMBDA x : x"),
    # --- proofs ---
    ("ProofSteps", "THEOREM TRUE\n<1>1. TRUE\n  OBVIOUS\n<1>2. QED\n  BY <1>1"),
    ("ProofImplicit", "THEOREM TRUE\n<+> TRUE\n<*> QED"),
    ("ProofBadLevel", "THEOREM TRUE\n<1>1. TRUE\n<2>2. QED"),
    ("ProofDeeper", "THEOREM TRUE\n<1>1. TRUE\n  <2>1. TRUE\n  <2>2. QED\n<1>2. QED"),
    ("ProofHaveWithProof", "THEOREM TRUE\n<1>1. HAVE TRUE\n  OBVIOUS\n<1>2. QED"),
    ("ProofCaseStep", "THEOREM TRUE\n<1>1. CASE TRUE\n  OBVIOUS\n<1>2. QED"),
    ("ProofCaseArrow", "THEOREM TRUE\n<1>1. CASE TRUE -> 1\n<1>2. QED"),
    ("ProofStepOutside", "x == <1>1"),
    ("ProofPlusInExpr", "THEOREM TRUE\n<1>1. TRUE\n<1>2. QED BY <+>1"),
    ("ProofAssumeProve", "THEOREM ASSUME NEW c \\in {}, NEW VARIABLE v, NEW ACTION A, TRUE\n        PROVE TRUE\nOBVIOUS"),
    ("ProofNewArgsIn", "THEOREM ASSUME NEW F(_) \\in {} PROVE TRUE"),
    ("ProofSufficesPick", "THEOREM TRUE\n<1>1. SUFFICES ASSUME TRUE PROVE TRUE\n  OBVIOUS\n<1>2. PICK y \\in {} : TRUE\n<1>3. TAKE a, b\n<1>4. WITNESS 1, 2\n<1> DEFINE z == 1\n<1>5. QED"),
    ("ProofOnlyBy", "THEOREM TRUE\nPROOF BY ONLY DEF x"),
    ("ProofOmitted", "THEOREM TRUE\nPROOF OMITTED"),
    ("ProofLabelAssume", "THEOREM ASSUME lab :: ASSUME TRUE PROVE TRUE PROVE TRUE"),
    ("ProofBoxAssume", "THEOREM []ASSUME TRUE []PROVE TRUE"),
    ("ProofUseInStep", "THEOREM TRUE\n<1> USE DEF x\n<1> HIDE DEF x\n<1> QED"),
    # --- subexpressions ---
    ("SubexprSelectors", "x == Op!1!<<!>>!:!@"),
    ("SubexprDecimal", "x == Op!1.2"),
    ("SubexprArgs", "x == Op(1)!b!c(2, 3)"),
    ("SubexprOps", "x == M!+(1, 2)"),
    # --- lexer ---
    ("NumberForms", "x == \\b101 + \\o17 + \\hFF + \\B1 + \\O7 + \\HaB + 1.5"),
    ("StringEscapes", "x == \"a\\\"b\\\\c\\n\\t\\r\\f\""),
    ("StringBadEscape", "x == \"\\q\""),
    ("StringNewline", "x == \"abc\ndef\""),
    ("LexSemicolon", "x == 1 ; 2"),
    ("LexLessDigit", "x == 3 <4"),
    ("LexStepInExpr", "x == 3 <4> 5"),
    ("NestedComments", "(* a (* b *) c *)\nx == 1 \\* comment (* not open"),
    ("UnterminatedComment", "(* a (* b *)\nx == 1"),
    ("UnicodeDefEq", "x ≜ ⟨1, 2⟩"),
    ("UnicodeSets", "x == ℕ \\cup ℤ \\cup ℝ"),
    ("BadJunctionLexeme", "x == a./\\ b"),
    ("IdentifierForms", "x == _a + a_ + 1a + WFx + S_F + @"),
    ("WFIdentifier", "WF == 1\nx == WF + 1"),
    ("KeywordPrefixIdent", "THEOREMS == 1"),
    ("DoubleEquals", "x === 1"),
]

def nested_proof(levels):
    """A theorem whose proof nests `levels` deep (SANY allows 100)."""
    lines = ["THEOREM TRUE"]
    for k in range(1, levels + 1):
        lines.append("  " * (k - 1) + "<%d>1. TRUE" % k)
    for k in range(levels, 0, -1):
        lines.append("  " * (k - 1) + "<%d>2. QED" % k)
    return "\n".join(lines)


CASES += [
    ("ProofDepth100", nested_proof(100)),
    ("ProofDepth101", nested_proof(101)),
]


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, body in CASES:
        src = "---- MODULE %s ----\n%s\n====\n" % (name, body)
        with open(os.path.join(out, name + ".tla"), "w", encoding="utf-8") as fh:
            fh.write(src)
    extra = {
        "PreludeAndTrailer": "Junk before\n---- MODULE PreludeAndTrailer ----\nx == 1\n==== trailing junk \"unterminated\n(* open",
        "PragmaPrelude": "---> 12 abc\n---- MODULE PragmaPrelude ----\nx == 1\n====\n",
        "NestedModule": "---- MODULE NestedModule ----\n---- MODULE Inner ----\ny == 1\n====\nx == Inner!y\n====\n",
        "NoModule": "just text\n",
        "PlusCalComment": "---- MODULE PlusCalComment ----\n(* --algorithm a\nvariables x = 0;\nbegin\n  l: x := (x + 1) * 2;\nend algorithm *)\nVARIABLE x\n====\n",
        "TabHeader": "----\tMODULE TabHeader ----\n====\n",
        "MissingEnd": "---- MODULE MissingEnd ----\nx == 1\n",
    }
    for name, src in extra.items():
        with open(os.path.join(out, name + ".tla"), "w", encoding="utf-8") as fh:
            fh.write(src)
    print(len(CASES) + len(extra))


if __name__ == "__main__":
    main()
