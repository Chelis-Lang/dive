---- MODULE SymOutside ----
\* A state outside the model is checked without a lookup: x = p2 has the
\* image of x = p1, already seen, yet its (asymmetric) invariant fails.
EXTENDS TLC
CONSTANT P, p2
VARIABLE x
Init == x = "none"
Next == \E p \in P : x' = p
Constr == x # p2
Inv == x # p2
Sym == Permutations(P)
====
