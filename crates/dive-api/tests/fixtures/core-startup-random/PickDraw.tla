---- MODULE PickDraw ----
\* A parameterized random operator is never fixed at start-up.
EXTENDS Naturals, TLC
Pick(S) == RandomElement(S)
VARIABLE x
Init == x = 0
Next == x' = Pick({0, 1})
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x \in {0, 1})
====
