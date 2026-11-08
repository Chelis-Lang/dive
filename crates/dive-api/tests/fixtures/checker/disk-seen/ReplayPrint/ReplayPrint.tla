---- MODULE ReplayPrint ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == PrintT("expand") /\ (x' = 1 \/ x' = 2 \/ x' = 3)
Inv == IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
====
