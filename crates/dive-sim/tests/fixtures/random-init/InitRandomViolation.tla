---- MODULE InitRandomViolation ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement(1..1000000)
Next == UNCHANGED x
Safe == x<0
====
