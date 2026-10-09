---- MODULE SymCountsNone ----
\* SymCounts without SYMMETRY: the baseline counts.
EXTENDS Naturals, TLC
CONSTANT Procs
VARIABLE c
Init == c = [p \in Procs |-> 0]
Next == \E p \in Procs : c[p] < 2 /\ c' = [c EXCEPT ![p] = @ + 1]
====
