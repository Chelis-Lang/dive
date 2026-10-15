---- MODULE InitRandomNonSet ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement(42)
Next == UNCHANGED x
Safe == TRUE
====
