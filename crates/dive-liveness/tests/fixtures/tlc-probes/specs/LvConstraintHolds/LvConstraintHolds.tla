---- MODULE LvConstraintHolds ----
\* As LvConstraint, but the property is <>(x = 2), which holds on every path
\* inside the constraint. Does TLC report success?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 2)
Bound == x < 3
====
