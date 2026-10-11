---- MODULE Guard ----
EXTENDS Naturals,TLC
VARIABLES n,x
CONSTANT C(_)
Init == n=0 /\ x=0
Next == n'=n+1 /\ x'=C(1..100)
Inv == x # 80
V == n
====
