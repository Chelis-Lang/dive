---- MODULE RndInvariant ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = 1 - x
Safe == x # RandomElement({0, 1})
====
