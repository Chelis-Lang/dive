---- MODULE ReplayPrintInvariant ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = 1 \/ x' = 2 \/ x' = 3
Inv == PrintT("invariant") /\ IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
====
