---- MODULE SymTwoSets ----
\* Two symmetry sets, Permutations(A) \cup Permutations(B). Is the reduction
\* by the group the union generates (|A|! * |B|! permutations), or by the
\* union as given?
EXTENDS Naturals, TLC
CONSTANTS A, B
VARIABLES x, y
Init == x = [a \in A |-> 0] /\ y = [b \in B |-> 0]
Next == \/ \E a \in A : x[a] = 0 /\ x' = [x EXCEPT ![a] = 1] /\ UNCHANGED y
        \/ \E b \in B : y[b] = 0 /\ y' = [y EXCEPT ![b] = 1] /\ UNCHANGED x
Perms == Permutations(A) \cup Permutations(B)
====
