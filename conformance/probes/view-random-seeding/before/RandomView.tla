---- MODULE RandomView ----
EXTENDS Naturals,TLC
VARIABLES n,x
Init == n=0 /\ x=0
Next == n<4 /\ n'=n+1 /\ x'=RandomElement(1..100)
Inv == n<3
V == n
====
