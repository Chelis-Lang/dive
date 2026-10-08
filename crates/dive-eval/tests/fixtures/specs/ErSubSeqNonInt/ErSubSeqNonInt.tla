---- MODULE ErSubSeqNonInt ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME SubSeq(<<1, 2>>, "a", 3) = <<>>
====
