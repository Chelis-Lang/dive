---- MODULE SymAsym ----
\* A symmetry declaration the spec does not respect: only p1 may count to 2.
\* Under SYMMETRY the counts depend on which representative of each class TLC
\* explores, so they pin down TLC's exact choice.
EXTENDS Naturals, TLC
CONSTANTS Procs, p1
VARIABLE c
Init == c = [p \in Procs |-> 0]
Next == \E p \in Procs : c[p] < (IF p = p1 THEN 2 ELSE 1) /\ c' = [c EXCEPT ![p] = @ + 1]
Perms == Permutations(Procs)
====
