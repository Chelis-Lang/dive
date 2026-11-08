---- MODULE SimConstraintDeadlock ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
C == x < 3
====
