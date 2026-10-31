---- MODULE NextConstant ----
EXTENDS Integers, FiniteSets
VARIABLE x
Tup == <<0>>
Rec == [a |-> 0]
Init == x=0
Next == x<2 /\ Tup # Rec /\ x'=x+1
Inv == TRUE
====
