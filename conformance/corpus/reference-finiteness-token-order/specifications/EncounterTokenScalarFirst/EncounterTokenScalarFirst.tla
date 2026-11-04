---- MODULE EncounterTokenScalarFirst ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET seed == [zephyrAxis |-> x, amberAxis |-> x] IN /\ seed.zephyrAxis = x /\ IsFiniteSet([amberAxis : {}, zephyrAxis : x] \X {x})
====
