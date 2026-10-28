---- MODULE SymView ----
\* VIEW with SYMMETRY: TLC evaluates the VIEW on the canonical image, so a
\* VIEW that is not symmetric (here, one process's counter and the total)
\* keys states by the image's values, not the state's.
EXTENDS Naturals, TLC
CONSTANT P
VARIABLE owner, count
Init == owner = [p \in P |-> 0] /\ count = 0
Next == \E p \in P : /\ owner[p] < 2
                     /\ owner' = [owner EXCEPT ![p] = @ + 1]
                     /\ count' = count + 1
Sym == Permutations(P)
View == <<owner[CHOOSE p \in P : TRUE], count>>
====
