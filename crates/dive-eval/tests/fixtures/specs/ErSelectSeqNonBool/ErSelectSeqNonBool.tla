---- MODULE ErSelectSeqNonBool ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SelectSeq(<<1>>, LAMBDA x : 1) = <<>>
====
