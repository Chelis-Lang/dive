---- MODULE Guard ----
EXTENDS Naturals,TLC
VARIABLES n,x
CONSTANTS C(_),D(_)
Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=C(1..100)
Inv == n<3
V == n
====
