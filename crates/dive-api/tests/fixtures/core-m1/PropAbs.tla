---- MODULE PropAbs ----
\* The abstract specification PropRefine instantiates; also a fixture.
EXTENDS Naturals
VARIABLE n
Spec == n = 0 /\ [][n < 4 /\ n' = n + 1]_n
====
