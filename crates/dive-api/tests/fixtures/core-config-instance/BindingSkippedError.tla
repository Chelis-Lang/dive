---- MODULE BindingSkippedError ----
EXTENDS BindingBase, Naturals
VARIABLE x
Target == 1
A == INSTANCE BindingErrorInner WITH v <- x
Init == x=0
Next == x'=IF x=0 THEN 1 ELSE x
Inv == IF x=0 THEN A!Read=1 ELSE TRUE
====
