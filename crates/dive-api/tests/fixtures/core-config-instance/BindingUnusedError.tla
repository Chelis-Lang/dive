---- MODULE BindingUnusedError ----
EXTENDS BindingBase, Integers
VARIABLE x
Target == [i \in 0..1 |-> IF i=0 THEN 5 ELSE 1 \div 0]
A == INSTANCE BindingSelect WITH v <- x
Init == x=0
Next == x'=IF x=0 THEN 1 ELSE x
Inv == IF x=0 THEN A!Read=5 ELSE TRUE
====
