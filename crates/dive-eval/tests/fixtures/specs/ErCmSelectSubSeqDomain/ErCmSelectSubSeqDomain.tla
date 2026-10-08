---- MODULE ErCmSelectSubSeqDomain ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(SelectInSubSeq(<<1, 2>>, 0, 1, LAMBDA x: TRUE))
====
