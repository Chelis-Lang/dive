---- MODULE SafetyInitial ----
EXTENDS Naturals
VARIABLE x
Init == x=1
Next == UNCHANGED x
Inv == x = 0
====
