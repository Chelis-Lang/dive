---- MODULE RndErrThird ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomSetOfSubsets(2, 1, Nat))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
