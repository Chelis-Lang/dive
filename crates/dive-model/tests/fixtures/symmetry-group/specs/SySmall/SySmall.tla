---- MODULE SySmall ----
EXTENDS TLC
CONSTANT S
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == Permutations(S)
====
