---- MODULE ErCmSelectSubSeqNonInt ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(SelectInSubSeq(<<1, 2>>, "a", 1, LAMBDA x: TRUE))
====
