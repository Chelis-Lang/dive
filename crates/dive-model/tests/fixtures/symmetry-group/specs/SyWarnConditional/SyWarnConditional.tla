---- MODULE SyWarnConditional ----
EXTENDS TLC
CONSTANT S
Sym == IF TRUE THEN Permutations( (* comment *) S ) ELSE Permutations(S)
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
====
