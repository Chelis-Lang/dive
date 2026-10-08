---- MODULE ErSubSeqHigh ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SubSeq(<<1, 2>>, 1, 3) = <<>>
====
