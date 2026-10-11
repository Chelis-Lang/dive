---- MODULE ConfiguredView ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLES n,x
Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=C(1..100)
Inv == n<3
V == n
====
