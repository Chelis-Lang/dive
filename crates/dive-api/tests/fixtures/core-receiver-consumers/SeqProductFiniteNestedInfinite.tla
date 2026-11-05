---- MODULE SeqProductFiniteNestedInfinite ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == ~IsFiniteSet(Seq(Seq((1..0) \X {x})))
====
