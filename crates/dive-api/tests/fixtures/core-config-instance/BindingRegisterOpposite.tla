---- MODULE BindingRegisterOpposite ----
EXTENDS BindingBase, Naturals, TLC
CONSTANT D
VARIABLE x
Target == D
Effect == IF TLCSet(0,7) THEN 5 ELSE 0
A == INSTANCE BindingInner WITH v <- x
Init == TLCSet(0,0) /\ x=0
Next == TLCSet(0,0) /\ x'=IF x<2 THEN x+1 ELSE x
Inv == A!Read=x+5 /\ TLCGet(0)#7
====
