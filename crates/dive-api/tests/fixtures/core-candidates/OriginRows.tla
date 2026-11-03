---- MODULE OriginRows ----
EXTENDS Naturals
VARIABLES x, p
Init == x = {} /\ p = 0
Next == /\ p = 0
        /\ p' = 1
        /\ \E k \in 1..128 : x' = {k}
====
