---- MODULE InitRandomEmpty ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement({})
Next == UNCHANGED x
Safe == TRUE
====
