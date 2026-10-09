---- MODULE DeadlockScopedCover ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Step(k) == x<k /\ x'=x+1
Next == Step(1)
====
