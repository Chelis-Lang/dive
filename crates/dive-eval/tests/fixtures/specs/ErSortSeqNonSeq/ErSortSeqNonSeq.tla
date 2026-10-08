---- MODULE ErSortSeqNonSeq ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SortSeq(1, LAMBDA a, b : a < b) = <<>>
====
