---- MODULE OrderedRecordEmpty ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == /\ IsFiniteSet([a : {}, b : x] \X {x})
       /\ IsFiniteSet([b : x, a : {}] \X {x})
       /\ IsFiniteSet({x} \X [b : {}, a : Nat])
       /\ IsFiniteSet([c : x, b : {}, a : Int] \X {x})
====
