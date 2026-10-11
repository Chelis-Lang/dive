---- MODULE Guard ----
EXTENDS Naturals,TLC
VARIABLES n,x

Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=n
Inv == n<3
V == n
====
