---- MODULE Viewed ----
\* A VIEW with a temporal property: refused by the core, as by the capture.
EXTENDS Naturals
VARIABLES x, t
Init == x = 0 /\ t = 0
Next == x' = (x + 1) % 3 /\ t' = (t + 1) % 2
V == x
P == []<>(x = 0)
====
