---- MODULE Stream ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLES n,x
ASSUME RandomElement(1..100)>0
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == n<3
====
