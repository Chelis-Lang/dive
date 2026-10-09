---- MODULE SyNonModel ----
EXTENDS TLC
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == Permutations({1,2})
====
