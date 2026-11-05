---- MODULE SeqProduct ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 1
Next == UNCHANGED x
Inv == /\ <<<<x, 0>>, <<x+1, 0>>>> \in Seq({x, x+1} \X {0})
       /\ <<>> \in Seq((1..0) \X 0)
====
