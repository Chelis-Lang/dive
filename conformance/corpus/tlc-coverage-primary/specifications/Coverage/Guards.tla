---- MODULE Guards ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
A == /\ x<2 /\ x'=x+1
B == /\ x=2 /\ x'=x
Next == A \/ B
====
