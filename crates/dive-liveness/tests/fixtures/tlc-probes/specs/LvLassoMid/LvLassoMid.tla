---- MODULE LvLassoMid ----
\* x goes 0, 1, 2, 3 and then loops back to 1. Next is always enabled and
\* weakly fair, so there is no stuttering. <>(x = 9) is violated by a lasso
\* whose cycle does not contain the initial state. Which state does TLC name
\* in "Back to state"?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x = 3 THEN 1 ELSE x + 1
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 9)
====
