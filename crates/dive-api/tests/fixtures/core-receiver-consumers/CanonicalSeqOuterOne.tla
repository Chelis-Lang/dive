---- MODULE CanonicalSeqOuterOne ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == Cardinality({Seq(1..(1+x)), Seq({1+x})}) = 1
====
