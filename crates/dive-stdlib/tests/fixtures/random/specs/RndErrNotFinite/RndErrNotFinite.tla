---- MODULE RndErrNotFinite ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomSubset(2, Nat))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
