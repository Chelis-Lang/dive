---- MODULE EmptyRecordBeforeScalar ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == IsFiniteSet([a : {}, b : x] \X {x})
====
