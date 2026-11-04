---- MODULE InfiniteRightFiniteAnswer ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == ~IsFiniteSet({x} \X Nat)
====
