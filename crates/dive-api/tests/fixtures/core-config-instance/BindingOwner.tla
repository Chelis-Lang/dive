---- MODULE BindingOwner ----
EXTENDS BindingBase, Naturals
CONSTANT K
VARIABLE x
Target == K
A == INSTANCE BindingOwnedInner WITH K <- 17, v <- x
B == INSTANCE BindingOwnedInner WITH K <- 17, v <- x + 10
M == INSTANCE BindingMiddle WITH w <- x + 20
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Inv == A!Read=x+22 /\ B!Read=x+32
====
