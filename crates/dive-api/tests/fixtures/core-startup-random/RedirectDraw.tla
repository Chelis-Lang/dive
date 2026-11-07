---- MODULE RedirectDraw ----
\* `Op <- D` evaluates D's body at every use, so D's RandomElement draws
\* while checking even though D alone would be fixed at start-up.
EXTENDS Naturals, TLC
Op == 0
D == RandomElement(0..1)
VARIABLE x
Init == x = 0
Next == x' = Op
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == <>[](x = 0)
====
