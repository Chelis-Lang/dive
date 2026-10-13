---- MODULE CdEval ----
\* Probe: A \cdot B evaluated in an action property. A step of +3 is not
\* two steps of +1.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == (x < 6 /\ x' = x + 2) \/ (x = 4 /\ x' = x + 3)
Prop == [][(x' = x + 1) \cdot (x' = x + 1)]_x
====
