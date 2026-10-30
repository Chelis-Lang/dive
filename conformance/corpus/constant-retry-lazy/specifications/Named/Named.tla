---- MODULE Named ----
EXTENDS Integers
VARIABLE x
F == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]
Init == x=0
Next == x'=1-x
Inv == F[0]=0
====
