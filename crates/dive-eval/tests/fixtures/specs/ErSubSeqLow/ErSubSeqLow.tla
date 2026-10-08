---- MODULE ErSubSeqLow ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SubSeq(<<1, 2>>, 0, 1) = <<>>
====
