---- MODULE Strong ----
\* Set is enabled only when x = 0, so it is never continuously enabled on the
\* toggling lasso: WF admits that lasso as a counterexample to <>b, SF does not.
\* Tick is weakly fair in both, so neither admits stuttering.
EXTENDS Naturals
VARIABLES x, b
vars == <<x, b>>
Init == x = 0 /\ b = FALSE
Tick == x' = 1 - x /\ UNCHANGED b
Set == x = 0 /\ ~b /\ b' = TRUE /\ UNCHANGED x
Next == Tick \/ Set
Spec == Init /\ [][Next]_vars
WFSpec == Spec /\ WF_vars(Tick) /\ WF_vars(Set)
SFSpec == Spec /\ WF_vars(Tick) /\ SF_vars(Set)
EventuallyB == <>b
====
