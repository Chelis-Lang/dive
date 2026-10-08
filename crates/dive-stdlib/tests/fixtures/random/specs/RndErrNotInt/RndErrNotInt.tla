---- MODULE RndErrNotInt ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomSubset("a", 1..5))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
