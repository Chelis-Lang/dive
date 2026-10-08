---- MODULE RndErrNotSet ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomElement(3))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
