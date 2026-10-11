---- MODULE ReplayMembership ----
EXTENDS Integers
VARIABLE x
Init == x = 0 \/ x = 2
Next == x = 0 /\ (x' = 1 \/ x' = 2)
Inv == IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
====
