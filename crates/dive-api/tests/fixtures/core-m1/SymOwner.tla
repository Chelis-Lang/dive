---- MODULE SymOwner ----
\* SYMMETRY over a function from model values: states equal up to a
\* permutation of P share one key; the explored states stay concrete.
EXTENDS Naturals, TLC
CONSTANT P
VARIABLE owner, count
Init == owner = [p \in P |-> 0] /\ count = 0
Next == \E p \in P : /\ owner[p] < 2
                     /\ owner' = [owner EXCEPT ![p] = @ + 1]
                     /\ count' = count + 1
Sym == Permutations(P)
Inv == count < 5
====
