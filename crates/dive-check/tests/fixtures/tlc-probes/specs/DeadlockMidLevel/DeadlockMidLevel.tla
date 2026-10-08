---- MODULE DeadlockMidLevel ----
\* Probe: counts and depth at a deadlock. Level 2 holds 1 and 2. Expanding 1
\* generates 3 (level 3). Expanding 2 generates nothing: deadlock.
\* Expected: 4 generated, 4 distinct, 1 on the queue (state 3), and a reported
\* depth of 3 although the counterexample (0, 2) has 2 states.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' \in {1, 2}
        \/ x = 1 /\ x' = 3
        \/ x = 3 /\ x' = 3
====
