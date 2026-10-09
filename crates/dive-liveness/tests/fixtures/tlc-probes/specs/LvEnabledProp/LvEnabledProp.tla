---- MODULE LvEnabledProp ----
\* WF_x(Next) itself as a property, which expands to a formula over ENABLED
\* and an action. It holds under the spec's own WF_x(Next).
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == WF_x(Next)
====
