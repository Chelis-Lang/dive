---- MODULE BindingCapture ----
EXTENDS BindingBase, Naturals
VARIABLE x
Target == 5
A == INSTANCE BindingInner WITH v <- x
B == INSTANCE BindingInner WITH v <- x + 10
M == INSTANCE BindingMiddle WITH w <- x + 20
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Use(n) == LET Local == n + A!Read IN Local
Inv == Use(10) = x+15 /\ Use(30) = x+35
====
