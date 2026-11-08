---- MODULE RolePart ----
EXTENDS Naturals
VARIABLE v
Init == v = 0
Next == v < 3 /\ v' = v + 1
P == [](v < 3)
====
