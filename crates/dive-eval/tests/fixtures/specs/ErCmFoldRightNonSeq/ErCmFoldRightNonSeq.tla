---- MODULE ErCmFoldRightNonSeq ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(FoldRight(LAMBDA a, b: a, 7, 0))
====
