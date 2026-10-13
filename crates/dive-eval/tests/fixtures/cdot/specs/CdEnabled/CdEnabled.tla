---- MODULE CdEnabled ----
\* Probe: ENABLED of A \cdot B in an invariant, through TLC's enabled search.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == ENABLED ((x' = x + 1) \cdot (x < 2 /\ x' = x + 1))
====
