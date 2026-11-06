---- MODULE BindingHolding ----
EXTENDS BindingBase, Naturals
VARIABLE x
Target == 5
A == INSTANCE BindingInner WITH v <- x
B == INSTANCE BindingInner WITH v <- x + 10
M == INSTANCE BindingMiddle WITH w <- x + 20
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Inv == C = 5 /\ A!Read = x+5 /\ B!Read = x+15 /\ M!Read = x+26
====
