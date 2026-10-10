---- MODULE SySubstitution ----
EXTENDS TLC
CONSTANT S
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == TRUE
Other == Permutations(S)
====
