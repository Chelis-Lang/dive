---- MODULE Guard ----
EXTENDS Naturals,TLC
VARIABLES n,x
ASSUME PrintT("must not print")
Startup == PrintT("must not print")
Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=RandomElement(1..100)
Inv == n<3
V == n
====
