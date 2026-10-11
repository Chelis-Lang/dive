---- MODULE Transport ----
EXTENDS Naturals
VARIABLES n, s
Init == /\ n = 0 /\ s = 1..2
Next == /\ n' = n + 1 /\ UNCHANGED s
Inv == n < 3
====
