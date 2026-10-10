---- MODULE SafetyConstrainedInitial ----
EXTENDS Naturals
VARIABLE x
Init == x=1
Next == UNCHANGED x
C == x<1
P == x=0
====
