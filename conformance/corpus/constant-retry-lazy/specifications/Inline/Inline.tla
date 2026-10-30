---- MODULE Inline ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=1-x
Inv == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0][0]=0
====
