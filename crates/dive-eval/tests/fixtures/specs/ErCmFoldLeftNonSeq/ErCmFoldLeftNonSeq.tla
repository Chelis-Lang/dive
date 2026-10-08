---- MODULE ErCmFoldLeftNonSeq ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(FoldLeft(LAMBDA a, b: a, 0, {1}))
====
