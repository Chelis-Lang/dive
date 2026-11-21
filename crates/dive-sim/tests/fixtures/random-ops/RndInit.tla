---- MODULE RndInit ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement(1..1000000)
Next == x' = x + 1
Safe == x >= 0
====
