---- MODULE RndErrInfinite ----
EXTENDS Naturals, TLC, Randomization
ASSUME PrintT(RandomElement([a : {1}, b : Nat]))
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
