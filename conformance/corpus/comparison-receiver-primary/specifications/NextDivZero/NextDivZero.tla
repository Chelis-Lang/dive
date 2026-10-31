---- MODULE NextDivZero ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=0
Next == x<2 /\ 1 \div x=1 /\ x'=x+1
Inv == TRUE
====
