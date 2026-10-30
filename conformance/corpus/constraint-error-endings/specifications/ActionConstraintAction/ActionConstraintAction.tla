---- MODULE ActionConstraintAction ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=1
Limit == x'=x
Prop == [][x'=x]_x
====
