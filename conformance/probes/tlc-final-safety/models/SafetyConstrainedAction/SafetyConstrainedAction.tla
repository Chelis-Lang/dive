---- MODULE SafetyConstrainedAction ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' = x+1
C == x < 1
P == [][x' < 1]_x
====
