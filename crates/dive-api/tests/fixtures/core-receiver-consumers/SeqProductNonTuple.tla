---- MODULE SeqProductNonTuple ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == <<x>> \in Seq((1..0) \X 0)
====
