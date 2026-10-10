---- MODULE SimConstrainedAction ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1
Prop == [][x' = x]_x
Limit == x = 0
====
