---- MODULE InitRandomNextBoundary ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement({2,7})
Next == PrintT("next-before-random") /\ x'=RandomElement({2,7})
Safe == TRUE
====
