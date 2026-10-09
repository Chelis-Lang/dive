---- MODULE SymAsymLate ----
\* SymAsym with the asymmetric process enumerated last (p3 may count to 2).
\* The counts differ from SymAsym only if TLC's choice of explored
\* representative depends on enumeration order.
EXTENDS Naturals, TLC
CONSTANTS Procs, p3
VARIABLE c
Init == c = [p \in Procs |-> 0]
Next == \E p \in Procs : c[p] < (IF p = p3 THEN 2 ELSE 1) /\ c' = [c EXCEPT ![p] = @ + 1]
Perms == Permutations(Procs)
====
