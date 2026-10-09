---- MODULE LvMixedLiveOnly ----
\* PROPERTY Prop == [](x < 3) /\ <>(x = 9): the safety conjunct holds, so
\* only the liveness conjunct is violated. Which name does TLC report: Prop,
\* or the conjunct?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == [](x < 3) /\ <>(x = 9)
====
