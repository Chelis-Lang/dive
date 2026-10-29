---- MODULE FirstErrorConstraint ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=x+1
First == IF x=2 THEN 1 \div 0 = 0 ELSE TRUE
Limit == x<2
Inv == x<2
====
