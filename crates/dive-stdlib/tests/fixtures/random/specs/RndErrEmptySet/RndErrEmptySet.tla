---- MODULE RndErrEmptySet ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomElement({}))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
