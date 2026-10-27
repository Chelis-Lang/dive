---- MODULE PermViol ----
\* A violation whose counts depend on the order Permutations enumerates in.
EXTENDS Naturals, TLC
VARIABLE p, n
Init == p = <<1, 2, 3, 4>> /\ n = 0
Next == n < 3 /\ \E q \in Permutations(1..4) : p' = [i \in 1..4 |-> p[q[i]]] /\ n' = n + 1
Inv == ~(n = 2 /\ p = <<2, 4, 1, 3>>)
====
