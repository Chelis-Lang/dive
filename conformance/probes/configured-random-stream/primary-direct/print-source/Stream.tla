---- MODULE Stream ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLES n,x
ASSUME Print(RandomElement(1..100),TRUE)
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=RandomElement(1..100)
Inv == n<3
====
