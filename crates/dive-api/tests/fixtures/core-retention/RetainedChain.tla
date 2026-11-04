---- MODULE RetainedChain ----
EXTENDS Naturals
VARIABLES n, payload
Init == /\ n = 0 /\ payload = [i \in 1..16 |-> <<0,i>>]
Next == /\ n < 80 /\ n' = n+1
        /\ payload' = [i \in 1..16 |-> <<n',i>>]
Inv == n < 64
====
