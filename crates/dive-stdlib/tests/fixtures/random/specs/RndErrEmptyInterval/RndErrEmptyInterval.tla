---- MODULE RndErrEmptyInterval ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomElement(1..0))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
