---- MODULE SafeFinal ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLES n,x
Init == n=0 /\ x=0
Next == /\ n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == x # 74
Live == <> (n=3)
====
