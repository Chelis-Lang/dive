---- MODULE NxStepAngle ----
\* Probe: <A>_v keeps only successors where v changes; [A]_v adds a
\* stuttering successor after A's.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
A == x' \in {x, x + 1} /\ y' = y
B == [x < 2 /\ x' = x + 1 /\ y' = y]_<<x, y>>
Next == (x < 3 /\ <<A>>_x) \/ B
====
