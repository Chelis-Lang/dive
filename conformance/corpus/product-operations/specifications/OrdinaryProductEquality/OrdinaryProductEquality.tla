---- MODULE OrdinaryProductEquality ----
EXTENDS Integers, FiniteSets, Sequences, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == ((x..x) \X (x..x)) = {<<x,x>>}
====
