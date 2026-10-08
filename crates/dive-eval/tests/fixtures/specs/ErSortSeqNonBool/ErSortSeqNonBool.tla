---- MODULE ErSortSeqNonBool ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SortSeq(<<1, 2>>, LAMBDA a, b : 1) = <<>>
====
