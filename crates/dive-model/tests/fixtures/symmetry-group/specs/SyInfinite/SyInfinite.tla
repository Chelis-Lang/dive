---- MODULE SyInfinite ----
EXTENDS TLC, Naturals
CONSTANT a, b
VARIABLE x
Init == x = Nat
Next == UNCHANGED x
Sym == Permutations({a, b})
====
