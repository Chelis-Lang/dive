---- MODULE EmptyFunctionAndSequence ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == /\ IsFiniteSet([{} -> x] \X {x})
       /\ IsFiniteSet([Nat -> {}] \X {x})
       /\ IsFiniteSet(Seq({} \X x))
       /\ IsFiniteSet({x} \X Seq({} \X x))
====
