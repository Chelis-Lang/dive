---- MODULE Symmetry ----
EXTENDS TLC
CONSTANT a,b
VARIABLE x
Init == x \in {a,b}
Next == x' \in {a,b}
Sym == Permutations({a,b})
====
