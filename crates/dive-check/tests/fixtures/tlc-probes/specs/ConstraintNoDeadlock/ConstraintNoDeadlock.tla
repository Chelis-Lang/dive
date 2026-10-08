---- MODULE ConstraintNoDeadlock ----
\* Probe: deadlock checking is on. State 1's only successor, 2, fails
\* CONSTRAINT. TLC defines deadlock as "no successor generated", before the
\* constraint is applied, so this should be a success with 2 distinct states
\* and 3 generated (0, 1 and the excluded 2), not a deadlock.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Constr == x < 2
====
