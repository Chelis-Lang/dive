---- MODULE ErCmSelectSubSeqDomainThird ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(SelectLastInSubSeq(<<1, 2>>, 1, 3, LAMBDA x: TRUE))
====
