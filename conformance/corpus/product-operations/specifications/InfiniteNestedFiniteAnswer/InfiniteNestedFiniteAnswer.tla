---- MODULE InfiniteNestedFiniteAnswer ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == /\ ~IsFiniteSet((Int \X {x}) \X {x})
       /\ ~IsFiniteSet({x} \X ({x} \X Nat))
       /\ IsFiniteSet((Nat \X {}) \X {x})
       /\ IsFiniteSet({} \X (Int \X {x}))
====
