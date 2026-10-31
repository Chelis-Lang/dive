---- MODULE WholeFunctionError ----
EXTENDS Integers
VARIABLE x
F == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]
Inv == F=F
Init == x=0
Next == x'=x
====
