---- MODULE ReplayLaterViolation ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = 1 \/ x' = 2 \/ x' = 3
InvA == IF x = 1 THEN (1 \div 0) = 0 ELSE TRUE
InvB == x # 2
====
