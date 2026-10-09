---- MODULE LvConstraint ----
\* x increases without bound; CONSTRAINT x < 3 cuts the graph. The state x =
\* 2 has a successor outside the model. Is <>(x = 5) reported violated, and
\* how does the counterexample end, when the only behaviours in the model end
\* at the constraint boundary?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 5)
Bound == x < 3
====
