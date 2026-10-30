---- MODULE FirstFalseConstraint ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=x+1
Limit == x<2
Later == IF x=2 THEN 1 \div 0 = 0 ELSE TRUE
Inv == 1 \div (2-x) >= 0
====
