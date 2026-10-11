---- MODULE ActionProperty ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
P == [][x' < 2]_x
====
