---- MODULE ErCmFoldSetNonSet ----
EXTENDS Integers, Sequences, FiniteSets, TLC, FiniteSetsExt
ASSUME PrintT(FoldSet(LAMBDA x, y: x, 0, <<1>>))
====
