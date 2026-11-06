---- MODULE MixedView ----
EXTENDS Naturals
VARIABLES n, s
Init == n = 0 /\ s = {<<1>>, [a |-> 1, b |-> 1]}
Next == n < 3 /\ n' = n + 1 /\ UNCHANGED s
Inv == n < 3
V == n
====
