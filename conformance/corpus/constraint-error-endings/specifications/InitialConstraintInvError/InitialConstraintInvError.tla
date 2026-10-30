---- MODULE InitialConstraintInvError ----
EXTENDS Integers
VARIABLE x
Init == x=2
Next == x'=x
Limit == x<2
Inv == 1 \div (2-x) >= 0
====
