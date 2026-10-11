---- MODULE ReplayPrintEmpty ----
EXTENDS Integers, TLC
VARIABLE x
Init == PrintT("") /\ x = 0
Next == x' = 1 \/ x' = 2 \/ x' = 3
Inv == IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
====
