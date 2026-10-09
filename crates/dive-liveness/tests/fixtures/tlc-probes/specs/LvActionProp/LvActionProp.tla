---- MODULE LvActionProp ----
\* []<><<x' = 0>>_x: infinitely many steps that reset x. Without fairness it
\* is violated by stuttering; the ending shows how TLC reports a violation of
\* a property over actions.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
Prop == []<><<x' = 0>>_x
====
