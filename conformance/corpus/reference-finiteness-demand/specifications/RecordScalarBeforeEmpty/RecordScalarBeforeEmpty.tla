---- MODULE RecordScalarBeforeEmpty ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET r == [a |-> x, b |-> x]
       IN /\ r.a = x
          /\ IsFiniteSet([b : {}, a : x] \X {x})
====
