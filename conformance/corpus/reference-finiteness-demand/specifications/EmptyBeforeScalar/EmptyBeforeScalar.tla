---- MODULE EmptyBeforeScalar ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == IsFiniteSet({} \X x)
====
