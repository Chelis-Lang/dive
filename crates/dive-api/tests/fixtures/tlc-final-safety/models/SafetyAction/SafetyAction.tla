---- MODULE SafetyAction ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' = ((x+1) % 3)
P == [][x' < 1]_x
====
