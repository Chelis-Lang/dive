---- MODULE Admission ----
EXTENDS Naturals,TLC
VARIABLES n,x
ASSUME RandomElement({TRUE})
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=RandomElement(1..100)
Projection == n
Inv == n<3
====
