---- MODULE RndErrPicks ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomSetOfSubsets(9, 1, 1..3))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
