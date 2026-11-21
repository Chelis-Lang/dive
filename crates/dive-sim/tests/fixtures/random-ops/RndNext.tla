---- MODULE RndNext ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = RandomElement(1..1000000)
Safe == x >= 0
====
