---- MODULE Fair ----
\* Tick toggles x forever; Set raises b once. Without fairness the lasso
\* that only ticks violates <>b; under WF or SF on Set it is not a behavior.
EXTENDS Naturals
VARIABLES x, b
vars == <<x, b>>
Init == x = 0 /\ b = FALSE
Tick == x' = 1 - x /\ UNCHANGED b
Set == ~b /\ b' = TRUE /\ UNCHANGED x
Next == Tick \/ Set
Spec == Init /\ [][Next]_vars
WFSpec == Spec /\ WF_vars(Set)
SFSpec == Spec /\ SF_vars(Set)
EventuallyB == <>b
====
