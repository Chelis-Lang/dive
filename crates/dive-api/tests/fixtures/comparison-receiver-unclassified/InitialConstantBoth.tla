---- MODULE InitialConstantBoth ----
EXTENDS Integers, FiniteSets
VARIABLE x
Tup == <<0>>
Rec == [a |-> 0]
Init == x=0
Next == x<2 /\ x'=x+1
Inv == Tup # Rec
====
