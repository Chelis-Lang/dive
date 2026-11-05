---- MODULE CanonicalSeqUnequal ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == Seq(1..(1+x)) # Seq({1+x})
====
