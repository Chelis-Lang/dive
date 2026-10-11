---- MODULE Guard ----
EXTENDS Naturals,TLC
VARIABLES n,x
CONSTANT C(_)
Replacement(S) == CHOOSE a \in S : TRUE
Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=C(1..100)
Inv == n<3
V == n
====
