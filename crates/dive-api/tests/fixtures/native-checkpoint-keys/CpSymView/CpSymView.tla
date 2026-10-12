---- MODULE CpSymView ----
EXTENDS Naturals, TLC
CONSTANT Procs
VARIABLES c, n
Init == c = [p \in Procs |-> 0] /\ n = 0
Next == \E p \in Procs : c[p] < 2 /\ c' = [c EXCEPT ![p] = @ + 1] /\ n' = 1-n
Perms == Permutations(Procs)
View == c
Inv == \A p \in Procs : c[p] < 2
====
