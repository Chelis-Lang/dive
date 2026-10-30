---- MODULE ConstraintAction ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=1
Limit == x=0
Prop == [][x'=x]_x
====
