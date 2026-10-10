---- MODULE SafetyPropertyInitial ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == UNCHANGED x
P == x = 1
====
