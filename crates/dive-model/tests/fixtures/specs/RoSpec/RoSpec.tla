---- MODULE RoSpec ----
EXTENDS Naturals
VARIABLES x, y
vars == <<x, y>>
Init == x = 0
YInit == y = 0
Next == x' = 1 - x /\ UNCHANGED y
MyInit(i) == i
Spec == MyInit(Init) /\ (y \in {0}) /\ LET v == vars IN [][Next]_v /\ WF_vars(Next)
====
