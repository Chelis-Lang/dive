---- MODULE SimLevelConstraintDeadlock ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = x + 1
L == TLCGet("level") < 4
====
