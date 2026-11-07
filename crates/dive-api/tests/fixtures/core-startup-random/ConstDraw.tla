---- MODULE ConstDraw ----
\* `C <- D` evaluates D's body at every use of the constant C.
EXTENDS Naturals, TLC
CONSTANT C
D == RandomElement(0..1)
VARIABLE x
Init == x = 0
Next == x' = C
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == <>[](x = 0)
====
