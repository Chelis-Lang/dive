---- MODULE SyWarnNonModel ----
EXTENDS TLC
Sym == Permutations({1})
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
====
