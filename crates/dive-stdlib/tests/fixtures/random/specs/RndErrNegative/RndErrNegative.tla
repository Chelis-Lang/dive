---- MODULE RndErrNegative ----
EXTENDS Integers, TLC, Randomization
ASSUME PrintT(RandomSubset(-1, 1..5))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
