---- MODULE NxFastPath ----
\* Probe: once every variable is assigned, TLC evaluates the rest of the
\* action as booleans (getNextStatesAllAssigned): the disjunction and the
\* \E after the assignments produce one successor each, not two or three.
\* Before the assignment of y, the disjunction y' = 1 \/ y' = 1 produces
\* two. So each state generates 2 successors.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == /\ x < 3
        /\ x' = x + 1
        /\ (y' = 1 \/ y' = 1)
        /\ (TRUE \/ TRUE)
        /\ \E i \in {1, 2, 3} : TRUE
====
