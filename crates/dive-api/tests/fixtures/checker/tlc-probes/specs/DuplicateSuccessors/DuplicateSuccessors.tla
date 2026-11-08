---- MODULE DuplicateSuccessors ----
\* Probe: "states generated" counts every successor TLC's enumeration produces,
\* duplicates included. From each of the two states 0 and 1:
\*   x' = 1 \/ x' = 1              2 successors (both 1)
\*   x' \in {0, 1} /\ x' = 1       1 successor (0 fails the equality check)
\*   \E i \in {1, 2} : x' = 1      2 successors (one action per binding)
\* Expected: 2 distinct; 1 + 5 + 5 = 11 generated.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ (x' = 1 \/ x' = 1)
        \/ (x' \in {0, 1} /\ x' = 1)
        \/ \E i \in {1, 2} : x' = 1
====
