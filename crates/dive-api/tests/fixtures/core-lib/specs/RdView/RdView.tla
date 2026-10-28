---- MODULE RdView ----
\* RandomElement with a VIEW: the reference engine refuses random streams
\* under VIEW or SYMMETRY, and the core refuses them too.
EXTENDS Naturals, TLC
VARIABLE x, n
Init == x = 0 /\ n = 0
Next == n < 3 /\ n' = n + 1 /\ x' = RandomElement(1..5)
V == n
====
