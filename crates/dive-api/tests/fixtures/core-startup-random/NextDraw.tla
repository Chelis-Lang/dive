---- MODULE NextDraw ----
\* RandomElement drawn while checking: graph capture still refuses it.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = RandomElement({0, 1})
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x \in {0, 1})
====
