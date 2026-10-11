---- MODULE RuntimeDuplicates ----
EXTENDS Naturals,TLC
VARIABLE n,s
Init == n=0 /\ s="0"
Step == n<4 /\ n'=n+1 /\ s'=ToString(n+1)
Next == Step \/ Step
Inv == n<3
====
