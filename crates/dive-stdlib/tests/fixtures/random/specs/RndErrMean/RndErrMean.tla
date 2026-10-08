---- MODULE RndErrMean ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomSetOfSubsets(2, 4, 1..3))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
