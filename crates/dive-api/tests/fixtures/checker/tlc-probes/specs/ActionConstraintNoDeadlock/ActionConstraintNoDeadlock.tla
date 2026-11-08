---- MODULE ActionConstraintNoDeadlock ----
\* Probe: as ConstraintNoDeadlock, but the step 1 -> 2 is excluded by an
\* ACTION_CONSTRAINT. Expected: success, 2 distinct, 3 generated, no deadlock.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Small == x' < 2
====
