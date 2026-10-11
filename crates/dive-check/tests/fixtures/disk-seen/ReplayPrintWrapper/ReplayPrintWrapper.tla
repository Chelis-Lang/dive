---- MODULE ReplayPrintWrapper ----
EXTENDS Integers, TLC
VARIABLE x
Emit == PrintT("wrapped")
Init == x = 0
Next == Emit /\ (x' = 1 \/ x' = 2 \/ x' = 3)
Inv == IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
====
