---- MODULE LvTautology ----
\* PROPERTY []<>TRUE is a tautology. Does TLC refuse it
\* (TLC_LIVE_FORMULA_TAUTOLOGY) or report success?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []<>TRUE
====
