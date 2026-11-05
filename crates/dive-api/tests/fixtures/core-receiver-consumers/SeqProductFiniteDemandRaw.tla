---- MODULE SeqProductFiniteDemandRaw ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == LET raw == {x, TRUE}
       IN /\ IsFiniteSet(Seq((1..0) \X raw))
          /\ Cardinality(raw) = 2
====
