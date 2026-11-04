---- MODULE DynamicNames ----
EXTENDS Naturals, TLC
VARIABLES n, name
Init == /\ n = 0 /\ name = ""
Next == /\ n < 12 /\ n' = n+1 /\ name' = ToString(n')
Inv == n < 10
====
