---- MODULE Stream ----
EXTENDS Naturals,TLC
C(S) == CHOOSE a \in S : TRUE
VARIABLES n,x

Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=RandomElement(1..100)
Inv == n<3
====
