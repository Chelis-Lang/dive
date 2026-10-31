---- MODULE NextLiteral ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=0
Next == x<2 /\ <<x>> # [a |-> x] /\ x'=x+1
Inv == TRUE
====
