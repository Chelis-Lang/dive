---- MODULE PropertyAlias ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Original == [](x < 3)
P == Original
====
