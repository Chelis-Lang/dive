---- MODULE LvWFIntermittent ----
\* A is enabled only when b is TRUE, and Toggle flips b forever. WF_v(A) does
\* not force A, because A is not continuously enabled, so <>x is violated by
\* a lasso that toggles forever. Compare LvSFIntermittent.
VARIABLES x, b
vars == <<x, b>>
Init == x = FALSE /\ b = FALSE
Toggle == b' = ~b /\ UNCHANGED x
A == b /\ ~x /\ x' = TRUE /\ UNCHANGED b
Next == Toggle \/ A
Spec == Init /\ [][Next]_vars /\ WF_vars(Next) /\ WF_vars(A)
Prop == <>x
====
