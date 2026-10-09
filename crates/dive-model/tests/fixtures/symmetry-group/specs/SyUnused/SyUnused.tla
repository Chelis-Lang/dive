---- MODULE SyUnused ----
EXTENDS TLC
CONSTANT A,B
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == Permutations(A) \cup Permutations(B)
====
