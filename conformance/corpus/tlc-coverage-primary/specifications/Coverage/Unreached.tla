---- MODULE Unreached ----
EXTENDS Naturals
VARIABLE x,y
Init == /\ x=0 /\ y=0
A == /\ x<0 /\ x'=1 /\ y'=1
B == /\ x'=x /\ y'=y
Next == A \/ B
====
