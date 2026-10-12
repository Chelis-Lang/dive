---- MODULE CpSymConcrete ----
EXTENDS Naturals, TLC
CONSTANTS A, B
VARIABLES owner, step
Init == owner = B /\ step = 0
Next == owner' = owner /\ step < 3 /\ step' = step+1
Perms == Permutations({A,B})
Inv == step < 2
====
