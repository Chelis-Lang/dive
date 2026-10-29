---- MODULE InitialConstraintProperty ----
EXTENDS Integers
VARIABLE x
Init == x=2
Next == x'=x
Limit == x<2
Prop == x<2
====
