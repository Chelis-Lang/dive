---- MODULE SeqProductFiniteMalformed ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == IsFiniteSet(Seq({x} \X 0))
====
