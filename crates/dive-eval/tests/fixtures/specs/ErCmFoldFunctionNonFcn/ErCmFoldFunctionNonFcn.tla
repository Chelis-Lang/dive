---- MODULE ErCmFoldFunctionNonFcn ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Functions
ASSUME PrintT(FoldFunction(LAMBDA x, y: x, 0, 3))
====
