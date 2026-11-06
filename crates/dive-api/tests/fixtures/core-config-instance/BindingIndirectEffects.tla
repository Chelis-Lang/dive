---- MODULE BindingIndirectEffects ----
EXTENDS BindingBase, Naturals, TLC
CONSTANT D
VARIABLE x
Target == D
Effect == IF PrintT("bound-target") THEN 5 ELSE 0
A == INSTANCE BindingInner WITH v <- x
B == INSTANCE BindingInner WITH v <- x + 10
M == INSTANCE BindingMiddle WITH w <- x + 20
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Inv == A!Read=x+5 /\ B!Read=x+15
====
