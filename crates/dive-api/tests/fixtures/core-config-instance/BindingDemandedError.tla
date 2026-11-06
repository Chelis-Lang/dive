---- MODULE BindingDemandedError ----
EXTENDS BindingBase, Naturals
VARIABLE x
Target == 1
A == INSTANCE BindingErrorInner WITH v <- x
Init == x=0
Next == x'=IF x=0 THEN 1 ELSE x
Inv == A!Read=1
====
