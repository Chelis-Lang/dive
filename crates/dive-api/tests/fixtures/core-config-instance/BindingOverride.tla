---- MODULE BindingOverride ----
EXTENDS BindingBase, Naturals
VARIABLE x
Target == 2
A == INSTANCE BindingInner WITH v <- x
B == INSTANCE BindingInner WITH v <- x + 10
M == INSTANCE BindingMiddle WITH w <- x + 20
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Inv == C = 9 /\ Target = 9 /\ A!Read = 9+x
====
