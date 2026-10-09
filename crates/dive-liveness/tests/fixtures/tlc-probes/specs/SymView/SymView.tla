---- MODULE SymView ----
\* SYMMETRY and VIEW together. The view drops n, a step counter that is not
\* symmetric-relevant. What are the counts?
EXTENDS Naturals, TLC
CONSTANT Procs
VARIABLES c, n
Init == c = [p \in Procs |-> 0] /\ n = 0
Next == \E p \in Procs : c[p] < 1 /\ c' = [c EXCEPT ![p] = @ + 1] /\ n' = n + 1
Perms == Permutations(Procs)
View == c
====
