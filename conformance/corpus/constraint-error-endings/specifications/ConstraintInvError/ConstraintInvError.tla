---- MODULE ConstraintInvError ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=x+1
Limit == x<2
Inv == 1 \div (2-x) >= 0
====
