---- MODULE ErCmFoldFunctionOnSetNonSet ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Functions
ASSUME PrintT(FoldFunctionOnSet(LAMBDA x, y: x, 0, <<1>>, 3))
====
