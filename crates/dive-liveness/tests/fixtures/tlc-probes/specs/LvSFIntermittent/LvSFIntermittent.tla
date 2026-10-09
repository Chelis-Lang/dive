---- MODULE LvSFIntermittent ----
\* As LvWFIntermittent, but with SF_v(A): A is enabled infinitely often, so
\* it must happen, and <>x holds.
VARIABLES x, b
vars == <<x, b>>
Init == x = FALSE /\ b = FALSE
Toggle == b' = ~b /\ UNCHANGED x
A == b /\ ~x /\ x' = TRUE /\ UNCHANGED b
Next == Toggle \/ A
Spec == Init /\ [][Next]_vars /\ WF_vars(Next) /\ SF_vars(A)
Prop == <>x
====
