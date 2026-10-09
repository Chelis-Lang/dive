---- MODULE SymViolation ----
\* An invariant violation under SYMMETRY. Which states does TLC print in the
\* counterexample, and is its length the breadth-first length?
EXTENDS Naturals, TLC
CONSTANT Procs
VARIABLE c
Init == c = [p \in Procs |-> 0]
Next == \E p \in Procs : c[p] < 2 /\ c' = [c EXCEPT ![p] = @ + 1]
Perms == Permutations(Procs)
Inv == \A p \in Procs : c[p] < 2
====
